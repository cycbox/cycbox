-- CycBox Lua Script
-- Documentation: https://cycbox.io/docs/lua-api/

-- Available hooks (uncomment to use):

-- function on_start()
--   -- Called once when engine starts
--   log("info", "Engine started")
-- end

-- function on_timer(timestamp_ms)
--   -- Called every 100ms with current timestamp in milliseconds
-- end

-- function on_receive()
--   -- Called for each received message
--   -- Access message fields: message.payload, message.connection_id
--   -- Return true if modified, false otherwise
--   return false
-- end

-- function on_send()
--   -- Called for each outgoing message (before encoding)
--   -- Modify message fields if needed
--   return false
-- end

-- function on_send_confirm()
--   -- Called after message is successfully sent
--   return false
-- end

-- function on_stop()
--   -- Called before engine stops or script is reloaded
-- end

--[[
{
  "version": "2.0.0",
  "name": "ESP32 AT",
  "description": "ESP32 AT Commands",
  "configs": [
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "at_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "serial_port_transport": {
        "serial_port_transport_port": "/dev/ttyUSB0",
        "serial_port_transport_baud_rate": 115200,
        "serial_port_transport_data_bits": 8,
        "serial_port_transport_parity": "none",
        "serial_port_transport_stop_bits": "1",
        "serial_port_transport_flow_control": "hardware"
      },
      "at_codec": {
        "at_codec_custom_urc_prefixes": "",
        "at_codec_custom_length_prefixed_urcs": ""
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "284L0DN7",
      "name": "Basic",
      "inputs": [
        {
          "input_type": "simple",
          "id": "28HVPJ3H",
          "name": "AT",
          "raw_value": "AT",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28YLZS3M",
          "name": "AT+GMR",
          "raw_value": "AT+GMR",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28W3CMPG",
          "name": "CMD",
          "raw_value": "AT+CMD?",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28QIJ9NM",
          "name": "SLEEP",
          "raw_value": "AT+SLEEP?",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "280WR2OJ",
          "name": "System Message",
          "raw_value": "AT+SYSMSG?",
          "is_hex": false,
          "connection_id": 0
        }
      ]
    },
    {
      "id": "28AQWGER",
      "name": "WIFI",
      "inputs": [
        {
          "input_type": "simple",
          "id": "28OBVGQ2",
          "name": "WIFI Init",
          "raw_value": "AT+CWINIT?",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "288QVXFM",
          "name": "Mode",
          "raw_value": "AT+CWMODE?",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "282M0CH9",
          "name": "Station Mode",
          "raw_value": "AT+CWMODE=1",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28WKDHGF",
          "name": "List AP",
          "raw_value": "AT+CWLAP",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28E2A68I",
          "name": "Connect WIFI",
          "raw_value": "AT+CWJAP=\"SSID\",\"PWD\"",
          "is_hex": false,
          "connection_id": 0
        }
      ]
    },
    {
      "id": "28T771B0",
      "name": "HTTP",
      "inputs": [
        {
          "input_type": "simple",
          "id": "284ZGO34",
          "name": "Message",
          "raw_value": "AT",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28MOOUS8",
          "name": "Message",
          "raw_value": "AT+HTTPCGET=\"https://cycbox.io/\"",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "285GRUDR",
          "name": "Message",
          "raw_value": "AT+HTTPCHEAD?",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28WP3F6H",
          "name": "Message",
          "raw_value": "AT+HTTPCHEAD=18",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28NZB8LT",
          "name": "Message",
          "raw_value": "Range: bytes=0-255",
          "is_hex": false,
          "connection_id": 0
        }
      ]
    }
  ]
}
]]
