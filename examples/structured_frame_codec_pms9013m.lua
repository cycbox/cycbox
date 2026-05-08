-- PMS9013M Particle Concentration Sensor Monitor
-- Connects to a PMS9013M sensor via Serial (9600 8N1) using a structured frame codec.
-- Parses 32-byte measurement frames for PM1.0/2.5/10 concentrations and particle counts,
-- and handles 8-byte command acknowledgment frames.
--
-- Device: PMS9013M (Laser Particulate Matter Sensor)
--   Frame: [0x42 0x4D] [Length (16-bit)] [Payload] [Checksum (16-bit sum)]
--   Measurement Payload (26 bytes): 6 concentration values (u16be), 6 count values (u16be), version (u8), error (u8).
--   ACK Payload (2 bytes): Command echo (u8), Data/Status (u8).
--

function on_receive()
    -- Only process messages from the Serial connection (ID 0)
    if message.connection_id ~= 0 then
        return false
    end

    -- The structured_frame_codec handles the 0x42 0x4D sync and sum16 checksum validation
    if not message.checksum_valid then
        log("warn", "PMS9013M: Checksum mismatch detected")
        return false
    end

    local payload = message.payload
    if not payload then return false end

    -- Distinguish between Measurement (26-byte payload) and ACK (2-byte payload) frames
    if #payload == 26 then
        -- Concentration (Standard Particles CF=1)
        local pm1_0_std  = read_u16_be(payload, 1)
        local pm2_5_std  = read_u16_be(payload, 3)
        local pm10_std   = read_u16_be(payload, 5)

        -- Concentration (Atmospheric Environment)
        local pm1_0_atm  = read_u16_be(payload, 7)
        local pm2_5_atm  = read_u16_be(payload, 9)
        local pm10_atm   = read_u16_be(payload, 11)

        -- Particle Counts (per 0.1 Liters)
        local count_0_3  = read_u16_be(payload, 13)
        local count_0_5  = read_u16_be(payload, 15)
        local count_1_0  = read_u16_be(payload, 17)
        local count_2_5  = read_u16_be(payload, 19)
        local count_5_0  = read_u16_be(payload, 21)
        local count_10   = read_u16_be(payload, 23)

        -- Metadata
        local version    = read_u8(payload, 25)
        local error_code = read_u8(payload, 26)

        -- Add values for UI visualization and logging
        message:add_int_value("pm1_0_std", pm1_0_std)
        message:add_int_value("pm2_5_std", pm2_5_std)
        message:add_int_value("pm10_std", pm10_std)
        message:add_int_value("pm1_0_atm", pm1_0_atm)
        message:add_int_value("pm2_5_atm", pm2_5_atm)
        message:add_int_value("pm10_atm", pm10_atm)
        message:add_int_value("count_0_3um", count_0_3)
        message:add_int_value("count_0_5um", count_0_5)
        message:add_int_value("count_1_0um", count_1_0)
        message:add_int_value("count_2_5um", count_2_5)
        message:add_int_value("count_5_0um", count_5_0)
        message:add_int_value("count_10um", count_10)
        message:add_int_value("version", version)
        message:add_int_value("error_code", error_code)

        message.highlighted = (error_code > 0)
        
        return true

    elseif #payload == 2 then
        -- Handle Acknowledgment frames (e.g. response to mode switch or sleep command)
        local cmd_echo = read_u8(payload, 1)
        local status   = read_u8(payload, 2)
        
        message:add_int_value("ack_cmd", cmd_echo)
        message:add_int_value("ack_status", status)
        
        log("info", string.format("PMS9013M ACK: Command 0x%02X, Status 0x%02X", cmd_echo, status))
        return true
    end

    return false
end


