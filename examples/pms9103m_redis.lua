-- PMS9103M Air Quality Sensor with Redis Storage
-- Parses PMS9103M sensor data, stores PM2.5 (CF=1) to Redis on every reading,
-- and prints the latest value + rolling average every 10 seconds.
--
-- Redis keys:
--   pms9103m:pm25          latest PM2.5 (CF=1) in μg/m³
--   pms9103m:pm25:sum      running sum of all PM2.5 readings
--   pms9103m:pm25:count    total number of readings (reset with redis_del)

local REDIS_URL = get_env("REDIS_URL") or "redis://localhost:6379"
local REDIS_DATABASE = 0
local REDIS_USERNAME = nil  -- Set if using ACL
local REDIS_PASSWORD = nil  -- Set if using AUTH

local PRINT_INTERVAL = 10000  -- 10 seconds in ms
local timer_counter = 0

function on_start()
    local ok, err = redis_connect(REDIS_URL, REDIS_DATABASE, REDIS_USERNAME, REDIS_PASSWORD)
    if not ok then
        log("error", "Failed to connect to Redis: " .. (err or "unknown error"))
        return
    end
    log("info", "Connected to Redis. Keys: pms9103m:pm25, pms9103m:pm25:sum, pms9103m:pm25:count")
end

function on_receive()
    if message.connection_id ~= 0 then
        return false
    end

    local payload = message.payload
    if #payload ~= 26 then
        log("warn", string.format("PMS9103M payload should be 26 bytes, got %d", #payload))
        return false
    end

    -- Parse PM concentrations (CF=1, standard particles) in μg/m³
    local pm1_0 = read_u16_be(payload, 1)
    local pm2_5 = read_u16_be(payload, 3)
    local pm10  = read_u16_be(payload, 5)

    -- Parse PM concentrations (atmospheric environment) in μg/m³
    local pm1_0_atm = read_u16_be(payload, 7)
    local pm2_5_atm = read_u16_be(payload, 9)
    local pm10_atm  = read_u16_be(payload, 11)

    -- Add values to chart
    message:add_int_value("PM1.0-CF1", pm1_0)
    message:add_int_value("PM2.5-CF1", pm2_5)
    message:add_int_value("PM10-CF1", pm10)
    message:add_int_value("PM1.0-ATM", pm1_0_atm)
    message:add_int_value("PM2.5-ATM", pm2_5_atm)
    message:add_int_value("PM10-ATM", pm10_atm)

    -- Store latest PM2.5 value (fire-and-forget; only read by on_timer every 10s)
    redis_set_async("pms9103m:pm25", tostring(pm2_5), nil)

    -- Update running sum and count for average calculation
    local sum_str, _ = redis_get("pms9103m:pm25:sum")
    local count_str, _ = redis_get("pms9103m:pm25:count")
    local sum   = (tonumber(sum_str) or 0) + pm2_5
    local count = (tonumber(count_str) or 0) + 1
    redis_set("pms9103m:pm25:sum",   tostring(sum),   nil)
    redis_set("pms9103m:pm25:count", tostring(count), nil)

    log("info", message.values_json)
    return true
end

-- Called every 100ms; prints PM2.5 latest + average every 10 seconds
function on_timer(elapsed_ms)
    timer_counter = timer_counter + 100
    if timer_counter < PRINT_INTERVAL then
        return
    end
    timer_counter = 0

    local value, err = redis_get("pms9103m:pm25")
    if err then
        log("error", "redis_get pm25 failed: " .. err)
        return
    end
    if not value then
        log("info", "[PM2.5] no data yet")
        return
    end

    local sum_str, _   = redis_get("pms9103m:pm25:sum")
    local count_str, _ = redis_get("pms9103m:pm25:count")
    local count = tonumber(count_str) or 0

    local avg_str = "N/A"
    if count > 0 then
        avg_str = string.format("%.1f", (tonumber(sum_str) or 0) / count)
    end

    log("info", string.format("[PM2.5] latest=%s μg/m³  avg=%s μg/m³ (n=%d)", value, avg_str, count))
end

function on_stop()
    redis_del("pms9103m:pm25")
    redis_del("pms9103m:pm25:sum")
    redis_del("pms9103m:pm25:count")
    redis_disconnect()
    log("info", "Cleaned up Redis keys and disconnected")
end

--[[
id: "serial_assistant"
version: "1.10.0"
name: "Serial Assistant"
configs:
  - # Config 0
    app:
      app_transport: serial
      app_codec: frame_codec
      app_transformer: disable
      app_encoding: UTF-8
    serial:
      serial_port: /dev/ttyUSB0
      serial_baud_rate: 9600
      serial_data_bits: 8
      serial_parity: none
      serial_stop_bits: "1"
      serial_flow_control: none
    frame_codec:
      frame_codec_prefix: 42 4d
      frame_codec_header_size: 0
      frame_codec_tailer_length: 0
      frame_codec_suffix: ''
      frame_codec_length_mode: u16_be
      frame_codec_fixed_payload_size: 32
      frame_codec_length_meaning: payload_checksum
      frame_codec_checksum_algo: sum16_be
      frame_codec_checksum_scope: prefix_header_length_payload
message_input_groups:
  - key: "default"
    name: "Default"
    inputs:
      -
        type: single
        id: 39648271-f8dc-4114-b1ab-67354d3de995
        name: Message
        text: ''
        is_hex_mode: false
        auto_append: none
        connection_id: 0
]]
