-- PMS9103M Air Quality Sensor with File Logging
-- Parses PMS9103M sensor data and writes JSON values to a file in /tmp
-- File is created on start and properly closed on stop

local OUTPUT_FILE = "/tmp/pms9103m_data.jsonl"
local file_handle = nil

function on_start()
    -- Open file in write mode (creates new or truncates existing)
    file_handle = io.open(OUTPUT_FILE, "w")
    if not file_handle then
        log("error", "Failed to open output file: " .. OUTPUT_FILE)
        return
    end

    log("info", "Logging sensor data to: " .. OUTPUT_FILE)

    -- Write header comment
    file_handle:write("# PMS9103M Sensor Data Log\n")
    file_handle:write("# Format: JSON Lines (one JSON object per line)\n")
    file_handle:write("# Timestamp, PM concentrations in μg/m³\n")
    file_handle:flush()
end

function on_receive()
    if message.connection_id ~= 0 then
        return false
    end

    -- Check if file is open
    if not file_handle then
        log("error", "File handle not available")
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

    -- Add values to chart (same as Redis example)
    message:add_int_value("PM1.0-CF1", pm1_0)
    message:add_int_value("PM2.5-CF1", pm2_5)
    message:add_int_value("PM10-CF1", pm10)
    message:add_int_value("PM1.0-ATM", pm1_0_atm)
    message:add_int_value("PM2.5-ATM", pm2_5_atm)
    message:add_int_value("PM10-ATM", pm10_atm)

    -- Write JSON line to file
    local json_str = message.values_json
    file_handle:write(json_str .. "\n")
    file_handle:flush()  -- Ensure data is written immediately

    log("info", "Wrote: " .. json_str)
    return true
end

function on_stop()
    if file_handle then
        -- Flush any remaining data
        file_handle:flush()

        -- Close the file
        file_handle:close()

        log("info", "File closed: " .. OUTPUT_FILE)
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
