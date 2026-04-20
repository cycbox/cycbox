-- EID041 Modbus Temperature & Humidity Sensor to MQTT Publisher
-- Polls EID041 sensor via Modbus RTU, parses temperature and humidity,
-- and publishes the values to MQTT topic "cycbox/eid041" in JSON format.
--
-- Connections:
--   Connection 0: Serial (Modbus RTU) - polls sensor registers
--   Connection 1: MQTT - publishes parsed values as JSON
--
-- MQTT JSON payload example:
--   {"temperature":25.3,"humidity":60.1,"temperature_float":25.32,"humidity_float":60.12}
--
-- Register Map (starting from 0x0000):
--   0x0000 (1 reg):  Temperature (int16, 0.1°C resolution, signed)
--   0x0001 (1 reg):  Humidity (uint16, 0.1%RH resolution)
--   0x0002-0x0003 (2 regs): Temperature (float32, big-endian, °C)
--   0x0004-0x0005 (2 regs): Humidity (float32, big-endian, %RH)

-- ============================================================================
-- Configuration
-- ============================================================================

local SLAVE_ADDR = 3          -- Modbus slave address
local START_ADDR = 0x0000     -- Start reading from register 0
local NUM_REGISTERS = 6       -- Read all 6 registers (2 int + 4 float)
local POLL_INTERVAL = 5000    -- Poll every 5 seconds (5000ms)
local MQTT_TOPIC = "cycbox/eid041"
local MQTT_CONNECTION_ID = 1  -- MQTT is the second connection (0-based)

-- Called once when engine starts
function on_start()
  log("info", "=== EID041 Modbus Sensor Script Started ===")
end

local last_poll_time = nil
-- Called every 100ms for periodic tasks
function on_timer(timestamp_ms)
  -- Check if enough time has passed since last poll
  if last_poll_time == nil or timestamp_ms - last_poll_time >= POLL_INTERVAL then
    -- modbus_read_input_registers(slave, start, qty, delay, connection_id)
    modbus_rtu_read_input_registers(SLAVE_ADDR, START_ADDR, NUM_REGISTERS, 0, 0)
    last_poll_time = timestamp_ms
  end
end


-- Called for each received message
function on_receive()
  local payload = message.payload

  -- Minimum Modbus response: Address(1) + Function(1) + ByteCount(1) = 3 bytes
  if #payload < 3 then
      return false
  end

  -- Extract Modbus fields
  local slave_addr = read_u8(payload, 1)
  local function_code = read_u8(payload, 2)
  local byte_count = read_u8(payload, 3)

  -- Only process Read Input Registers responses (0x04) from our device
  if function_code ~= 0x04 or slave_addr ~= SLAVE_ADDR then
      return false
  end

  -- ========================================================================
  -- Use Codec-Parsed Values
  -- ========================================================================
  -- The Modbus RTU codec automatically parses responses and creates two keys per register:
  --   Logical key:        modbus_rtu_{slave}:{30001 + addr}     (Modicon convention)
  --   Protocol-type key:  modbus_rtu_{slave}:input_{addr:04X}   (hex address)
  --
  -- For EID041 starting at register 0x0000:
  --   Register 0x0000 → modbus_rtu_3:30001 / modbus_rtu_3:input_0000 (Temperature int16)
  --   Register 0x0001 → modbus_rtu_3:30002 / modbus_rtu_3:input_0001 (Humidity uint16)
  --   Register 0x0002 → modbus_rtu_3:30003 / modbus_rtu_3:input_0002 (Temperature float32 high word)
  --   Register 0x0003 → modbus_rtu_3:30004 / modbus_rtu_3:input_0003 (Temperature float32 low word)
  --   etc.

  local temperature_int_raw = message:get_value(string.format("modbus_rtu_%d:%d", slave_addr, 30001 + START_ADDR))
  local humidity_int_raw = message:get_value(string.format("modbus_rtu_%d:%d", slave_addr, 30001 + START_ADDR + 1))

  local json_parts = {}
  local temperature_float_value = nil
  local humidity_float_value = nil

  if temperature_int_raw then
    local temperature_int_value = temperature_int_raw * 0.1
    message:add_float_value("temperature_int", temperature_int_value)
    table.insert(json_parts, string.format('"temperature":%.1f', temperature_int_value))
  end
  if humidity_int_raw then
    local humidity_int_value = humidity_int_raw * 0.1
    message:add_float_value("humidity_int", humidity_int_value)
    table.insert(json_parts, string.format('"humidity":%.1f', humidity_int_value))
  end

  if #payload >= 15 then
    temperature_float_value = read_float_be(payload, 8)
    if temperature_float_value then
      message:add_float_value("temperature_float", temperature_float_value)
      table.insert(json_parts, string.format('"temperature_float":%.2f', temperature_float_value))
    end
    humidity_float_value = read_float_be(payload, 12)
    if humidity_float_value then
      message:add_float_value("humidity_float", humidity_float_value)
      table.insert(json_parts, string.format('"humidity_float":%.2f', humidity_float_value))
    end
  end

  -- Publish parsed values to MQTT in JSON format
  if #json_parts > 0 then
    local json_payload = "{" .. table.concat(json_parts, ",") .. "}"
    mqtt_publish(MQTT_TOPIC, json_payload, 0, false, 0, MQTT_CONNECTION_ID)
    log("info", "Published to " .. MQTT_TOPIC .. ": " .. json_payload)
  end

  return true
end

--[[
{
  "version": "2.0.0",
  "name": "EID041 Modbus RTU to MQTT",
  "description": "Poll EID041 temperature & humidity sensor via Modbus RTU and publish to MQTT in JSON format",
  "configs": [
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "modbus_rtu_codec",
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
      "modbus_rtu_codec": {
        "with_receive_timeout": 20
      }
    },
    {
      "app": {
        "app_transport": "mqtt_transport",
        "app_codec": "timeout_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "mqtt_transport": {
        "mqtt_transport_broker_url": "mqtt://broker.emqx.io:1883",
        "mqtt_transport_client_id": "cycbox-eid041",
        "mqtt_transport_username": "",
        "mqtt_transport_password": "",
        "mqtt_transport_use_tls": false,
        "mqtt_transport_ca_path": "",
        "mqtt_transport_client_cert_path": "",
        "mqtt_transport_client_key_path": "",
        "mqtt_transport_subscribe_topics": "cycbox/#",
        "mqtt_transport_subscribe_qos": 1
      },
      "timeout_codec": {
        "with_receive_timeout": 100
      }
    }
  ]
}
]]
