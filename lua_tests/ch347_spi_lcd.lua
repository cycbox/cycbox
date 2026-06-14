-- Waveshare 1.69" LCD Test (ST7789V2)
-- CH347 SPI at CS0, GPIO6 = DC (Data/Command), GPIO7 = RST (Reset).
-- Fills the 240x280 screen with red / green / blue horizontal bands.
--
-- Uses the `lcd_new` builder: every :cmd / :data / :reset / :delay call appends
-- one segment to a program, and a single :flush() sends the whole thing as one
-- `lcd_op` message. The CH347 worker replays it atomically on its device thread,
-- toggling DC/RST, chunking the pixel blob and using real delays -- so there is
-- no manual DC toggling, no `t` timeline and no hand-rolled chunk loop.
--
-- Register map (ST7789V2): 0x36 MADCTL, 0x3A COLMOD, 0x21 INVON, 0x11 SLPOUT,
-- 0x29 DISPON, 0x2A/0x2B CASET/RASET, 0x2C RAMWR.

local function be16(v)
    return string.char(bit.band(bit.rshift(v, 8), 0xFF), bit.band(v, 0xFF))
end

function on_start()
    log("info", "Starting LCD 1.69 test (lcd_op)...")

    local lcd = lcd_new { cs = 0, dc = 6, rst = 7 }

    -- 1. Hardware reset: high 1ms, low 20ms, high 40ms.
    lcd:reset(1, 20, 40)

    -- 2. Init sequence ----------------------------------------------------
    lcd:cmd(0x36, 0x00)                                       -- MADCTL
    lcd:cmd(0x3A, 0x05)                                       -- COLMOD (RGB565)
    lcd:cmd(0xB2, { 0x0B, 0x0B, 0x00, 0x33, 0x35 })          -- PORCTRL
    lcd:cmd(0xB7, 0x11)                                       -- GCTRL
    lcd:cmd(0xBB, 0x35)                                       -- VCOMS
    lcd:cmd(0xC0, 0x2C)                                       -- LCMCTRL
    lcd:cmd(0xC2, 0x01)                                       -- VDVVRHEN
    lcd:cmd(0xC3, 0x0D)                                       -- VRHS
    lcd:cmd(0xC4, 0x20)                                       -- VDVS
    lcd:cmd(0xC6, 0x13)                                       -- FRCTRL2 (60Hz)
    lcd:cmd(0xD0, { 0xA4, 0xA1 })                            -- PWCTRL1
    lcd:cmd(0xD6, 0xA1)                                       -- product specific
    lcd:cmd(0xE0, { 0xF0, 0x06, 0x0B, 0x0A, 0x09, 0x26, 0x29, 0x33, 0x41, 0x18, 0x16, 0x15, 0x29, 0x2D }) -- gamma+
    lcd:cmd(0xE1, { 0xF0, 0x04, 0x08, 0x08, 0x07, 0x03, 0x28, 0x32, 0x40, 0x3B, 0x19, 0x18, 0x2A, 0x2E }) -- gamma-
    lcd:cmd(0x21)                                             -- INVON
    lcd:cmd(0x11) lcd:delay(120)                              -- SLPOUT + settle
    lcd:cmd(0x29) lcd:delay(20)                               -- DISPON

    -- 3. Window: full 240x280. This glass starts at Y offset 20.
    local x_start, x_end = 0, 239
    local y_start, y_end = 20, 20 + 280 - 1
    lcd:cmd(0x2A, be16(x_start) .. be16(x_end))               -- CASET
    lcd:cmd(0x2B, be16(y_start) .. be16(y_end))               -- RASET
    lcd:cmd(0x2C)                                             -- RAMWR

    -- 4. Three color bands (worker chunks this 134400-byte blob for us).
    local rows = math.floor(280 / 3)
    local red   = string.rep(string.char(0xF8, 0x00), 240 * rows)
    local green = string.rep(string.char(0x07, 0xE0), 240 * rows)
    local blue  = string.rep(string.char(0x00, 0x1F), 240 * (280 - 2 * rows))
    lcd:data(red)
    lcd:data(green)
    lcd:data(blue)

    lcd:flush()
    log("info", "LCD test program flushed.")
end

function on_receive()
    return false
end

--[[
{
  "version": "2.2.1",
  "name": "CH347 SPI LCD Connection",
  "description": "Configured CH347 SPI connection for the 1.69-inch LCD on CS0 using Passthrough codec for raw command/data transmission.",
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
        "ch347_transport_spi_clock": "3"
      }
    }
  ]
}
]]
