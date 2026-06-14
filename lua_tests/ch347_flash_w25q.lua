-- CH347 SPI NOR Flash (W25Q / GD25Q / MX25L) reader / programmer.
--
-- Demonstrates the `flash_new` API: identify the chip by its JEDEC id, then read
-- (and, on demand, erase + program + verify) a 25-series SPI NOR flash over the
-- CH347 USB bridge. Geometry is resolved from the Lua chip table below — there is
-- no hard-coded chip database in Rust — so adding a part is a one-line edit here.
--
-- Wiring: flash on SPI CS0. SPI mode 0, MSB first (standard for 25-series NOR).
--
-- By default this script is READ-ONLY: on start it reads the JEDEC id, identifies
-- the chip, and dumps the first 256 bytes. Erase/program are destructive and are
-- left as `program_demo()` for you to call deliberately (see on_start).

local CS = 0
local CONN_ID = 0

-- ── Chip table ───────────────────────────────────────────────────────────────
-- Keyed by part name. `jedec` is {manufacturer, mem_type, capacity_code} as
-- returned by RDID (0x9F). page/sector are the program/erase granularities;
-- addr_bytes is 3 for parts up to 16 MB, 4 beyond. Add parts freely.
local FLASH_CHIPS = {
    -- Winbond W25Q (mfr 0xEF)
    W25Q80   = { jedec = {0xEF, 0x40, 0x14}, capacity =  1*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    W25Q16   = { jedec = {0xEF, 0x40, 0x15}, capacity =  2*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    W25Q32   = { jedec = {0xEF, 0x40, 0x16}, capacity =  4*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    W25Q64   = { jedec = {0xEF, 0x40, 0x17}, capacity =  8*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    W25Q128  = { jedec = {0xEF, 0x40, 0x18}, capacity = 16*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    W25Q256  = { jedec = {0xEF, 0x40, 0x19}, capacity = 32*1024*1024, page = 256, sector = 4096, addr_bytes = 4 },
    W25Q512  = { jedec = {0xEF, 0x40, 0x20}, capacity = 64*1024*1024, page = 256, sector = 4096, addr_bytes = 4 },
    -- GigaDevice GD25Q (mfr 0xC8)
    GD25Q32  = { jedec = {0xC8, 0x40, 0x16}, capacity =  4*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    GD25Q64  = { jedec = {0xC8, 0x40, 0x17}, capacity =  8*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    GD25Q128 = { jedec = {0xC8, 0x40, 0x18}, capacity = 16*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    -- Macronix MX25L (mfr 0xC2)
    MX25L3206E  = { jedec = {0xC2, 0x20, 0x16}, capacity =  4*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    MX25L6406E  = { jedec = {0xC2, 0x20, 0x17}, capacity =  8*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
    MX25L12835F = { jedec = {0xC2, 0x20, 0x18}, capacity = 16*1024*1024, page = 256, sector = 4096, addr_bytes = 3 },
}

-- Identify a chip from its 3 JEDEC bytes: first by exact table match, then by a
-- generic fall-back that derives geometry from the standard capacity code (the
-- 3rd id byte: capacity = 2^code), which covers most unlisted 25-series parts.
local function identify(b1, b2, b3)
    for name, c in pairs(FLASH_CHIPS) do
        if c.jedec[1] == b1 and c.jedec[2] == b2 and c.jedec[3] == b3 then
            return name, c
        end
    end
    if b3 >= 0x10 and b3 <= 0x20 then
        local capacity = bit.lshift(1, b3)
        return string.format("unknown(%02X%02X%02X)", b1, b2, b3), {
            jedec = {b1, b2, b3}, capacity = capacity,
            page = 256, sector = 4096,
            addr_bytes = capacity > 16 * 1024 * 1024 and 4 or 3,
        }
    end
    return nil, nil
end

-- Open a chip handle from a preset (or a chip table returned by identify()).
local function flash_open(chip, opts)
    opts = opts or {}
    return flash_new {
        cs = opts.cs or CS,
        page = chip.page,
        sector = chip.sector,
        addr_bytes = chip.addr_bytes,
        chunk = opts.chunk or 4096,
        connection_id = opts.connection_id or CONN_ID,
    }
end

-- ── State ────────────────────────────────────────────────────────────────────
local flash       -- chip handle, set once the chip is identified
local chip_info   -- its geometry table

function on_start()
    log("info", "Reading JEDEC id on CS" .. CS .. "...")
    -- A bare handle is enough to read the id (geometry isn't needed for RDID).
    flash_new{ cs = CS, connection_id = CONN_ID }:id()
end

-- Destructive demo — call this from on_start (uncomment) to exercise the full
-- erase + program + verify path on the first sector. Programming auto-erases the
-- covered sectors and verifies the read-back by default.
local function program_demo()
    if not flash then
        log("warn", "program_demo: chip not identified yet")
        return
    end
    local data = string.rep("CycBox flash test \0\1\2\3", 12) -- a few hundred bytes
    log("warn", "Programming " .. #data .. " bytes at 0x000000 (erase + verify)...")
    flash:program(0x000000, data, { erase = true, verify = true })
end

function on_receive()
    local op = message:get_metadata("flash_op")
    if not op then return false end

    if op == "id_response" then
        local p = message.payload
        if not p or #p < 3 then return false end
        local b1, b2, b3 = string.byte(p, 1), string.byte(p, 2), string.byte(p, 3)
        local name, c = identify(b1, b2, b3)
        log("info", string.format("JEDEC id: %02X %02X %02X", b1, b2, b3))
        if not c then
            log("warn", "Unrecognized flash; supply geometry manually with flash_new{}")
            return true
        end
        chip_info = c
        flash = flash_open(c, { cs = CS, connection_id = CONN_ID })
        log("info", string.format("Detected %s — %d KB, page %d, sector %d, %d-byte addr",
            name, c.capacity / 1024, c.page, c.sector, c.addr_bytes))
        -- Non-destructive: dump the first 256 bytes.
        flash:read(0x000000, 256)
        -- To program, uncomment:
        -- program_demo()
        return true

    elseif op == "read_chunk" then
        local addr = tonumber(message:get_metadata("flash_addr")) or 0
        local p = message.payload or ""
        log("info", string.format("read @ 0x%06X: %d bytes", addr, #p))
        return true

    elseif op == "progress" then
        local phase = message:get_metadata("flash_phase")
        local done  = tonumber(message:get_metadata("flash_done")) or 0
        local total = tonumber(message:get_metadata("flash_total")) or 0
        if total > 0 then
            log("info", string.format("%s: %d/%d (%d%%)", phase, done, total, math.floor(done * 100 / total)))
        end
        return false

    elseif op == "complete" then
        local phase  = message:get_metadata("flash_phase")
        local result = message:get_value("result")
        if result == "mismatch" then
            log("error", string.format("%s mismatch at 0x%X", phase,
                tonumber(message:get_value("mismatch_at")) or 0))
        else
            log("info", string.format("%s complete: %s", phase, result))
        end
        return true

    elseif op == "error" then
        log("error", "flash error: " .. (message:get_value("error") or "?"))
        return true
    end
    return false
end


--[[
{
  "version": "2.2.1",
  "name": "CH347 SPI NOR Flash (W25Q/GD25Q/MX25L)",
  "description": "Identify a 25-series SPI NOR flash by JEDEC id over the CH347 bridge, then read (and on demand erase/program/verify) via the flash_new API. Read-only by default.",
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
