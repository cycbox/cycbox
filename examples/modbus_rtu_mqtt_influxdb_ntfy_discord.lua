-- Senseair S8 CO2 Sensor Monitor
-- Connects a Senseair S8 CO2 sensor via Modbus RTU over serial port, an MQTT client, and an MQTT broker.
-- Polls Senseair S8 CO2 sensor via Modbus RTU over serial, publishes readings to MQTT and InfluxDB v3,
-- and sends ntfy/Discord alerts on CO2 threshold breach.
--
-- Device: Senseair S8 CO2 Sensor (Modbus RTU Slave)
--   Slave Address: 1–247; 0xFE = broadcast "any sensor" (point-to-point test)
--   Communication: 9600 baud, 8 data bits, no parity, 1 stop bit (2 on TX)
--   Response timeout: 180 ms max; poll interval: 2 s (lamp cycle period)
--   Max 8 registers per read, max packet 39 bytes (including CRC)
--
-- Input Register Map (Function 0x04, 0-based wire addresses):
--   0x0000  MeterStatus       RO  System fault flags (0x0000 = healthy)
--                                   Bit 0: ERR_FATAL
--                                   Bit 1: ERR_OFFSET_REG
--                                   Bit 2: ERR_ALGORITHM
--                                   Bit 3: ERR_OUTPUT
--                                   Bit 4: ERR_SELF_DIAG
--                                   Bit 5: ERR_OUT_OF_RANGE
--                                   Bit 6: ERR_MEMORY
--   0x0001  AlarmStatus       RO  Alarm flags (reserved)
--   0x0002  Output Status     RO  Bit 0: ALARM_OUT (inverted, open-collector)
--                                   Bit 1: PWM_OUT (1 = full output)
--   0x0003  Space CO2         RO  Measured CO2 concentration (ppm)
--   0x0019  Sensor Type ID Hi RO  Device Type ID upper 16 bits
--   0x001A  Sensor Type ID Lo RO  Device Type ID lowest 8 bits (in high byte)
--   0x001B  Memory Map Ver    RO  Memory map structure version
--   0x001C  FW Version        RO  Firmware Main (bits 15:8) . Sub (bits 7:0)
--   0x001D  Sensor ID High    RO  Serial number upper 16 bits
--   0x001E  Sensor ID Low     RO  Serial number lower 16 bits
--
-- Holding Register Map (Function 0x03 read / 0x06 write):
--   0x0000  Acknowledgement   R/W Calibration completion flags
--                                   Bit 5: ACK_BG_CAL, Bit 6: ACK_ZERO_CAL
--   0x0001  Special Command   WO  0x7C06 = background cal, 0x7C07 = zero cal
--   0x001F  ABC Period        R/W Auto Baseline Correction interval (hours; 0 = suspend)

local SLAVE_ADDR = 254
local POLL_MS = 2000
local last_poll_ms = 0

-- External Services Config
local MQTT_CONN_ID = 1
local MQTT_TOPIC = "cycbox/sensor/co2"

local INFLUX_URL = get_env("INFLUX_URL") or "http://localhost:8181"
local INFLUX_TOKEN = get_env("INFLUX_TOKEN") or "your-influx-token"
local INFLUX_DB = "cycbox"

local NTFY_TOPIC = "cycbox_alerts"
local DISCORD_WEBHOOK = get_env("DISCORD_WEBHOOK") or "https://discord.com/api/webhooks/your_webhook_id/your_webhook_token"

local CO2_THRESHOLD_HIGH = 1000
local CO2_THRESHOLD_LOW = 700
local is_co2_high = false

function on_start()
    log("info", "Starting Senseair S8 Modbus script with MQTT, InfluxDB and Alerts.")
    -- Query device info on startup: 6 registers starting at 0x0019
    modbus_rtu_read_input_registers(SLAVE_ADDR, 0x0019, 6, 100, 0)
end

function on_timer(now_ms)
    -- Poll for sensor values every POLL_MS (2 seconds)
    if now_ms - last_poll_ms >= POLL_MS then
        -- Query sensor active data: 4 registers starting at 0x0000
        modbus_rtu_read_input_registers(SLAVE_ADDR, 0x0000, 4, 0, 0)
        last_poll_ms = now_ms
    end
end

