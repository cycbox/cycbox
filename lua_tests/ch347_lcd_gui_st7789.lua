-- LCD GUI prototype (ST7789V2, Waveshare 1.69" 240x280) over CH347.
-- GPIO6 = DC, GPIO7 = RST, SPI CS0.
--
-- Demonstrates the lcd-gui layer: draw with embedded-graphics primitives from
-- Lua into an in-RAM framebuffer, then `:flush()` rasterizes the whole frame
-- into one `lcd_op` message. The controller init sequence runs automatically on
-- the first flush -- no manual command tables, no DC toggling, no chunking.
--
-- Colors accept 0xRRGGBB, a name ("red"), or a {r,g,b} table.

function on_start()
    log("info", "LCD GUI st7789 demo...")

    local d = display_new {
        driver = "st7789",
        w = 240, h = 280,
        y_offset = 20, -- this glass is inset 20px in the row direction
        cs = 0, dc = 6, rst = 7,
    }

    -- Start from a clean black frame.
    d:clear("black")

    -- Title bar: filled rect + text in a larger font.
    d:rect(0, 0, 240, 28, 0x003366, true)
    d:text(8, 19, "CycBox LCD GUI", "white", "9x18bold")

    -- Rectangles: thick-outlined, filled, and a square.
    d:rect(8, 40, 70, 44, "red", false, 2)       -- outlined (2px stroke)
    d:rect(16, 48, 54, 28, 0x00AA00, true)       -- filled
    d:square(96, 40, 44, "magenta", false, 2)    -- equal-sided rect

    -- A filled rounded rectangle.
    d:rounded_rect(160, 40, 72, 44, 10, 0x884400, true)

    -- Circles (outlined + filled) and an ellipse.
    d:circle(40, 120, 22, "cyan", false, 2)
    d:circle(40, 120, 10, { 255, 128, 0 }, true) -- orange via {r,g,b} table
    d:ellipse(120, 120, 60, 36, "yellow", false, 2)

    -- Arc (outline) and sector (pie slice); both centered, angles in degrees.
    d:arc(196, 120, 44, 0, 270, "white", 3)
    d:sector(196, 178, 44, 30, 120, 0x00FF88, true)

    -- A filled triangle plus thin and thick lines.
    d:triangle(8, 150, 8, 210, 60, 180, "red", true)
    d:line(8, 220, 232, 220, "white", 1)         -- thin separator
    d:line(8, 230, 120, 260, "yellow", 3)        -- thick diagonal

    -- A polyline strip (a tiny waveform).
    d:polyline({
        { 130, 245 }, { 145, 230 }, { 160, 250 },
        { 175, 232 }, { 190, 248 }, { 205, 235 },
    }, "cyan", 2)

    -- Individual pixels (a dotted accent row).
    for x = 130, 230, 8 do
        d:pixel(x, 224, "white")
    end

    -- A 16x16 image blitted from raw big-endian RGB565 bytes: a red→blue
    -- horizontal gradient built on the fly (height is derived from length).
    local w, h = 16, 16
    local rows = {}
    for _ = 1, h do
        local row = {}
        for x = 0, w - 1 do
            local r5 = math.floor((1 - x / (w - 1)) * 31)
            local b5 = math.floor((x / (w - 1)) * 31)
            local px = r5 * 2048 + b5 -- r in bits 11-15, b in bits 0-4
            row[#row + 1] = string.char(math.floor(px / 256), px % 256)
        end
        rows[#rows + 1] = table.concat(row)
    end
    d:image(210, 232, w, table.concat(rows))

    -- A closing caption in the default font.
    d:text(8, 274, "all draw methods", 0xAAAAAA)

    d:flush()
    log("info", "Frame flushed to panel.")
end

function on_receive()
    return false
end

--[[
{
  "version": "2.2.1",
  "name": "CH347 LCD GUI (ST7789)",
  "description": "CH347 SPI connection driving a 1.69-inch ST7789 LCD on CS0 via the lcd-gui display API (Passthrough codec for raw command/data).",
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
