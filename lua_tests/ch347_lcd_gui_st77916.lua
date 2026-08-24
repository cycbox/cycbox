-- LCD GUI prototype (ST77916, 1.8" 360x360 round panel) over CH347.
-- GPIO6 = DC, GPIO7 = RST, SPI CS0.
--
-- Demonstrates the lcd-gui layer: draw with embedded-graphics primitives from
-- Lua into an in-RAM framebuffer, then `:flush()` rasterizes the whole frame
-- into one `lcd_op` message. The controller init sequence runs automatically on
-- the first flush -- no manual command tables, no DC toggling, no chunking.
--
-- The ST77916 glass is circular, so the demo keeps content inside a centered
-- circle (cx, cy = 180, radius ~180).
--
-- Colors accept 0xRRGGBB, a name ("red"), or a {r,g,b} table.

function on_start()
    log("info", "LCD GUI st77916 demo...")

    local d = display_new {
        driver = "st77916",
        w = 360, h = 360,
        invert = true, -- round ST77916 panels expect display inversion on
        cs = 0, dc = 6, rst = 7,
    }

    local cx, cy = 180, 180

    -- Start from a clean black frame.
    d:clear("black")

    -- Outer bezel ring that traces the round edge of the glass.
    d:circle(cx, cy, 178, 0x003366, false, 4)

    -- Title arc-fitted text near the top.
    d:text(cx - 46, 46, "CycBox GUI", "white", "9x18bold")

    -- Concentric decorative rings.
    d:circle(cx, cy, 144, 0x224466, false, 2)
    d:circle(cx, cy, 105, "cyan", false, 3)
    d:circle(cx, cy, 27, { 255, 128, 0 }, true) -- orange hub via {r,g,b} table

    -- A pie sector and an arc, both centered on the hub.
    d:sector(cx, cy, 90, 200, 320, 0x00AA00, true)
    d:arc(cx, cy, 126, 20, 160, "magenta", 4)

    -- Tick marks around the dial, every 30 degrees.
    for deg = 0, 330, 30 do
        local a = deg * math.pi / 180
        local x0 = cx + math.floor(math.cos(a) * 156)
        local y0 = cy + math.floor(math.sin(a) * 156)
        local x1 = cx + math.floor(math.cos(a) * 171)
        local y1 = cy + math.floor(math.sin(a) * 171)
        d:line(x0, y0, x1, y1, "white", 3)
    end

    -- A "needle" line plus a small filled triangle pointer.
    d:line(cx, cy, cx + 75, cy - 45, "yellow", 4)
    d:triangle(cx - 15, cy + 60, cx + 15, cy + 60, cx, cy + 87, "red", true)

    -- A tiny waveform strip across the lower middle.
    d:polyline({
        { 105, 252 }, { 128, 234 }, { 150, 264 },
        { 172, 237 }, { 195, 261 }, { 218, 240 }, { 240, 255 },
    }, "cyan", 3)

    -- A 24x24 image blitted from raw big-endian RGB565 bytes: a red->blue
    -- horizontal gradient built on the fly (height is derived from length).
    local w, h = 24, 24
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
    d:image(cx - 12, 282, w, table.concat(rows))

    -- A closing caption in the default font near the bottom of the circle.
    d:text(cx - 40, 320, "round panel", 0xAAAAAA)

    d:flush()
    log("info", "Frame flushed to panel.")
end

function on_receive()
    return false
end

--[[
{
  "version": "2.2.1",
  "name": "CH347 LCD GUI (ST77916)",
  "description": "CH347 SPI connection driving a 1.8-inch ST77916 360x360 round LCD on CS0 via the lcd-gui display API (Passthrough codec for raw command/data).",
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
