-- LCD GUI image loading on a Waveshare 4.2" tri-color e-paper (B/W/R) over CH347.
-- GPIO6 = DC, GPIO7 = RST, SPI CS0. Panel: 400x300, UC8176-class controller.
--
-- Demonstrates `display:load_image()` on an e-paper: decode a local
-- PNG/JPEG/BMP/GIF in Rust, scale it to fit a box (aspect preserved), and blit
-- it into the in-RAM framebuffer. `:flush()` then splits the frame into the
-- panel's two 1-bit planes (black + red) and runs the full reset -> power-on ->
-- refresh -> deep-sleep cycle, so each flush is a self-contained refresh.
--
-- E-paper shows only three inks, so each pixel is classified: strongly red ->
-- red, otherwise light -> white / dark -> black. `dither = true` applies
-- Floyd–Steinberg before quantization, which greatly improves photo rendering
-- on such a limited palette. A tri-color full refresh is slow (~18 s); BUSY is
-- left unconnected, so the worker waits with fixed delays.
--
-- Edit IMAGE_PATH to point at a file readable on the host running CycBox.

local IMAGE_PATH = "/tmp/400300.png"

function on_start()
    log("info", "LCD GUI epd4in2bc image demo...")

    local d = display_new {
        driver = "epd4in2bc",
        w = 400, h = 300,
        cs = 0, dc = 6, rst = 7,
    }

    -- Start from a clean white sheet (the e-paper background).
    d:clear("white")

    -- Decode IMAGE_PATH and scale it to fit the full 400x300 sheet, preserving
    -- aspect ratio. Dithering helps map a full-color photo onto the B/W/R
    -- palette. load_image returns the size actually drawn.
    local ok, w, h = pcall(function()
        return d:load_image(0, 0, IMAGE_PATH, { w = 400, h = 300, dither = true })
    end)
    if ok then
        log("info", "Image blitted: " .. w .. "x" .. h)
    else
        log("error", "load_image failed: " .. tostring(w))
    end

    d:flush()
    log("info", "Frame flushed: full tri-color refresh (~18 s).")
end

function on_receive()
    return false
end

--[[
{
  "version": "2.2.1",
  "name": "CH347 LCD GUI image (EPD 4.2inch BC)",
  "description": "CH347 SPI connection driving a Waveshare 4.2-inch tri-color e-paper (BC) on CS0, loading a local image file via the lcd-gui display:load_image API (Passthrough codec for raw command/data).",
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
