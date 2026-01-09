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
local POLL_INTERVAL = 1000    -- Poll every 2 seconds (2000ms)

-- ============================================================================
-- Global State
-- ============================================================================

local last_poll_time = 0      -- Track last poll time for timer

-- ============================================================================
-- Lifecycle Hooks
-- ============================================================================

-- Called once when engine starts
function on_start()
    log("info", "=== EID041 Modbus Sensor Script Started ===")
    log("info", string.format("Configuration: Slave=%d, Start=0x%04X, Registers=%d",
        SLAVE_ADDR, START_ADDR, NUM_REGISTERS))
    log("info", string.format("Poll interval: %dms", POLL_INTERVAL))
end

-- Called every 100ms for periodic tasks
function on_timer(elapsed_ms)
    -- Check if enough time has passed since last poll
    if elapsed_ms - last_poll_time >= POLL_INTERVAL then
        -- Send Modbus Read Input Registers request
        -- Function signature: modbus_read_input_registers(slave, start, qty, delay, connection_id)
        local success = modbus_read_input_registers(
            SLAVE_ADDR,      -- slave address
            START_ADDR,      -- starting register address
            NUM_REGISTERS,   -- number of registers to read
            0,               -- send immediately (0ms delay)
            nil              -- use default connection (can specify connection_id if multiple)
        )

        if success then
            log("debug", string.format(
                "Polling EID041: slave=%d, start=0x%04X, qty=%d",
                SLAVE_ADDR, START_ADDR, NUM_REGISTERS
            ))
        else
            log("warn", "Failed to send Modbus read request")
        end

        last_poll_time = elapsed_ms
    end
end

-- ============================================================================
-- Message Processing
-- ============================================================================

-- Called for each received message
-- Demonstrates both codec-parsed values and manual parsing
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

    -- Get temperature raw value (stored as unsigned by codec)
    local temp_int_raw = message:get_value(
        string.format("modbus_rtu_%d:input_%d", slave_addr, 30001 + START_ADDR)
    )

    -- Get humidity raw value
    local hum_int_raw = message:get_value(
        string.format("modbus_rtu_%d:input_%d", slave_addr, 30001 + START_ADDR + 1)
    )

    if temp_int_raw ~= nil and hum_int_raw ~= nil then

        local temp_int_value = temp_int_raw * 0.1

        local hum_int_value = hum_int_raw * 0.1

        log("info", string.format("Temperature (int16): %.1f°C (raw=%d)",
            temp_int_value, temp_int_raw))
        log("info", string.format("Humidity (uint16): %.1f%%RH (raw=%d)",
            hum_int_value, hum_int_raw))

    end



    -- ========================================================================
    -- Debug All Parsed Values (JSON)
    -- ========================================================================
    -- Access all codec-parsed values as JSON for debugging
    local values_json = message.values_json
    log("info", "All codec-parsed values: " .. values_json)

    -- Return true because we added values to the message
    return false
end

