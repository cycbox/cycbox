-- CH347 I2C EEPROM (AT24Cxx) reader / programmer.
--
-- Demonstrates the `eeprom_new` API: read (and, on demand, program + verify) a
-- 24-series I2C EEPROM over the CH347 bridge. Geometry is resolved from the Lua
-- chip table below — there is no hard-coded chip database in Rust — so adding a
-- part is a one-line edit here.
--
-- I2C EEPROMs have no JEDEC id, so you must pick the part by name. Set CHIP below.
-- Wiring: AT24 on I2C, device address 0x50 (A2/A1/A0 grounded).
--
-- By default this script is READ-ONLY: on start it dumps the first 64 bytes.
-- `write_demo()` (a destructive program + verify round-trip) is left for you to
-- call deliberately (see on_start).

local CHIP    = "AT24C256"   -- which part is wired up
local DEV     = 0x50         -- I2C device address
local CONN_ID = 0

-- ── Chip table ───────────────────────────────────────────────────────────────
-- `capacity` (bytes), `page` (write granularity), `addr_bytes` (word-address
-- width: 1 for ≤2 Kbit parts, 2 for AT24C32 and larger).
--
-- NOTE on AT24C04/08/16: these pack the high address bits into the device-address
-- LSBs (block select), which this transport does not handle — only their first
-- 256 bytes are addressable here. The 1-byte (≤AT24C02) and 2-byte (≥AT24C32)
-- parts are fully supported.
local EEPROM_CHIPS = {
    AT24C01  = { capacity =    128, page =   8, addr_bytes = 1 },
    AT24C02  = { capacity =    256, page =   8, addr_bytes = 1 },
    AT24C04  = { capacity =    512, page =  16, addr_bytes = 1 }, -- block-select, see NOTE
    AT24C08  = { capacity =   1024, page =  16, addr_bytes = 1 }, -- block-select, see NOTE
    AT24C16  = { capacity =   2048, page =  16, addr_bytes = 1 }, -- block-select, see NOTE
    AT24C32  = { capacity =   4096, page =  32, addr_bytes = 2 },
    AT24C64  = { capacity =   8192, page =  32, addr_bytes = 2 },
    AT24C128 = { capacity =  16384, page =  64, addr_bytes = 2 },
    AT24C256 = { capacity =  32768, page =  64, addr_bytes = 2 },
    AT24C512 = { capacity =  65536, page = 128, addr_bytes = 2 },
    AT24CM01 = { capacity = 131072, page = 256, addr_bytes = 2 }, -- 1 Mbit (block bit beyond 64 KB)
}

-- Open a chip handle from a preset.
local function eeprom_open(name, opts)
    local chip = EEPROM_CHIPS[name]
    if not chip then
        log("error", "Unknown EEPROM part: " .. tostring(name))
        return nil
    end
    opts = opts or {}
    return eeprom_new {
        dev = opts.dev or DEV,
        page = chip.page,
        addr_bytes = chip.addr_bytes,
        chunk = opts.chunk or 4096,
        -- write_ms = 0,  -- uncomment to ACK-poll write cycles (faster bulk writes)
        connection_id = opts.connection_id or CONN_ID,
    }, chip
end

local eeprom, chip_info

function on_start()
    eeprom, chip_info = eeprom_open(CHIP, { dev = DEV, connection_id = CONN_ID })
    if not eeprom then return end
    log("info", string.format("%s at 0x%02X — %d bytes, page %d, %d-byte addr",
        CHIP, DEV, chip_info.capacity, chip_info.page, chip_info.addr_bytes))
    -- Non-destructive: dump the first 64 bytes.
    eeprom:read(0x0000, 64)
    -- To program, uncomment:
    -- write_demo()
end

-- Destructive demo: write a known pattern at 0x0000 and verify the read-back.
function write_demo()
    if not eeprom then return end
    local data = "Hello from CycBox! " .. os.date("%Y-%m-%d %H:%M:%S")
    log("warn", "Writing " .. #data .. " bytes at 0x0000 (program + verify)...")
    eeprom:program(0x0000, data, { verify = true })
end

function on_receive()
    local op = message:get_metadata("eeprom_op")
    if not op then return false end

    if op == "read_chunk" then
        local addr = tonumber(message:get_metadata("eeprom_addr")) or 0
        local p = message.payload or ""
        -- Show the bytes as text where printable; the engine also hex-views them.
        log("info", string.format("read @ 0x%04X: %d bytes [%s]", addr, #p,
            (p:gsub("[^\32-\126]", "."))))
        return true

    elseif op == "progress" then
        local phase = message:get_metadata("eeprom_phase")
        local done  = tonumber(message:get_metadata("eeprom_done")) or 0
        local total = tonumber(message:get_metadata("eeprom_total")) or 0
        if total > 0 then
            log("info", string.format("%s: %d/%d (%d%%)", phase, done, total, math.floor(done * 100 / total)))
        end
        return false

    elseif op == "complete" then
        local phase  = message:get_metadata("eeprom_phase")
        local result = message:get_value("result")
        if result == "mismatch" then
            log("error", string.format("%s mismatch at 0x%X", phase,
                tonumber(message:get_value("mismatch_at")) or 0))
        else
            log("info", string.format("%s complete: %s", phase, result))
        end
        return true

    elseif op == "error" then
        log("error", "eeprom error: " .. (message:get_value("error") or "?"))
        return true
    end
    return false
end


--[[
{
  "version": "2.2.1",
  "name": "CH347 I2C EEPROM (AT24Cxx)",
  "description": "Read (and on demand program/verify) a 24-series I2C EEPROM over the CH347 bridge via the eeprom_new API. Read-only by default; pick the part by name.",
  "configs": [
    {
      "app": {
        "app_transport": "ch347_transport",
        "app_codec": "passthrough_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "ch347_transport": {
        "ch347_transport_device": "/dev/ch34x_pis0",
        "ch347_transport_i2c_speed": "2",
        "ch347_transport_spi_mode": "0",
        "ch347_transport_spi_clock": "4"
      }
    }
  ]
}
]]
