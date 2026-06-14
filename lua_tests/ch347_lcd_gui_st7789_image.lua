-- LCD GUI image slideshow (ST7789V2, Waveshare 1.69" 240x280) over CH347.
-- GPIO6 = DC, GPIO7 = RST, SPI CS0.
--
-- Demonstrates `display:load_image()` driven from `on_timer`: every SLIDE_MS the
-- script decodes the next local PNG/JPEG/BMP/GIF file in Rust, scales it to fit
-- the panel (aspect preserved), blits it into the in-RAM framebuffer and
-- `:flush()`es one `lcd_op` message. The controller init sequence runs
-- automatically on the first flush; later flushes just stream the new frame.
--
-- Edit IMAGE_PATHS to point at files readable on the host running CycBox.

local IMAGE_PATHS = {
    "/tmp/240280.png",
    "/tmp/image2.png",
    "/tmp/image3.png",
}

local SLIDE_MS = 3000 -- dwell time per image

local d          -- created lazily so flush() runs on the engine thread
local index = 0  -- index of the image currently shown (0 = none yet)
local last_swap = 0

local function show_image(i)
    local path = IMAGE_PATHS[i]

    -- Start from a clean black frame.
    d:clear("black")

    -- Decode `path` and scale it to fill the panel, preserving aspect ratio.
    local ok, w, h = pcall(function()
        return d:load_image(0, 0, path, { w = 240, h = 280 })
    end)
    if not ok then
        log("warn", "Failed to load image: " .. tostring(path))
        d:text(10, 130, "load failed", "red", "10x20")
    end

    d:flush()
    log("info", string.format("Showing image %d/%d: %s", i, #IMAGE_PATHS, path))
end

function on_start()
    log("info", "LCD GUI st7789 image slideshow...")

    d = display_new {
        driver = "st7789",
        w = 240, h = 280,
        y_offset = 20, -- this glass is inset 20px in the row direction
        cs = 0, dc = 6, rst = 7,
    }
end

function on_timer(now_ms)
    -- Advance to the next image once SLIDE_MS has elapsed (also fires the first
    -- frame immediately, since last_swap starts at 0).
    if index == 0 or now_ms - last_swap >= SLIDE_MS then
        index = (index % #IMAGE_PATHS) + 1
        show_image(index)
        last_swap = now_ms
    end
end

function on_receive()
    return false
end

--[[
{
  "version": "2.2.1",
  "name": "CH347 LCD GUI image slideshow (ST7789)",
  "description": "CH347 SPI connection driving a 1.69-inch ST7789 LCD on CS0, cycling through multiple local image files on a timer via the lcd-gui display:load_image API (Passthrough codec for raw command/data).",
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
