-- PMS9103M Air Quality Sensor with TimescaleDB Storage
-- Parses PMS9103M sensor data and writes to TimescaleDB using dedicated insert helpers.
--
-- Demonstrates four helpers:
--   timescaledb_insert_async       – fire-and-forget single row insert
--   timescaledb_insert_batch_async – fire-and-forget batch insert
--   timescaledb_insert_batch       – blocking batch insert (durable, confirmed)
--
-- Assumed schema (run once before starting the script):
--
--   CREATE TABLE IF NOT EXISTS pm25_realtime (
--       time   TIMESTAMPTZ DEFAULT NOW(),
--       sensor TEXT        NOT NULL,
--       value  INTEGER     NOT NULL
--   );
--   SELECT create_hypertable('pm25_realtime', 'time', if_not_exists => TRUE);
--
--   CREATE TABLE IF NOT EXISTS air_quality (
--       time   TIMESTAMPTZ DEFAULT NOW(),
--       sensor TEXT        NOT NULL,
--       type   TEXT        NOT NULL,   -- 'cf1' or 'atm'
--       pm1_0  INTEGER     NOT NULL,
--       pm2_5  INTEGER     NOT NULL,
--       pm10   INTEGER     NOT NULL
--   );
--   SELECT create_hypertable('air_quality', 'time', if_not_exists => TRUE);
--
-- Tables:
--   pm25_realtime  – latest PM2.5 (CF=1), one row per sensor reading (real-time dashboards)
--   air_quality    – full PM1.0/2.5/10 for both CF=1 and ATM environments (batch history)

local CONNSTR       = get_env("TIMESCALEDB_CONNSTR") or "host=localhost port=5432 dbname=iotsensors user=postgres sslmode=disable"
local POOL_SIZE     = 5

local FLUSH_INTERVAL = 10000  -- 10 seconds in ms
local timer_counter  = 0

-- Accumulated full readings; flushed as a batch every FLUSH_INTERVAL
local pending_rows = {}

function on_start()
    local ok, err = timescaledb_connect(CONNSTR, POOL_SIZE)
    if not ok then
        log("error", "Failed to connect to TimescaleDB: " .. (err or "unknown error"))
        return
    end
    log("info", string.format("Connected to TimescaleDB (pool_size=%d)", POOL_SIZE))
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

    -- Buffer both CF=1 and ATM rows for the next periodic batch flush
    pending_rows[#pending_rows + 1] = {"pms9103m", "cf1", pm1_0,     pm2_5,     pm10}
    pending_rows[#pending_rows + 1] = {"pms9103m", "atm", pm1_0_atm, pm2_5_atm, pm10_atm}

    -- Async single insert – fire-and-forget latest PM2.5 for real-time dashboards.
    -- Returns immediately; connection / query errors are logged by the engine.
    timescaledb_insert_async("pm25_realtime", {"sensor", "value"}, {"pms9103m", pm2_5})

    log("info", message.values_json)
    return true
end

-- Called every 100 ms; flushes pending_rows as an async batch every FLUSH_INTERVAL
function on_timer(elapsed_ms)
    timer_counter = timer_counter + 100
    if timer_counter < FLUSH_INTERVAL then
        return
    end
    timer_counter = 0

    if #pending_rows == 0 then
        return
    end

    -- Async batch insert – non-blocking; errors are logged by the engine.
    local ok = timescaledb_insert_batch_async(
        "air_quality",
        {"sensor", "type", "pm1_0", "pm2_5", "pm10"},
        pending_rows
    )

    if ok then
        log("info", string.format("[TimescaleDB] async flush queued: %d rows", #pending_rows))
    else
        log("error", "[TimescaleDB] async flush queuing failed")
    end
    pending_rows = {}
end

function on_stop()
    if #pending_rows == 0 then
        log("info", "TimescaleDB writer stopped (no pending data)")
        timescaledb_disconnect()
        return
    end

    local row_count = #pending_rows

    -- Sync batch insert – blocks until PostgreSQL confirms receipt.
    -- Ensures the last batch isn't lost if the process exits immediately after.
    -- Raises on failure, so wrap in pcall.
    local ok, result = pcall(
        timescaledb_insert_batch,
        "air_quality",
        {"sensor", "type", "pm1_0", "pm2_5", "pm10"},
        pending_rows
    )
    pending_rows = {}

    if not ok then
        log("error", "[TimescaleDB] shutdown flush failed: " .. tostring(result))
    else
        log("info", string.format("[TimescaleDB] shutdown flush: %d rows inserted (affected=%d)", row_count, result))
    end

    timescaledb_disconnect()
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
