-- PMS9103M Air Quality Sensor with InfluxDB v2 Storage
-- Parses PMS9103M sensor data and writes to InfluxDB via the v2 /api/v2/write endpoint.
--
-- Demonstrates three v2 helpers:
--   influxdb_write_v2_async       – real-time single-point fire-and-forget on every reading
--   influxdb_batch_write_v2       – periodic sync batch flush (HTTP status checked & logged)
--   influxdb_batch_write_v2_async – best-effort batch flush at shutdown
--
-- Line-protocol measurement: air_quality
--   tags:   sensor=pms9103m  type=cf1|atm
--   fields: pm1_0, pm2_5, pm10  (integer μg/m³)

local INFLUXDB_URL    = get_env("INFLUXDB_URL")    or "http://localhost:8086"
local INFLUXDB_TOKEN  = get_env("INFLUXDB_TOKEN")  or ""
local INFLUXDB_ORG    = get_env("INFLUXDB_ORG")    or "my-org"
local INFLUXDB_BUCKET = get_env("INFLUXDB_BUCKET") or "air-quality"

local FLUSH_INTERVAL = 10000  -- 10 seconds in ms
local timer_counter  = 0

-- Accumulated full readings; flushed as a batch every FLUSH_INTERVAL
local pending_lines = {}

local function format_line(tag_type, pm1, pm25, pm10)
    return string.format("air_quality,sensor=pms9103m,type=%s pm1_0=%di,pm2_5=%di,pm10=%di",
        tag_type, pm1, pm25, pm10)
end

function on_start()
    if INFLUXDB_TOKEN == "" then
        log("warn", "INFLUXDB_TOKEN is not set – writes will return 401")
    end
    log("info", string.format("InfluxDB v2 target: %s  org=%s  bucket=%s",
        INFLUXDB_URL, INFLUXDB_ORG, INFLUXDB_BUCKET))
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

    -- CF=1 (standard particles)
    local pm1_0     = read_u16_be(payload, 1)
    local pm2_5     = read_u16_be(payload, 3)
    local pm10      = read_u16_be(payload, 5)

    -- Atmospheric environment
    local pm1_0_atm = read_u16_be(payload, 7)
    local pm2_5_atm = read_u16_be(payload, 9)
    local pm10_atm  = read_u16_be(payload, 11)

    -- Chart values
    message:add_int_value("PM1.0-CF1",  pm1_0)
    message:add_int_value("PM2.5-CF1",  pm2_5)
    message:add_int_value("PM10-CF1",   pm10)
    message:add_int_value("PM1.0-ATM",  pm1_0_atm)
    message:add_int_value("PM2.5-ATM",  pm2_5_atm)
    message:add_int_value("PM10-ATM",   pm10_atm)

    -- Buffer both CF=1 and ATM lines for the next periodic batch flush
    pending_lines[#pending_lines + 1] = format_line("cf1", pm1_0, pm2_5, pm10)
    pending_lines[#pending_lines + 1] = format_line("atm", pm1_0_atm, pm2_5_atm, pm10_atm)

    -- Async single write – fire-and-forget latest PM2.5 for real-time dashboards.
    -- Returns true if queued; network errors are logged by the engine automatically.
    influxdb_write_v2_async(INFLUXDB_URL, INFLUXDB_TOKEN, INFLUXDB_ORG, INFLUXDB_BUCKET,
        string.format("pm25_realtime,sensor=pms9103m value=%di", pm2_5))

    log("info", message.values_json)
    return true
end

-- Called every 100 ms; flushes pending_lines as a sync batch every FLUSH_INTERVAL
function on_timer(elapsed_ms)
    timer_counter = timer_counter + 100
    if timer_counter < FLUSH_INTERVAL then
        return
    end
    timer_counter = 0

    if #pending_lines == 0 then
        return
    end

    local line_count = #pending_lines

    -- Sync batch write – blocks until InfluxDB responds so we can check the status code.
    -- Raises on network / validation failure, so wrap in pcall.
    local ok, result = pcall(influxdb_batch_write_v2,
        INFLUXDB_URL, INFLUXDB_TOKEN, INFLUXDB_ORG, INFLUXDB_BUCKET, pending_lines)
    pending_lines = {}  -- clear regardless to prevent unbounded growth

    if not ok then
        log("error", "[InfluxDB v2] batch flush failed: " .. tostring(result))
        return
    end

    -- result is the HTTP status code on success (204 = written)
    if result == 204 then
        log("info", string.format("[InfluxDB v2] flushed %d lines OK", line_count))
    else
        log("warn", string.format("[InfluxDB v2] flushed %d lines, HTTP %d", line_count, result))
    end
end

function on_stop()
    -- Best-effort async batch flush of any remaining buffered lines.
    -- The engine may shut down before the HTTP round-trip completes, but
    -- this gives InfluxDB a chance to persist the tail end of the data.
    if #pending_lines > 0 then
        local ok = influxdb_batch_write_v2_async(
            INFLUXDB_URL, INFLUXDB_TOKEN, INFLUXDB_ORG, INFLUXDB_BUCKET, pending_lines)
        if ok then
            log("info", string.format("[InfluxDB v2] shutdown flush queued: %d lines", #pending_lines))
        end
        pending_lines = {}
    end
    log("info", "InfluxDB v2 writer stopped")
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
