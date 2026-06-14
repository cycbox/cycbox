-- LCD GUI demo on a Waveshare 4.2" tri-color e-paper (B/W/R) over CH347.
-- GPIO6 = DC, GPIO7 = RST, SPI CS0. Panel: 400x300, UC8176-class controller.
--
-- Same lcd-gui draw API as the ST7789 demo: draw embedded-graphics primitives
-- into an in-RAM framebuffer, then `:flush()` rasterizes the frame and emits one
-- `lcd_op` message. The epd4in2bc driver splits the frame into the panel's two
-- 1-bit planes (black + red) and runs the full reset -> power-on -> refresh ->
-- deep-sleep cycle, so each flush is a self-contained refresh.
--
-- E-paper shows only three inks, so colors are classified: strongly red -> red,
-- otherwise light -> white / dark -> black. A tri-color full refresh is slow
-- (~18 s); BUSY is left unconnected, so the worker waits with fixed delays.

function on_start()
    log("info", "LCD GUI epd4in2bc (tri-color) demo...")

    local d = display_new {
        driver = "epd4in2bc",
        w = 400, h = 300,
        cs = 0, dc = 6, rst = 7,
    }

    -- Start from a clean white sheet (the e-paper background).
    d:clear("white")

    -- Title bar: filled red rect with white text.
    d:rect(0, 0, 400, 40, "red", true)
    d:text(10, 27, "CycBox e-Paper GUI", "white", "10x20")

    -- Black-outlined / filled shapes.
    d:rect(12, 56, 110, 70, "black", false, 2)         -- outlined (2px stroke)
    d:rounded_rect(140, 56, 110, 70, 12, "black", true) -- filled
    d:circle(312, 91, 34, "black", false, 3)           -- outlined

    -- Red accents.
    d:circle(312, 91, 16, "red", true)                 -- filled dot
    d:triangle(12, 150, 12, 230, 80, 190, "red", true) -- filled triangle
    d:line(100, 150, 388, 150, "black", 2)             -- separator

    -- A red waveform drawn as a polyline.
    local pts = {}
    for x = 100, 388, 8 do
        local y = 200 + math.floor(30 * math.sin((x - 100) / 24))
        pts[#pts + 1] = { x, y }
    end
    d:polyline(pts, "red", 2)

    -- Captions in black, two font sizes.
    d:text(12, 250, "black / white / red planes", "black", "9x18")
    d:text(12, 276, "embedded-graphics primitives", "black", "6x13")

    d:flush()
    log("info", "Frame flushed: full tri-color refresh (~18 s).")
end

function on_receive()
    return false
end

--[[
{
  "version": "2.2.1",
  "name": "CH347 LCD GUI (EPD 4.2inch BC)",
  "description": "CH347 SPI connection driving a Waveshare 4.2-inch tri-color e-paper (BC) on CS0 via the lcd-gui display API (Passthrough codec for raw command/data).",
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
