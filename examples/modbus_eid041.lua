-- EID041 Modbus Temperature & Humidity Sensor Parser
-- This script demonstrates automatic polling and using codec-parsed values
--
-- Modbus RTU Frame format:
--   Slave Address: 1 byte
--   Function Code: 1 byte (0x04 - Read Input Registers)
--   Byte Count: 1 byte (number of data bytes)
--   Data: N bytes (register values)
--   CRC: 2 bytes (Modbus CRC16)
--
-- Register Map (starting from 0x0000):
--   0x0000 (1 reg):  Temperature (int16, 0.1°C resolution, signed)
--   0x0001 (1 reg):  Humidity (uint16, 0.1%RH resolution)
--   0x0002-0x0003 (2 regs): Temperature (float32, big-endian, °C)
--   0x0004-0x0005 (2 regs): Humidity (float32, big-endian, %RH)

-- ============================================================================
-- Configuration
-- ============================================================================

local SLAVE_ADDR = 1          -- Modbus slave address
local START_ADDR = 0x0000     -- Start reading from register 0
local NUM_REGISTERS = 6       -- Read all 6 registers (2 int + 4 float)
local POLL_INTERVAL = 5000    -- Poll every 5 seconds (5000ms)

-- Called once when engine starts
function on_start()
  log("info", "=== EID041 Modbus Sensor Script Started ===")
end

local last_poll_time = 0
-- Called every 100ms for periodic tasks
function on_timer(elapsed_ms)
  -- Check if enough time has passed since last poll
  if elapsed_ms - last_poll_time >= POLL_INTERVAL then
    -- modbus_read_input_registers(slave, start, qty, delay, connection_id)
    modbus_rtu_read_input_registers(SLAVE_ADDR, START_ADDR, NUM_REGISTERS, 0, 0)
    last_poll_time = elapsed_ms
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
  -- The Modbus RTU codec automatically parses responses and creates Value objects
  -- with IDs: "modbus_rtu_{slave}:input_{30001 + register_address}"
  --
  -- For EID041 starting at register 0x0000:
  --   Register 0x0000 → modbus_rtu_1:input_30001 (Temperature int16)
  --   Register 0x0001 → modbus_rtu_1:input_30002 (Humidity uint16)
  --   Register 0x0002 → modbus_rtu_1:input_30003 (Temperature float32 high word)
  --   Register 0x0003 → modbus_rtu_1:input_30004 (Temperature float32 low word)
  --   etc.

  local temperature_int_raw = message:get_value(string.format("modbus_rtu_%d:input_%d", slave_addr, 30001 + START_ADDR))
  local humidity_int_raw = message:get_value(string.format("modbus_rtu_%d:input_%d", slave_addr, 30001 + START_ADDR + 1))

  if temperature_int_raw then
    local temperature_int_value = temperature_int_raw * 0.1
    message:add_float_value("temperature_int", temperature_int_value)
  end
  if humidity_int_raw then
    local humidity_int_value = humidity_int_raw * 0.1
    message:add_float_value("humidity_int", humidity_int_value)
  end

  if #payload >= 15 then
    local temperature_float_value = read_float_be(payload, 8)
    if temperature_float_value then
      message:add_float_value("temperature_float", temperature_float_value)
    end
    local humidity_float_value = read_float_be(payload, 12)
    if humidity_float_value then
      message:add_float_value("humidity_float", humidity_float_value)
    end
  end

  -- Access all codec-parsed values as JSON for debugging
  local values_json = message.values_json
  log("info", values_json)

  -- Return true because we added values to the message
  return true
end

--[[
id: "serial_assistant"
version: "1.8.1"
name: "Serial Assistant"
configs:
  - # Config 0
    app:
      app_transport: serial
      app_codec: modbus_rtu_codec
      app_transformer: disable
      app_encoding: UTF-8
    serial:
      serial_port: /dev/ttyUSB0
      serial_baud_rate: 9600
      serial_data_bits: 8
      serial_parity: none
      serial_stop_bits: "1"
      serial_flow_control: none
    modbus_rtu_codec:
      with_receive_timeout: 20
message_input_groups:
  - key: "default"
    name: "Default"
    inputs:
      -
        type: modbus_rtu
        id: cb9d5168-9625-412e-ac6e-559066f311df
        name: Read All Data
        slave_address: 1
        function_code: read_input_registers
        start_address: 0
        quantity: 6
        data_value: ''
        connection_id: 0
        start_address_hex_mode: false
        data_value_hex_mode: true
]]