--[[
{
  "version": "2.0.1",
  "name": "PMS9013M Sensor Connection",
  "description": "PMS9013M PM2.5 sensor serial connection with structured frame codec for measurement, command, and ACK frames.",
  "configs": [
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "structured_frame_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "serial_port_transport": {
        "serial_port_transport_port": "/dev/ttyUSB0",
        "serial_port_transport_baud_rate": 9600,
        "serial_port_transport_data_bits": 8,
        "serial_port_transport_parity": "none",
        "serial_port_transport_stop_bits": "1",
        "serial_port_transport_flow_control": "none"
      },
      "structured_frame_codec": {
        "structured_frame_codec_schema": "{\n  \"schema_version\": 1,\n  \"variants\": [\n    {\n      \"name\": \"measurement\",\n      \"direction\": \"rx\",\n      \"description\": \"32-byte active/passive measurement frame (device -> host)\",\n      \"fields\": [\n        {\n          \"name\": \"header\",\n          \"kind\": \"const\",\n          \"role\": \"sync\",\n          \"bytes\": \"42 4d\",\n          \"size_adjust\": 0\n        },\n        {\n          \"name\": \"length\",\n          \"kind\": \"u16be\",\n          \"size_adjust\": 0,\n          \"description\": \"Remaining length (28 bytes)\"\n        },\n        {\n          \"name\": \"payload\",\n          \"kind\": \"bytes\",\n          \"role\": \"payload\",\n          \"size_from\": \"length\",\n          \"size_adjust\": -2,\n          \"description\": \"Concentration and particle count data\"\n        },\n        {\n          \"name\": \"checksum\",\n          \"kind\": \"u16be\",\n          \"role\": \"checksum\",\n          \"size_adjust\": 0,\n          \"algo\": \"sum16_be\",\n          \"scope\": [\n            \"header\",\n            \"length\",\n            \"payload\"\n          ],\n          \"description\": \"Sum of all preceding bytes\"\n        }\n      ]\n    },\n    {\n      \"name\": \"command\",\n      \"direction\": \"tx\",\n      \"description\": \"7-byte host command frame (host -> device)\",\n      \"fields\": [\n        {\n          \"name\": \"header\",\n          \"kind\": \"const\",\n          \"role\": \"sync\",\n          \"bytes\": \"42 4d\",\n          \"size_adjust\": 0\n        },\n        {\n          \"name\": \"cmd\",\n          \"kind\": \"u8\",\n          \"size_adjust\": 0,\n          \"description\": \"Command byte (e.g., 0xE2)\"\n        },\n        {\n          \"name\": \"datah\",\n          \"kind\": \"u8\",\n          \"size_adjust\": 0,\n          \"description\": \"Parameter high byte\"\n        },\n        {\n          \"name\": \"datal\",\n          \"kind\": \"u8\",\n          \"size_adjust\": 0,\n          \"description\": \"Parameter low byte\"\n        },\n        {\n          \"name\": \"checksum\",\n          \"kind\": \"u16be\",\n          \"role\": \"checksum\",\n          \"size_adjust\": 0,\n          \"algo\": \"sum16_be\",\n          \"scope\": [\n            \"header\",\n            \"cmd\",\n            \"datah\",\n            \"datal\"\n          ],\n          \"description\": \"Sum of all preceding bytes\"\n        }\n      ]\n    },\n    {\n      \"name\": \"ack\",\n      \"direction\": \"rx\",\n      \"description\": \"8-byte command acknowledgment frame (device -> host)\",\n      \"fields\": [\n        {\n          \"name\": \"header\",\n          \"kind\": \"const\",\n          \"role\": \"sync\",\n          \"bytes\": \"42 4d\",\n          \"size_adjust\": 0\n        },\n        {\n          \"name\": \"length\",\n          \"kind\": \"u16be\",\n          \"size_adjust\": 0,\n          \"description\": \"Remaining length (4 bytes)\"\n        },\n        {\n          \"name\": \"payload\",\n          \"kind\": \"bytes\",\n          \"role\": \"payload\",\n          \"size_from\": \"length\",\n          \"size_adjust\": -2,\n          \"description\": \"Command echo and status/mode\"\n        },\n        {\n          \"name\": \"checksum\",\n          \"kind\": \"u16be\",\n          \"role\": \"checksum\",\n          \"size_adjust\": 0,\n          \"algo\": \"sum16_be\",\n          \"scope\": [\n            \"header\",\n            \"length\",\n            \"payload\"\n          ],\n          \"description\": \"Sum of all preceding bytes\"\n        }\n      ]\n    }\n  ]\n}"
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "073JVV02",
      "name": "Mode Control",
      "inputs": [
        {
          "input_type": "structured_frame",
          "id": "071FHGBO",
          "name": "Set Active Mode (Default)",
          "connection_id": 0,
          "json_text": "{\n  \"cmd\": 225,\n  \"datah\": 0,\n  \"datal\": 1,\n  \"variant\": \"command\"\n}",
          "fields": {
            "cmd": 225,
            "datah": 0,
            "datal": 1,
            "variant": "command"
          }
        },
        {
          "input_type": "structured_frame",
          "id": "07OMI6AC",
          "name": "Set Passive Mode",
          "connection_id": 0,
          "json_text": "{\n  \"cmd\": 225,\n  \"datah\": 0,\n  \"datal\": 0,\n  \"variant\": \"command\"\n}",
          "fields": {
            "cmd": 225,
            "datah": 0,
            "datal": 0,
            "variant": "command"
          }
        },
        {
          "input_type": "structured_frame",
          "id": "07WNBVHB",
          "name": "Read (Passive Only)",
          "connection_id": 0,
          "json_text": "{\n  \"cmd\": 226,\n  \"datah\": 0,\n  \"datal\": 0,\n  \"variant\": \"command\"\n}",
          "fields": {
            "cmd": 226,
            "datah": 0,
            "datal": 0,
            "variant": "command"
          }
        }
      ]
    },
    {
      "id": "071JOYVD",
      "name": "Power Control",
      "inputs": [
        {
          "input_type": "structured_frame",
          "id": "07B75TO2",
          "name": "Sleep",
          "connection_id": 0,
          "json_text": "{\n  \"cmd\": 228,\n  \"datah\": 0,\n  \"datal\": 0,\n  \"variant\": \"command\"\n}",
          "fields": {
            "cmd": 228,
            "datah": 0,
            "datal": 0,
            "variant": "command"
          }
        },
        {
          "input_type": "structured_frame",
          "id": "07VDY1LJ",
          "name": "Wakeup",
          "connection_id": 0,
          "json_text": "{\n  \"cmd\": 228,\n  \"datah\": 0,\n  \"datal\": 1,\n  \"variant\": \"command\"\n}",
          "fields": {
            "cmd": 228,
            "datah": 0,
            "datal": 1,
            "variant": "command"
          }
        }
      ]
    },
    {
      "id": "076NV3PF",
      "name": "Calibration (TSI)",
      "inputs": [
        {
          "input_type": "structured_frame",
          "id": "07BY0JIY",
          "name": "Original Curve",
          "connection_id": 0,
          "json_text": "{\n  \"cmd\": 82,\n  \"datah\": 0,\n  \"datal\": 1,\n  \"variant\": \"command\"\n}",
          "fields": {
            "cmd": 82,
            "datah": 0,
            "datal": 1,
            "variant": "command"
          }
        },
        {
          "input_type": "structured_frame",
          "id": "07NOAN33",
          "name": "TSI CF=0.38",
          "connection_id": 0,
          "json_text": "{\n  \"cmd\": 82,\n  \"datah\": 0,\n  \"datal\": 2,\n  \"variant\": \"command\"\n}",
          "fields": {
            "cmd": 82,
            "datah": 0,
            "datal": 2,
            "variant": "command"
          }
        },
        {
          "input_type": "structured_frame",
          "id": "07NA05HF",
          "name": "TSI CF=1",
          "connection_id": 0,
          "json_text": "{\n  \"cmd\": 82,\n  \"datah\": 0,\n  \"datal\": 3,\n  \"variant\": \"command\"\n}",
          "fields": {
            "cmd": 82,
            "datah": 0,
            "datal": 3,
            "variant": "command"
          }
        }
      ]
    }
  ]
}
]]
