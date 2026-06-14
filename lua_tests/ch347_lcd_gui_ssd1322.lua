-- LCD GUI prototype (SSD1322, 256x64 4-bit grayscale OLED) over CH347.
-- GPIO6 = DC, GPIO7 = RST, SPI CS0.
--
-- Demonstrates the lcd-gui layer on a grayscale panel: draw with
-- embedded-graphics primitives from Lua into an in-RAM framebuffer, then
-- `:flush()` rasterizes the whole frame into one `lcd_op` message. The
-- controller init sequence runs automatically on the first flush -- no manual
-- command tables, no DC toggling, no chunking.
--
-- The SSD1322 has 16 gray levels: each color's Rec.601 luma is quantized, so
-- colors collapse onto a gray ramp. Pick brightness with a hex 0xRRGGBB, a
-- {r,g,b} table, or a name ("white", "gray", "black"); only luminance matters.

function on_start()
    log("info", "LCD GUI ssd1322 demo...")

    local d = display_new {
        driver = "ssd1322",
        w = 256, h = 64,
        x_offset = 112, -- typical 256x64 module starts at controller column 0x1C
        cs = 0, dc = 6, rst = 7,
    }

    -- Start from a clean black frame.
    d:clear("black")

    -- Title bar: filled rect + text in a larger font.
    d:rect(0, 0, 256, 14, 0x404040, true)
    d:text(60, 2, "CycBox SSD1322 GUI", "white", "6x10")

    -- Rectangles: thick-outlined, filled, and an equal-sided square.
    d:rect(4, 18, 38, 26, "white", false, 2)       -- outlined (2px stroke)
    d:rect(10, 24, 26, 14, { 128, 128, 128 }, true) -- mid-gray fill via {r,g,b}
    d:square(48, 18, 26, 0xA0A0A0, false, 2)        -- equal-sided rect

    -- A filled rounded rectangle (a dim gray).
    d:rounded_rect(80, 18, 44, 26, 7, 0x404040, true)

    -- Circles (outlined + filled) and an ellipse.
    d:circle(146, 31, 12, "white", false, 2)
    d:circle(146, 31, 5, 0xC0C0C0, true)            -- bright inner dot
    d:ellipse(206, 28, 56, 22, 0x909090, false, 2)

    -- Arc (outline) and sector (pie slice); both centered, angles in degrees.
    d:arc(176, 31, 22, 0, 270, "white", 2)
    d:sector(232, 31, 22, 30, 150, 0xB0B0B0, true)

    -- A filled triangle plus thin and thick lines in the lower band.
    d:triangle(4, 62, 4, 48, 28, 56, 0x808080, true)
    d:line(34, 50, 120, 50, "white", 1)             -- thin separator
    d:line(34, 62, 80, 52, 0xE0E0E0, 3)             -- thick diagonal

    -- A polyline strip (a tiny waveform), brightening as it goes.
    d:polyline({
        { 86, 60 }, { 96, 50 }, { 106, 62 },
        { 116, 51 }, { 126, 60 }, { 136, 52 },
    }, 0xD0D0D0, 1)

    -- Individual pixels (a dotted accent row).
    for x = 86, 140, 6 do
        d:pixel(x, 46, "white")
    end

    -- A raw 4-bpp grayscale image: a left->right gray ramp (level 0..15) built
    -- on the fly. Native format is 4 bits/pixel, MSB-first -- two pixels per
    -- byte, the left pixel in the high nibble. Height is derived from length.
    local w, h = 32, 14
    local rows = {}
    for _ = 1, h do
        local row = {}
        for x = 0, w - 1, 2 do
            local hi = math.floor(x / (w - 1) * 15)
            local lo = math.floor((x + 1) / (w - 1) * 15)
            row[#row + 1] = string.char(hi * 16 + lo)
        end
        rows[#rows + 1] = table.concat(row)
    end
    d:image(148, 46, w, table.concat(rows))
    d:flush()
    log("info", "Frame flushed to panel.")
end

function on_receive()
    return false
end

--[[
{
  "version": "2.2.1",
  "name": "CH347 LCD GUI (SSD1322)",
  "description": "CH347 SPI connection driving a 256x64 SSD1322 grayscale OLED on CS0 via the lcd-gui display API (Passthrough codec for raw command/data).",
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