function on_receive()
    if message.connection_id ~= 0 then return false end
    local modified = false

    -- 1. Parse active sensor values
    local co2 = message:get_value(string.format("modbus_rtu_%d:input_0003", SLAVE_ADDR))
    if co2 then
        message:add_int_value("CO2_ppm", co2)
        modified = true
        
        -- MQTT Publish
        local mqtt_payload = string.format('{"co2_ppm": %d}', co2)
        mqtt_publish(MQTT_TOPIC, mqtt_payload, 0, false, 0, MQTT_CONN_ID)
        
        -- InfluxDB v3 Publish
        local line_data = string.format("senseair_s8 co2_ppm=%d", co2)
        influxdb_write_v3_async(INFLUX_URL, INFLUX_TOKEN, INFLUX_DB, line_data, "auto", true, false)
        
        -- Alerts Logic
        if co2 > CO2_THRESHOLD_HIGH and not is_co2_high then
            is_co2_high = true
            local alert_msg = string.format("High CO2 Alert! Level at %d ppm", co2)
            log("warn", alert_msg)
            ntfy_send_async({topic = NTFY_TOPIC, message = alert_msg, title = "CO2 Alert", priority = "high"})
            discord_send_async(DISCORD_WEBHOOK, alert_msg)
        elseif co2 <= CO2_THRESHOLD_LOW and is_co2_high then
            is_co2_high = false
            local recovery_msg = string.format("CO2 returned to normal. Level at %d ppm", co2)
            log("info", recovery_msg)
            ntfy_send_async({topic = NTFY_TOPIC, message = recovery_msg, title = "CO2 Normal", priority = "default"})
            discord_send_async(DISCORD_WEBHOOK, recovery_msg)
        end
    end

    local status = message:get_value(string.format("modbus_rtu_%d:input_0000", SLAVE_ADDR))
    if status then
        message:add_int_value("MeterStatus", status)
        if status ~= 0 then
            local fatal = bit.band(status, 0x01)
            local out_of_range = bit.band(status, 0x20)
            if fatal > 0 then log("error", "Sensor FATAL ERROR") end
            if out_of_range > 0 then log("warn", "Sensor OUT OF RANGE") end
        end
        modified = true
    end

    local out_status = message:get_value(string.format("modbus_rtu_%d:input_0002", SLAVE_ADDR))
    if out_status then
        local alarm = (bit.band(out_status, 0x01) > 0)
        message:add_bool_value("Alarm_Active", alarm)
        modified = true
    end

    -- 2. Parse startup device info
    local fw = message:get_value(string.format("modbus_rtu_%d:input_001C", SLAVE_ADDR))
    if fw then
        local main_ver = bit.rshift(fw, 8)
        local sub_ver = bit.band(fw, 0xFF)
        message:add_string_value("Firmware_Version", string.format("%d.%d", main_ver, sub_ver))
        modified = true
    end

    local id_hi = message:get_value(string.format("modbus_rtu_%d:input_001D", SLAVE_ADDR))
    local id_lo = message:get_value(string.format("modbus_rtu_%d:input_001E", SLAVE_ADDR))
    if id_hi and id_lo then
        local sensor_id = id_hi * 65536 + id_lo
        message:add_int_value("Sensor_ID", sensor_id)
        modified = true
    end
    
    local type_hi = message:get_value(string.format("modbus_rtu_%d:input_0019", SLAVE_ADDR))
    local type_lo = message:get_value(string.format("modbus_rtu_%d:input_001A", SLAVE_ADDR))
    if type_hi and type_lo then
        -- Type ID spans upper 16 bits in 0x0019, and lowest 8 bits in the high byte of 0x001A
        local type_id = type_hi * 256 + bit.rshift(type_lo, 8)
        message:add_int_value("Sensor_Type_ID", type_id)
        modified = true
    end

    return modified
end

--[[
{
  "version": "2.0.0",
  "name": "Senseair S8 CO2 Sensor Monitor",
  "description": "Connects a Senseair S8 CO2 sensor via Modbus RTU over serial port, an MQTT client, and an MQTT broker.",
  "configs": [
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "modbus_rtu_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "serial_port_transport": {
        "serial_port_transport_port": "/dev/ttyS0",
        "serial_port_transport_baud_rate": 9600,
        "serial_port_transport_data_bits": 8,
        "serial_port_transport_parity": "none",
        "serial_port_transport_stop_bits": "1",
        "serial_port_transport_flow_control": "none"
      },
      "modbus_rtu_codec": {
        "with_receive_timeout": 20
      }
    },
    {
      "app": {
        "app_transport": "mqtt_transport",
        "app_codec": "timeout_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "mqtt_transport": {
        "mqtt_transport_broker_url": "mqtt://localhost:1883",
        "mqtt_transport_client_id": "cycbox-287QZ5DI",
        "mqtt_transport_username": "",
        "mqtt_transport_password": "",
        "mqtt_transport_use_tls": false,
        "mqtt_transport_ca_path": "",
        "mqtt_transport_client_cert_path": "",
        "mqtt_transport_client_key_path": "",
        "mqtt_transport_subscribe_topics": "",
        "mqtt_transport_subscribe_qos": 0
      },
      "timeout_codec": {
        "with_receive_timeout": 100
      }
    },
    {
      "app": {
        "app_transport": "mqtt_server_transport",
        "app_codec": "timeout_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "mqtt_server_transport": {
        "mqtt_server_transport_bind_address": "0.0.0.0",
        "mqtt_server_transport_bind_port": 1883,
        "mqtt_server_transport_use_tls": false,
        "mqtt_server_transport_tls_ca_path": "",
        "mqtt_server_transport_tls_cert_path": "",
        "mqtt_server_transport_tls_key_path": "",
        "mqtt_server_transport_credentials": "",
        "mqtt_server_transport_topic_filter": "#"
      },
      "timeout_codec": {
        "with_receive_timeout": 100
      }
    }
  ]
}
]]
