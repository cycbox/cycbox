-- Waveshare 4.2" e-Paper (B&C, tri-color) Test -- EPD_4in2bc
-- CH347 SPI at CS0, GPIO6 = DC (Data/Command), GPIO7 = RST (Reset).
-- Draws a black / white / red horizontal-band test pattern.
--
-- Panel: 400 x 300, 1 bit/pixel per color plane (UC8176-class controller).
-- Two image planes are sent: 0x10 = black plane, 0x13 = red plane.
--   White = (0xFF, 0xFF), Black = (0x00, 0xFF), Red = (0xFF, 0x00).
--
-- BUSY is left unconnected: the worker replays the program with real `:delay`
-- sleeps, so the firmware's "wait while BUSY" loops become fixed delays. A
-- tri-color full refresh is slow -- budget ~15-20 s.
--
-- Unlike the old script there is no manual DC toggling, no `t` timeline and no
-- chunk loop: :data() takes a whole 15000-byte plane and the CH347 worker splits
-- it into SPI writes under the USB buffer limit.

local W       = 400
local H       = 300
local W_BYTES = W / 8 -- 50 bytes per row

-- Build the two planes for a black / white / red horizontal-band pattern.
local function build_planes()
    local black, red = {}, {}
    local all_white = string.rep(string.char(0xFF), W_BYTES)
    local all_black = string.rep(string.char(0x00), W_BYTES)
    local third = math.floor(H / 3)
    for y = 0, H - 1 do
        if y < third then         -- top: black  -> black=0x00, red=0xFF
            black[#black + 1] = all_black
            red[#red + 1]     = all_white
        elseif y < 2 * third then -- middle: white -> black=0xFF, red=0xFF
            black[#black + 1] = all_white
            red[#red + 1]     = all_white
        else                      -- bottom: red  -> black=0xFF, red=0x00
            black[#black + 1] = all_white
            red[#red + 1]     = all_black
        end
    end
    return table.concat(black), table.concat(red)
end

function on_start()
    log("info", "EPD 4.2inch B&C (tri-color): init + full refresh test...")

    local epd = lcd_new { cs = 0, dc = 6, rst = 7, chunk = 2500 }

    -- 1. Reset, mirroring EPD_4IN2BC_Reset(): high 200ms, low 5ms, high 200ms.
    epd:reset(200, 5, 200)

    -- 2. Init (LUT from OTP) ---------------------------------------------
    epd:cmd(0x06, { 0x17, 0x17, 0x17 }) -- booster soft start
    epd:cmd(0x04) epd:delay(300)        -- POWER_ON (wait BUSY ~300ms)
    epd:cmd(0x00, 0x0F)                 -- panel setting: LUT from OTP

    -- 3. Push planes: 0x10 = black, 0x13 = red ---------------------------
    local black_plane, red_plane = build_planes()
    epd:cmd(0x10) epd:data(black_plane)
    epd:cmd(0x13) epd:data(red_plane)

    -- 4. Refresh and wait out the tri-color update (~15-20 s) -------------
    epd:cmd(0x12) epd:delay(18000)

    -- 5. Deep sleep ------------------------------------------------------
    epd:cmd(0x02) epd:delay(300) -- POWER_OFF (wait BUSY)
    epd:cmd(0x07, 0xA5)          -- DEEP_SLEEP

    epd:flush()
    log("info", string.format("EPD B&C test flushed: 2 x %d bytes.", W_BYTES * H))
end

function on_receive()
    return false
end

--[[
{
  "version": "2.2.1",
  "name": "CH347 SPI E-Paper 4.2inch BC Test",
  "description": "CH347 SPI connection for the Waveshare 4.2-inch tri-color e-paper (BC) on CS0 using the Passthrough codec for raw command/data transmission.",
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
