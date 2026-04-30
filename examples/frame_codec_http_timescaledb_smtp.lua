-- WZ-S Formaldehyde Sensor Monitor
-- Connects a WZ-S formaldehyde sensor via UART with frame codec.
-- Polls the WZ-S in Question-Answer mode, parses ppb/ug/m³ readings, posts to
-- Home Assistant via HTTP, inserts into TimescaleDB, and sends SMTP alarm
-- emails with hysteresis-based debounce on threshold breach and recovery.
--
-- Device: WZ-S Formaldehyde Detection Module (UART Sensor)
--   Communication: 9600 baud, 8 data bits, no parity, 1 stop bit
--   Logic level: 0V to 3.3V; supply voltage: 5V to 7V
--   Frame format: 9-byte fixed-length, big-endian, 0xFF start byte
--   Checksum (byte 8): (NOT(Sum(Bytes 1..7))) + 1 — LRC variant, scope = payload
--   Warm-up: < 3 min; T90: < 40 s; T10: < 60 s
--   Measurement range: 0–2 ppm; max overload: 10 ppm
--
-- Frame Map (payload excludes 0xFF prefix and trailing checksum — codec handles both):
--   Host → Device:
--     01 78 41 00 00 00 00   Switch to QA Mode
--     01 78 40 00 00 00 00   Switch to Active Upload Mode
--     01 86 00 00 00 00 00   Read Concentration (QA)
--   Device → Host (payload byte 1 = command):
--     17 04 DP H L FRH FRL   Active Upload — ppb at bytes 4-5, full range bytes 6-7
--     86 UH UL -- -- PH PL   QA Response — ug/m³ at bytes 2-3, ppb at bytes 6-7
--
-- TimescaleDB Setup (run once before starting the script):
--   CREATE TABLE sensor_readings (
--       time                TIMESTAMPTZ NOT NULL DEFAULT NOW(),
--       sensor              TEXT        NOT NULL,
--       formaldehyde_ppb    INTEGER,
--       formaldehyde_ugm3   INTEGER
--   );
--   SELECT create_hypertable('sensor_readings', 'time', if_not_exists => TRUE);
--   -- Optional: add an index for fast per-sensor queries
--   CREATE INDEX ON sensor_readings (sensor, time DESC);

local HA_URL = get_env("HA_URL") or "http://localhost:8123/api/states/sensor.wz_s_formaldehyde"
local HA_TOKEN = get_env("HA_TOKEN") or "YOUR_LONG_LIVED_ACCESS_TOKEN"
local TS_CONN = get_env("TS_CONN") or "host=localhost port=5432 dbname=cycbox user=postgres password=secret sslmode=disable"

local POLL_INTERVAL = 5000 -- Poll every 5 seconds
local timer_counter = 0
local db_connected = false

-- Alarm Config
local ALARM_HIGH_PPB = 80
local ALARM_LOW_PPB = 50

local SMTP_CONFIG = {
    server   = "smtp.gmail.com",
    port     = 587,
    tls      = "starttls",
    username = "your-email@gmail.com",
    password = "your-app-password",
    from     = "your-email@gmail.com",
    to       = "recipient@example.com",
}

local is_ppb_high = false

local function send_alarm_email(ppb_val, state)
    local subject, msg
    if state == "HIGH" then
        subject = string.format("ALARM: Formaldehyde High (%d ppb)", ppb_val)
        msg = string.format("Formaldehyde level has exceeded the high threshold (%d ppb). Current value: %d ppb.", ALARM_HIGH_PPB, ppb_val)
        log("warn", msg)
    else
        subject = string.format("RECOVERY: Formaldehyde Normal (%d ppb)", ppb_val)
        msg = string.format("Formaldehyde level has dropped below the low threshold (%d ppb). Current value: %d ppb.", ALARM_LOW_PPB, ppb_val)
        log("info", msg)
    end

    smtp_send_async({
        server   = SMTP_CONFIG.server,
        port     = SMTP_CONFIG.port,
        tls      = SMTP_CONFIG.tls,
        username = SMTP_CONFIG.username,
        password = SMTP_CONFIG.password,
        from     = SMTP_CONFIG.from,
        to       = SMTP_CONFIG.to,
        subject  = subject,
        text     = msg,
    })
end

function on_start()
    -- Connect to TimescaleDB
    local ok, err = timescaledb_connect(TS_CONN, 3)
    if ok then
        db_connected = true
        log("info", "Connected to TimescaleDB")
    else
        log("error", "TimescaleDB connection failed: " .. (err or "unknown"))
    end

    -- Switch to Question-Answer (QA) Mode
    -- The frame codec auto-appends 0xFF prefix and the 1-byte checksum.
    local qa_mode_cmd = string.char(0x01, 0x78, 0x41, 0x00, 0x00, 0x00, 0x00)
    send_after(qa_mode_cmd, 100, 0)
    log("info", "WZ-S script started. Switched to QA mode.")
end

