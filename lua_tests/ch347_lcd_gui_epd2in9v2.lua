-- LCD GUI demo on a Waveshare 2.9" e-paper V2 (B/W) over CH347.
-- GPIO6 = DC, GPIO7 = RST, SPI CS0. Panel: 128x296, SSD1680-class controller.
--
-- Same lcd-gui draw API as the other demos: draw embedded-graphics primitives
-- into an in-RAM framebuffer, then `:flush()` rasterizes the frame and emits one
-- `lcd_op` message. The epd2in9v2 driver packs the frame into the panel's single
-- 1-bit RAM plane and runs the full reset -> init -> write -> refresh ->
-- deep-sleep cycle, so each flush is a self-contained refresh.
--
-- E-paper shows only two inks: light colors become white (no ink), dark colors
-- become black. The panel is portrait 128 wide x 296 tall. A full B/W refresh
-- takes ~2-3 s; BUSY is left unconnected, so the worker waits with fixed delays.

function on_start()
    log("info", "LCD GUI epd2in9v2 (B/W) demo...")

    local d = display_new {
        driver = "epd2in9v2",
        w = 128, h = 296,
        cs = 0, dc = 6, rst = 7,
    }


    -- Start from a clean white sheet (the e-paper background).
    d:clear("white")

    -- Title bar: filled black rect with white text.
    d:rect(0, 0, 128, 22, "black", true)
    d:text(36, 4, "CycBox EPD", "white", "6x13")

    -- Black-outlined / filled shapes.
    d:rect(8, 32, 52, 40, "black", false, 2)          -- outlined (2px stroke)
    d:rounded_rect(68, 32, 52, 40, 8, "black", true)  -- filled
    d:square(8, 84, 40, "black", false, 2)            -- equal-sided rect

    -- Circles (outlined + filled) and a separator line.
    d:circle(96, 104, 20, "black", false, 3)
    d:circle(96, 104, 8, "black", true)
    d:line(8, 136, 120, 136, "black", 2)

    -- A black waveform drawn as a polyline.
    local pts = {}
    for x = 8, 120, 6 do
        local y = 168 + math.floor(20 * math.sin((x - 8) / 12))
        pts[#pts + 1] = { x, y }
    end
    d:polyline(pts, "black", 2)

    -- An arc (outline) and a filled sector (pie slice); angles in degrees.
    d:arc(36, 232, 36, 0, 270, "black", 2)
    d:sector(92, 232, 36, 30, 150, "black", true)

    -- Captions in two font sizes.
    d:text(8, 274, "single B/W plane", "black", "7x13")

    d:flush()
    log("info", "Frame flushed: full B/W refresh (~2-3 s).")
end

function on_receive()
    return false
end

--[[
{
  "version": "2.2.1",
  "name": "CH347 LCD GUI (EPD 2.9inch V2)",
  "description": "CH347 SPI connection driving a Waveshare 2.9-inch B/W e-paper V2 on CS0 via the lcd-gui display API (Passthrough codec for raw command/data).",
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
