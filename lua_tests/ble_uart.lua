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
  "version": "2.2.1",
  "name": "BLE UART throughput test",
  "description": "BLE UART throughput test",
  "configs": [
    {
      "app": {
        "app_transport": "ble_transport",
        "app_codec": "line_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "ble_transport": {
        "ble_transport_device": "hci0/dev_60_55_F9_F6_98_42",
        "ble_transport_service_uuid": "6e400001-b5a3-f393-e0a9-e50e24dcca9e",
        "ble_transport_mtu_payload": 244
      },
      "line_codec": {
        "line_codec_line_ending": "lf"
      }
    },
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "line_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "serial_port_transport": {
        "serial_port_transport_port": "/dev/ttyUSB0",
        "serial_port_transport_baud_rate": 921600,
        "serial_port_transport_data_bits": 8,
        "serial_port_transport_parity": "none",
        "serial_port_transport_stop_bits": "1",
        "serial_port_transport_flow_control": "none"
      },
      "line_codec": {
        "line_codec_line_ending": "lf"
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "14WPMTZF",
      "name": "GroupMTZF",
      "inputs": [
        {
          "input_type": "batch",
          "id": "144YNQ96",
          "name": "65kB/s",
          "items": [
            {
              "message_input": {
                "input_type": "simple",
                "id": "148MS5I6",
                "name": "Message",
                "raw_value": "456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678",
                "is_hex": false,
                "connection_id": 1
              },
              "delay_ms": 15.0
            }
          ],
          "repeat": true
        },
        {
          "input_type": "batch",
          "id": "15FPXRAI",
          "name": "88kB/s",
          "items": [
            {
              "message_input": {
                "input_type": "simple",
                "id": "157KTWKO",
                "name": "Message",
                "raw_value": "012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678",
                "is_hex": false,
                "connection_id": 1
              },
              "delay_ms": 15.0
            }
          ],
          "repeat": true
        }
      ]
    }
  ]
}
]]
