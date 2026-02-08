-- PMS9103M Air Quality Sensor with InfluxDB v3 (Cloud / OSS) Storage
-- Parses PMS9103M sensor data and writes to InfluxDB via the v3 /api/v3/write_lp endpoint.
--
-- Demonstrates three v3 helpers:
--   influxdb_write_v3_async       – real-time single-point fire-and-forget on every reading
--   influxdb_batch_write_v3_async – periodic non-blocking batch flush
--   influxdb_batch_write_v3       – sync batch flush at shutdown (durable, no_sync=false)
--
-- v3-specific options exercised:
--   accept_partial = true  → partial writes accepted (bad lines don't abort the batch)
--   no_sync        = true  → skip fsync for higher throughput (fine for telemetry)
--   no_sync        = false → forced on the final shutdown flush to avoid data loss
--
-- Line-protocol measurement: air_quality
--   tags:   sensor=pms9103m  type=cf1|atm
--   fields: pm1_0, pm2_5, pm10  (integer μg/m³)

local INFLUXDB_URL   = get_env("INFLUXDB_URL")   or "http://localhost:8181"
local INFLUXDB_TOKEN = get_env("INFLUXDB_TOKEN") or ""
local INFLUXDB_DB    = get_env("INFLUXDB_DB")    or "air-quality"

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
    log("info", string.format("InfluxDB v3 target: %s  db=%s", INFLUXDB_URL, INFLUXDB_DB))
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
    -- accept_partial=true, no_sync=true: optimise for latency, not durability.
    influxdb_write_v3_async(INFLUXDB_URL, INFLUXDB_TOKEN, INFLUXDB_DB,
        string.format("pm25_realtime,sensor=pms9103m value=%di", pm2_5),
        "auto", true, true)

    log("info", message.values_json)
    return true
end

-- Called every 100 ms; flushes pending_lines as an async batch every FLUSH_INTERVAL
function on_timer(elapsed_ms)
    timer_counter = timer_counter + 100
    if timer_counter < FLUSH_INTERVAL then
        return
    end
    timer_counter = 0

    if #pending_lines == 0 then
        return
    end

    -- Async batch write – non-blocking; network errors are logged by the engine.
    -- accept_partial=true, no_sync=true for throughput during normal operation.
    local ok = influxdb_batch_write_v3_async(
        INFLUXDB_URL, INFLUXDB_TOKEN, INFLUXDB_DB,
        pending_lines, "auto", true, true)

    if ok then
        log("info", string.format("[InfluxDB v3] async flush queued: %d lines", #pending_lines))
    else
        log("error", "[InfluxDB v3] async flush queuing failed")
    end
    pending_lines = {}
end

function on_stop()
    if #pending_lines == 0 then
        log("info", "InfluxDB v3 writer stopped (no pending data)")
        return
    end

    local line_count = #pending_lines

    -- Sync batch write – blocks until InfluxDB confirms receipt.
    -- no_sync=false forces a durable write on this final flush so the last
    -- batch isn't lost if the server restarts immediately after.
    -- Raises on failure, so wrap in pcall.
    local ok, result = pcall(influxdb_batch_write_v3,
        INFLUXDB_URL, INFLUXDB_TOKEN, INFLUXDB_DB,
        pending_lines, "auto", true, false)
    pending_lines = {}

    if not ok then
        log("error", "[InfluxDB v3] shutdown flush failed: " .. tostring(result))
        return
    end

    -- result is the HTTP status code (204 = written for v3)
    if result == 204 then
        log("info", string.format("[InfluxDB v3] shutdown flush: %d lines OK", line_count))
    else
        log("warn", string.format("[InfluxDB v3] shutdown flush: %d lines, HTTP %d", line_count, result))
    end
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
