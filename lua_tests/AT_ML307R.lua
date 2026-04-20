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
  "name": "AT",
  "description": "ML307R AT Commands",
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
        "serial_port_transport_baud_rate": 9600,
        "serial_port_transport_data_bits": 8,
        "serial_port_transport_parity": "none",
        "serial_port_transport_stop_bits": "1",
        "serial_port_transport_flow_control": "none"
      },
      "at_codec": {
        "at_codec_custom_urc_prefixes": "",
        "at_codec_custom_length_prefixed_urcs": ""
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "28BMVA8L",
      "name": "Basic",
      "inputs": [
        {
          "input_type": "simple",
          "id": "28JE8598",
          "name": "AT",
          "raw_value": "AT",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "batch",
          "id": "28CCXMX9",
          "name": "Device Info",
          "items": [
            {
              "message_input": {
                "input_type": "simple",
                "id": "28YNGVCJ",
                "name": "AT",
                "raw_value": "AT",
                "is_hex": false,
                "connection_id": 0
              },
              "delay_ms": 0.0
            },
            {
              "message_input": {
                "input_type": "simple",
                "id": "28DP3DUT",
                "name": "AT+CGMR",
                "raw_value": "AT+CGMR",
                "is_hex": false,
                "connection_id": 0
              },
              "delay_ms": 1000.0
            },
            {
              "message_input": {
                "input_type": "simple",
                "id": "28U5G84F",
                "name": "AT+CEREG?",
                "raw_value": "AT+CEREG?",
                "is_hex": false,
                "connection_id": 0
              },
              "delay_ms": 1000.0
            },
            {
              "message_input": {
                "input_type": "simple",
                "id": "28DVTU63",
                "name": "AT+CSQ",
                "raw_value": "AT+CSQ",
                "is_hex": false,
                "connection_id": 0
              },
              "delay_ms": 1000.0
            }
          ],
          "repeat": false
        }
      ]
    },
    {
      "id": "28OLRE1X",
      "name": "MQTT",
      "inputs": [
        {
          "input_type": "simple",
          "id": "28N57VTG",
          "name": "MQTT Info",
          "raw_value": "AT+MQTTCFG=?",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28AYWFL1",
          "name": "MQTT Connect",
          "raw_value": "AT+MQTTCONN=0,\"broker.emqx.io\",1883,\"bfdba077fee0\"",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "28GZ47C9",
          "name": "MQTT Pub",
          "raw_value": "AT+MQTTPUB=0,\"cycbox\",1,0,0,4,\"3242\"",
          "is_hex": false,
          "connection_id": 0
        },
        {
          "input_type": "simple",
          "id": "283HZYME",
          "name": "MQTT Sub",
          "raw_value": "AT+MQTTSUB=0,\"cycbox\",1",
          "is_hex": false,
          "connection_id": 0
        }
      ]
    }
  ]
}
]]