function on_receive()
    -- Only process data from connection 0
    if message.connection_id ~= 0 then return false end
    -- The frame_codec auto-verifies the LRC checksum for us
    if not message.checksum_valid then return false end

    -- Payload excludes the 0xFF prefix and the 1-byte checksum tailer
    local payload = message.payload
    if not payload or #payload < 7 then return false end

    local cmd = string.byte(payload, 1)
    local ppb = nil
    local ugm3 = nil

    if cmd == 0x17 then
        -- Active Upload Mode data (handled just in case)
        ppb = string.byte(payload, 4) * 256 + string.byte(payload, 5)
        log("info", string.format("Active Upload - Formaldehyde: %d ppb", ppb))
        
    elseif cmd == 0x86 then
        -- Question-Answer Mode data response
        ugm3 = string.byte(payload, 2) * 256 + string.byte(payload, 3)
        ppb = string.byte(payload, 6) * 256 + string.byte(payload, 7)
        log("info", string.format("QA Read - Formaldehyde: %d ppb, %d ug/m3", ppb, ugm3))
        
    else
        -- Ignore Acknowledge messages (like 0x78) or unknown commands
        return false
    end

    -- Add parsed values to the message context
    if ppb then message:add_int_value("formaldehyde_ppb", ppb) end
    if ugm3 then message:add_int_value("formaldehyde_ugm3", ugm3) end

    if ppb then
        if ppb > ALARM_HIGH_PPB and not is_ppb_high then
            is_ppb_high = true
            send_alarm_email(ppb, "HIGH")
        elseif ppb <= ALARM_LOW_PPB and is_ppb_high then
            is_ppb_high = false
            send_alarm_email(ppb, "NORMAL")
        end

        -- Send values to Home Assistant via HTTP POST
        local ugm3_attr = ugm3 or 0
        local ha_body = string.format('{"state": %d, "attributes": {"unit_of_measurement": "ppb", "device_class": "volatile_organic_compounds", "ugm3": %d}}', ppb, ugm3_attr)
        local headers = {
            ["Authorization"] = "Bearer " .. HA_TOKEN,
            ["Content-Type"] = "application/json"
        }
        http_post(HA_URL, ha_body, headers)
    end

    -- Asynchronously insert into TimescaleDB
    if db_connected and ppb then
        local ugm3_val = ugm3 or 0
        timescaledb_insert_async("sensor_readings", 
            {"sensor", "formaldehyde_ppb", "formaldehyde_ugm3"}, 
            {"wz_s", ppb, ugm3_val})
    end

    return true
end

function on_timer(now_ms)
    timer_counter = timer_counter + 100
    if timer_counter >= POLL_INTERVAL then
        timer_counter = 0
        
        -- Poll concentration (QA Mode Read request)
        -- The frame codec will append 0xFF and the LRC checksum.
        local read_cmd = string.char(0x01, 0x86, 0x00, 0x00, 0x00, 0x00, 0x00)
        send_after(read_cmd, 0, 0)
    end
end

function on_stop()
    if db_connected then
        timescaledb_disconnect()
    end
end


--[[
{
  "version": "2.0.0",
  "name": "WZ-S Formaldehyde Sensor Monitor",
  "description": "Connects a WZ-S formaldehyde sensor via UART with frame codec.",
  "configs": [
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "frame_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "serial_port_transport": {
        "serial_port_transport_port": "/dev/ttyACM0",
        "serial_port_transport_baud_rate": 9600,
        "serial_port_transport_data_bits": 8,
        "serial_port_transport_parity": "none",
        "serial_port_transport_stop_bits": "1",
        "serial_port_transport_flow_control": "none"
      },
      "frame_codec": {
        "frame_codec_prefix": "FF",
        "frame_codec_header_size": 0,
        "frame_codec_tailer_length": 0,
        "frame_codec_suffix": "",
        "frame_codec_length_mode": "fixed",
        "frame_codec_fixed_payload_size": 7,
        "frame_codec_length_meaning": "payload_only",
        "frame_codec_checksum_algo": "lrc",
        "frame_codec_checksum_scope": "payload"
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "28HSKYT2",
      "name": "Operating Mode",
      "inputs": [
        {
          "input_type": "frame",
          "id": "28GR6PFW",
          "name": "Switch to QA Mode (Polling)",
          "header": "",
          "payload": "01 78 41 00 00 00 00",
          "tailer": "",
          "connection_id": 0
        },
        {
          "input_type": "frame",
          "id": "2882YPPW",
          "name": "Switch to Active Mode (Periodic)",
          "header": "",
          "payload": "01 78 40 00 00 00 00",
          "tailer": "",
          "connection_id": 0
        }
      ]
    },
    {
      "id": "2835H8V8",
      "name": "Data Acquisition",
      "inputs": [
        {
          "input_type": "frame",
          "id": "280DQX25",
          "name": "Read Concentration (QA Mode)",
          "header": "",
          "payload": "01 86 00 00 00 00 00",
          "tailer": "",
          "connection_id": 0
        }
      ]
    }
  ]
}
]]
