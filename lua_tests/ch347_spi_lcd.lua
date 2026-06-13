-- Waveshare 1.69" LCD Test (ST7789V2)
-- CH347 SPI transport at CS0, with GPIO0 for DC (Data/Command) and GPIO1 for RST (Reset).
-- Fixes an overflow error in coordinate byte calculation and fills the screen with color stripes.
--
-- Device: Waveshare 1.69inch LCD Module, 240x280 resolution.
-- Interface: SPI (Mode 0), DC=GPIO0, RST=GPIO1.
--
-- Register Map Summary (from ST7789V2 datasheet):
-- 0x36: MADCTL (Memory Data Access Control)
-- 0x3A: COLMOD (Interface Pixel Format)
-- 0x21: INVON (Display Inversion On)
-- 0x11: SLPOUT (Sleep Out)
-- 0x29: DISPON (Display On)
-- 0x2A/0x2B: CASET/RASET (Column/Row Address Set)
-- 0x2C: RAMWR (Memory Write)

local DC_PIN = 6
local RST_PIN = 7
local CS = 0

-- Helper to send a command byte
local function lcd_cmd(cmd, delay)
    gpio_write(DC_PIN, 0, 0, delay or 0)
    spi_write(CS, string.char(cmd), 0, delay or 0)
end

-- Helper to send data (single byte or string)
local function lcd_data(data, delay)
    gpio_write(DC_PIN, 1, 0, delay or 0)
    local payload = type(data) == "number" and string.char(data) or data
    spi_write(CS, payload, 0, delay or 0)
end

function on_start()
    log("info", "Starting LCD 1.69 test with coordinate fixes...")

    -- 1. Hardware Reset Sequence
    gpio_write(RST_PIN, 1, 0, 0)
    gpio_write(RST_PIN, 0, 0, 20)
    gpio_write(RST_PIN, 1, 0, 40)

    -- 2. Initialization sequence
    local t = 100
    
    lcd_cmd(0x36, t) lcd_data(0x00, t) t = t + 5 -- MADCTL
    lcd_cmd(0x3A, t) lcd_data(0x05, t) t = t + 5 -- COLMOD (16bit RGB565)
    
    lcd_cmd(0xB2, t) -- PORCTRL
    lcd_data(string.char(0x0B, 0x0B, 0x00, 0x33, 0x35), t) t = t + 5
    
    lcd_cmd(0xB7, t) lcd_data(0x11, t) t = t + 5 -- GCTRL
    lcd_cmd(0xBB, t) lcd_data(0x35, t) t = t + 5 -- VCOMS
    lcd_cmd(0xC0, t) lcd_data(0x2C, t) t = t + 5 -- LCMCTRL
    lcd_cmd(0xC2, t) lcd_data(0x01, t) t = t + 5 -- VDVVRHEN
    lcd_cmd(0xC3, t) lcd_data(0x0D, t) t = t + 5 -- VRHS
    lcd_cmd(0xC4, t) lcd_data(0x20, t) t = t + 5 -- VDVS
    lcd_cmd(0xC6, t) lcd_data(0x13, t) t = t + 5 -- FRCTRL2 (60Hz)
    
    lcd_cmd(0xD0, t) lcd_data(string.char(0xA4, 0xA1), t) t = t + 5 -- PWCTRL1
    lcd_cmd(0xD6, t) lcd_data(0xA1, t) t = t + 5 -- Unknown/Product specific
    
    -- Gamma Positive
    lcd_cmd(0xE0, t) 
    lcd_data(string.char(0xF0, 0x06, 0x0B, 0x0A, 0x09, 0x26, 0x29, 0x33, 0x41, 0x18, 0x16, 0x15, 0x29, 0x2D), t) t = t + 10
    
    -- Gamma Negative
    lcd_cmd(0xE1, t) 
    lcd_data(string.char(0xF0, 0x04, 0x08, 0x08, 0x07, 0x03, 0x28, 0x32, 0x40, 0x3B, 0x19, 0x18, 0x2A, 0x2E), t) t = t + 10
    
    lcd_cmd(0x21, t) t = t + 5 -- Display Inversion ON
    lcd_cmd(0x11, t) t = t + 120 -- Sleep Out
    lcd_cmd(0x29, t) t = t + 20  -- Display ON

    -- 3. Draw Color Test Bars
    -- Set window to full screen (240x280)
    -- Portrait offset Y starts at 20 per datasheet/Python driver for this specific glass
    local x_start, x_end = 0, 239
    local y_start, y_end = 20, 20 + 280 - 1 -- 299

    -- CASET: X range
    lcd_cmd(0x2A, t) 
    lcd_data(string.char(
        bit.band(bit.rshift(x_start, 8), 0xFF), bit.band(x_start, 0xFF), 
        bit.band(bit.rshift(x_end, 8), 0xFF), bit.band(x_end, 0xFF)
    ), t)
    
    -- RASET: Y range
    lcd_cmd(0x2B, t) 
    lcd_data(string.char(
        bit.band(bit.rshift(y_start, 8), 0xFF), bit.band(y_start, 0xFF), 
        bit.band(bit.rshift(y_end, 8), 0xFF), bit.band(y_end, 0xFF)
    ), t)
    
    lcd_cmd(0x2C, t) t = t + 5 -- RAMWR
    
    -- Draw three horizontal bands (Red, Green, Blue)
    -- Total pixels: 240 * 280 = 67,200. Each color ~22,400 pixels.
    local colors = {
        string.char(0xF8, 0x00), -- Red
        string.char(0x07, 0xE0), -- Green
        string.char(0x00, 0x1F)  -- Blue
    }

    gpio_write(DC_PIN, 1, 0, t)
    for i = 1, 3 do
        local chunk = string.rep(colors[i], 2240) -- 2240 pixels per chunk (staying within SPI buffer limits)
        for _ = 1, 10 do
            spi_write(CS, chunk, 0, t+1)
            t = t + 5
        end
    end
    
    log("info", "LCD test sequence with coordinate fixes queued.")
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
