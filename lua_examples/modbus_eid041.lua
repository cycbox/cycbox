-- EID041 Modbus Temperature & Humidity Sensor Parser
-- This script parses EID041 Modbus RTU responses (Function Code 0x04)
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
--
-- Common read scenarios:
--   Read 2 registers from 0x0000: Temperature (int) + Humidity (int)
--   Read 6 registers from 0x0000: All data (int temp, int hum, float temp, float hum)
--   Read 4 registers from 0x0002: Float temperature + Float humidity

-- Parse Modbus RTU response and extract register data
function on_receive()
    local payload = message:get_payload()

    -- Minimum Modbus response: Address(1) + Function(1) + ByteCount(1) = 3 bytes
    if #payload < 3 then
        log("warn", string.format("Modbus frame too short: %d bytes", #payload))
        return false
    end

    -- Extract Modbus fields
    local slave_addr = read_u8(payload, 1)
    local function_code = read_u8(payload, 2)
    local byte_count = read_u8(payload, 3)

    -- Verify function code (should be 0x04 for Read Input Registers)
    if function_code ~= 0x04 then
        log("debug", string.format("Not a Read Input Registers response (FC=0x%02X)", function_code))
        return false
    end

    -- Calculate expected frame length: Address(1) + Function(1) + ByteCount(1) + Data(N)
    local expected_length = 3 + byte_count
    if #payload < expected_length then
        log("warn", string.format("Incomplete frame: expected %d bytes, got %d", expected_length, #payload))
        return false
    end

    -- Calculate number of registers (2 bytes per register)
    local num_registers = byte_count / 2

    if num_registers < 1 then
        log("warn", "No register data in response")
        return false
    end

    -- Data starts at offset 4 (after Address, Function, ByteCount)
    local data_offset = 4

    -- Variables to store parsed values
    local temp_int = nil      -- Temperature as int16 (0.1°C)
    local hum_int = nil       -- Humidity as uint16 (0.1%RH)
    local temp_float = nil    -- Temperature as float32 (°C)
    local hum_float = nil     -- Humidity as float32 (%RH)

    -- Parse based on number of registers read
    -- We assume reading starts from address 0x0000

    if num_registers >= 1 then
        -- Register 0x0000: Temperature (int16, signed, 0.1°C resolution)
        local temp_raw = read_i16_be(payload, data_offset)
        temp_int = temp_raw * 0.1

        log("info", string.format("Temperature (int16): %d -> %.1f°C", temp_raw, temp_int))
    end

    if num_registers >= 2 then
        -- Register 0x0001: Humidity (uint16, 0.1%RH resolution)
        local hum_raw = read_u16_be(payload, data_offset + 2)
        hum_int = hum_raw * 0.1

        log("info", string.format("Humidity (uint16): %d -> %.1f%%RH", hum_raw, hum_int))
    end

    if num_registers >= 4 then
        -- Registers 0x0002-0x0003: Temperature (float32, big-endian)
        temp_float = read_float_be(payload, data_offset + 4)

        log("info", string.format("Temperature (float32): %.2f°C", temp_float))
    end

    if num_registers >= 6 then
        -- Registers 0x0004-0x0005: Humidity (float32, big-endian)
        hum_float = read_float_be(payload, data_offset + 8)

        log("info", string.format("Humidity (float32): %.2f%%RH", hum_float))
    end

    -- Add values to charts
    -- Prefer float values if available, otherwise use int values
    if temp_int ~= nil then
        message:add_float_value("Temperature (°C)", temp_int)
    end

    if hum_int ~= nil then
        message:add_float_value("Humidity (%RH)", hum_int)
    end

    -- Return true because we added values to the message
    return true
end
