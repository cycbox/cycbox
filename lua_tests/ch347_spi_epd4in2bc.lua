-- Waveshare 4.2" e-Paper (B&C, tri-color) Test -- EPD_4in2bc
-- CH347 SPI transport at CS0, with GPIO0 for DC (Data/Command) and GPIO1 for RST (Reset).
-- Ported from EPD_4in2bc.c and draws a black / white / red(yellow) test pattern.
--
-- Panel: 400 x 300, 1 bit/pixel per color plane, three-color (UC8176-class controller).
-- Wiring: DC = GPIO0, RST = GPIO1, CS = SPI CS0.
--   BUSY is left unconnected: the CH347 transport is a scheduled timeline (each helper queues
--   a message at a future timestamp), so the firmware's "wait while BUSY" loops are replaced
--   with fixed delays. A TRI-COLOR full update is slow -- budget ~15-20 s for the refresh.
--
-- This panel loads its waveform LUT from OTP, so init is just: booster -> power on -> panel
-- setting 0x0F. Two image planes are sent:
--   0x10 = black plane  (bit 1 = white, bit 0 = black)
--   0x13 = red plane     (bit 1 = no red, bit 0 = red)
-- White = (0xFF, 0xFF), Black = (0x00, 0xFF), Red = (0xFF, 0x00).

local CS       = 0
local DC_PIN   = 6
local RST_PIN  = 7

local W        = 400
local H        = 300
local W_BYTES  = W / 8         -- 50 bytes per row
local FRAME    = W_BYTES * H   -- 15000 bytes per plane
local CHUNK    = 2500          -- SPI payload per transaction (under buffer limit)

-- Timeline cursor in milliseconds. Each helper schedules its message `t` ms in the future and
-- advances `t`, so messages reach the bus in issue order.
local t = 0

-- Command byte: DC low, then clock one byte.
local function cmd(reg)
    gpio_write(DC_PIN, 0, 0, t) t = t + 1
    spi_write(CS, string.char(reg), 0, t) t = t + 2
end

-- Data byte (number) or blob (string): DC high, then clock it out.
local function data(payload)
    if type(payload) == "number" then payload = string.char(payload) end
    gpio_write(DC_PIN, 1, 0, t) t = t + 1
    spi_write(CS, payload, 0, t) t = t + 2
end

-- Stream a full image plane as data: DC high once, then chunk the payload.
local function send_frame(buf)
    gpio_write(DC_PIN, 1, 0, t) t = t + 1
    for i = 1, #buf, CHUNK do
        spi_write(CS, buf:sub(i, i + CHUNK - 1), 0, t)
        t = t + 5
    end
end

-- Hardware reset, mirroring EPD_4IN2BC_Reset(): high 200ms, low 5ms, high 200ms.
local function reset()
    gpio_write(RST_PIN, 1, 0, t) t = t + 200
    gpio_write(RST_PIN, 0, 0, t) t = t + 5
    gpio_write(RST_PIN, 1, 0, t) t = t + 200
end

-- Build the two planes for a black / white / red horizontal-band test pattern.
-- Returns black_plane, red_plane (each FRAME bytes).
local function build_planes()
    local black, red = {}, {}
    local all_white = string.rep(string.char(0xFF), W_BYTES)
    local all_black = string.rep(string.char(0x00), W_BYTES)
    local third = math.floor(H / 3)
    for y = 0, H - 1 do
        if y < third then             -- top: black  -> black=0x00, red=0xFF
            black[#black + 1] = all_black
            red[#red + 1]     = all_white
        elseif y < 2 * third then     -- middle: white -> black=0xFF, red=0xFF
            black[#black + 1] = all_white
            red[#red + 1]     = all_white
        else                          -- bottom: red  -> black=0xFF, red=0x00
            black[#black + 1] = all_white
            red[#red + 1]     = all_black
        end
    end
    return table.concat(black), table.concat(red)
end

function on_start()
    log("info", "EPD 4.2inch B&C (tri-color): init + full refresh test...")

    -- 1. Reset
    reset()

    -- 2. Init (LUT from OTP) ----------------------------------------------
    cmd(0x06) data(0x17) data(0x17) data(0x17)  -- booster soft start
    cmd(0x04) t = t + 300                        -- POWER_ON (wait BUSY ~300ms)
    cmd(0x00) data(0x0F)                         -- panel setting: LUT from OTP

    -- 3. Push planes: 0x10 = black, 0x13 = red ----------------------------
    local black_plane, red_plane = build_planes()
    cmd(0x10) send_frame(black_plane)
    cmd(0x13) send_frame(red_plane)

    -- 4. Refresh and wait out the tri-color update (~15-20 s) --------------
    cmd(0x12) t = t + 18000

    -- 5. Deep sleep -------------------------------------------------------
    cmd(0x02) t = t + 300   -- POWER_OFF (wait BUSY)
    cmd(0x07) data(0xA5)    -- DEEP_SLEEP

    log("info", string.format("EPD B&C test queued: 2 x %d bytes over %d ms timeline.", FRAME, t))
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
