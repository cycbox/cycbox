-- PMS9103M Air Quality Sensor Parser
-- This script parses PMS9103M particulate matter sensor data
--
-- Frame format:
--   Prefix: 0x42 0x4D ("BM")
--   Length: 2 bytes (big-endian, length of payload + checksum)
--   Payload: 26 bytes of sensor data
--   Checksum: 2 bytes (Sum16 big-endian)
--
-- Payload structure (all u16 big-endian):
--   Bytes 0-1:   PM1.0 concentration (CF=1) in μg/m³
--   Bytes 2-3:   PM2.5 concentration (CF=1) in μg/m³
--   Bytes 4-5:   PM10 concentration (CF=1) in μg/m³
--   Bytes 6-7:   PM1.0 concentration (atmospheric) in μg/m³
--   Bytes 8-9:   PM2.5 concentration (atmospheric) in μg/m³
--   Bytes 10-11: PM10 concentration (atmospheric) in μg/m³
--   Bytes 12-13: Particles >0.3μm per 0.1L air
--   Bytes 14-15: Particles >0.5μm per 0.1L air
--   Bytes 16-17: Particles >1.0μm per 0.1L air
--   Bytes 18-19: Particles >2.5μm per 0.1L air
--   Bytes 20-21: Particles >5.0μm per 0.1L air
--   Bytes 22-23: Particles >10μm per 0.1L air

function on_receive()
    -- Access the payload directly as a field (not a method)
    local payload = message.payload

    -- PMS9103M payload should be 26 bytes
    if #payload ~= 26 then
        log("warn", string.format("PMS9103M payload should be 26 bytes, got %d", #payload))
        return false
    end

    -- Parse PM concentrations (CF=1, standard particles) in μg/m³
    local pm1_0_cf1 = read_u16_be(payload, 1)   -- offset 1 = byte 0 in 0-indexed
    local pm2_5_cf1 = read_u16_be(payload, 3)
    local pm10_cf1 = read_u16_be(payload, 5)

    -- Parse PM concentrations (atmospheric environment) in μg/m³
    local pm1_0_atm = read_u16_be(payload, 7)
    local pm2_5_atm = read_u16_be(payload, 9)
    local pm10_atm = read_u16_be(payload, 11)

    -- Parse particle counts (number of particles per 0.1L air)
    local particles_0_3um = read_u16_be(payload, 13)
    local particles_0_5um = read_u16_be(payload, 15)
    local particles_1_0um = read_u16_be(payload, 17)
    local particles_2_5um = read_u16_be(payload, 19)
    local particles_5_0um = read_u16_be(payload, 21)
    local particles_10um = read_u16_be(payload, 23)

    -- Add PM1.0 values to chart (CF=1 vs Atmospheric)
    message:add_int_value("PM1.0-CF1", pm1_0_cf1)
    message:add_int_value("PM1.0-ATM", pm1_0_atm)

    -- Add PM2.5 values to chart (CF=1 vs Atmospheric)
    message:add_int_value("PM2.5-CF1", pm2_5_cf1)
    message:add_int_value("PM2.5-ATM", pm2_5_atm)

    -- Add PM10 values to chart (CF=1 vs Atmospheric)
    message:add_int_value("PM10-CF1", pm10_cf1)
    message:add_int_value("PM10-ATM", pm10_atm)

    -- Add particle counts to chart
    message:add_int_value("Particles >0.3μm", particles_0_3um)
    message:add_int_value("Particles >0.5μm", particles_0_5um)
    message:add_int_value("Particles >1.0μm", particles_1_0um)
    message:add_int_value("Particles >2.5μm", particles_2_5um)
    message:add_int_value("Particles >5.0μm", particles_5_0um)
    message:add_int_value("Particles >10μm", particles_10um)

    -- Log parsed data
    log("info", string.format(
        "PMS9103M | PM1.0: %d/%d | PM2.5: %d/%d | PM10: %d/%d (CF1/ATM μg/m³)",
        pm1_0_cf1, pm1_0_atm, pm2_5_cf1, pm2_5_atm, pm10_cf1, pm10_atm
    ))

    log("info", string.format(
        "Particles | >0.3μm: %d | >0.5μm: %d | >1.0μm: %d | >2.5μm: %d | >5.0μm: %d | >10μm: %d",
        particles_0_3um, particles_0_5um, particles_1_0um,
        particles_2_5um, particles_5_0um, particles_10um
    ))

    -- Return true because we added values to the message
    return true
end

-- Usage Notes:
-- 1. Enable Lua Script in CycBox settings
-- 2. Configure your transport (e.g., Serial Port with appropriate baud rate, typically 9600)
-- 3. Use Frame codec with the following settings:
--    - Prefix: 42 4D (hex for "BM")
--    - Length field: U16 Big-Endian
--    - Length meaning: Payload + Checksum
--    - Checksum: Sum16 Big-Endian
--    - Checksum scope: Prefix + Header + Length + Payload
-- 4. Paste this script into the Lua Script Code editor
-- 5. The script will automatically parse incoming PMS9103M frames and create charts for:
--    - PM1.0, PM2.5, PM10 concentrations (both CF=1 and Atmospheric)
--    - Particle counts for different size ranges
--
-- About PM measurements:
-- - CF=1: Concentration in standard particle environment (calibrated)
-- - ATM: Concentration in atmospheric environment (actual air quality)
-- - For outdoor air quality monitoring, use ATM values
-- - For indoor air quality or controlled environments, use CF=1 values
--
-- Lua API Reference (for this script):
-- Message fields:
--   message.payload       - Raw payload bytes (string)
--   message.frame         - Full frame including prefix/suffix (string)
--   message.timestamp     - Message timestamp in microseconds
--   message.connection_id - Source connection ID
-- Message methods:
--   message:add_int_value(id, value)    - Add integer value for charting
--   message:add_float_value(id, value)  - Add float value for charting
--   message:add_string_value(id, value) - Add string value
-- Binary read helpers (1-based indexing):
--   read_u8(bytes, offset)     - Read unsigned 8-bit integer
--   read_u16_be(bytes, offset) - Read unsigned 16-bit big-endian
--   read_u16_le(bytes, offset) - Read unsigned 16-bit little-endian
--   read_u32_be(bytes, offset) - Read unsigned 32-bit big-endian
--   (also: read_i8, read_i16_be/le, read_i32_be/le, read_float_be/le, read_double_be/le)
-- Utility functions:
--   log(level, message) - Log a message ("debug", "info", "warn", "error")
