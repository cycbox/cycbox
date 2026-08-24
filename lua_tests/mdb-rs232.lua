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
  "version": "2.3.0",
  "name": "MDB-RS232",
  "description": "MDB-RS232 debugging for NAYAX",
  "configs": [
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "mdb_codec",
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
      "mdb_codec": {
        "mdb_codec_enrich": true,
        "mdb_codec_append_crlf": false
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "MDB_MAINTENANCE",
      "name": "MDB Maintenance",
      "inputs": [
        {
          "input_type": "simple",
          "id": "24EX0QOE",
          "name": "Reset",
          "raw_value": "10",
          "is_hex": true,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "24B4MRY5",
          "name": "Expansion: Request ID",
          "raw_value": "17004E454330303030303030303030303020202020204B5245412020200005",
          "is_hex": true,
          "connection_id": 0
        }
      ]
    },
    {
      "id": "MDB_CONFIG",
      "name": "MDB Configuration",
      "inputs": [
        {
          "input_type": "simple",
          "id": "24UVJYRX",
          "name": "Setup: Config Data (Lvl 3)",
          "raw_value": "11 00 03 10 02 01",
          "is_hex": true,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "24A2SHXC",
          "name": "Setup: Max/Min Prices",
          "raw_value": "11 01 FF FF 00 00",
          "is_hex": true,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "243GFWBT",
          "name": "Enable Always Idle",
          "raw_value": "17 04 00 00 00 20",
          "is_hex": true,
          "connection_id": 0
        }
      ]
    },
    {
      "id": "MDB_OPERATIONAL",
      "name": "MDB Operational Control",
      "inputs": [
        {
          "input_type": "simple",
          "id": "24EIL4K1",
          "name": "Reader Enable",
          "raw_value": "14 01",
          "is_hex": true,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "24OL3XZK",
          "name": "Reader Disable",
          "raw_value": "14 00",
          "is_hex": true,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "242HLEGY",
          "name": "Session Complete",
          "raw_value": "13 04",
          "is_hex": true,
          "connection_id": 0
        }
      ]
    },
    {
      "id": "MDB_VENDING",
      "name": "MDB Vending Sequence",
      "inputs": [
        {
          "input_type": "simple",
          "id": "24JXHHRD",
          "name": "Vend Request ($1.00)",
          "raw_value": "13 00 00 64 FF FF",
          "is_hex": true,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "24ZWN82V",
          "name": "Vend Success",
          "raw_value": "13 02 FF FF",
          "is_hex": true,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "24OR9J2V",
          "name": "Vend Failure",
          "raw_value": "13 03",
          "is_hex": true,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "24LMM66H",
          "name": "Vend Cancel",
          "raw_value": "13 01",
          "is_hex": true,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "24QXFO0T",
          "name": "Session Complete",
          "raw_value": "13 04",
          "is_hex": true,
          "connection_id": 0
        }
      ]
    }
  ]
}
]]
