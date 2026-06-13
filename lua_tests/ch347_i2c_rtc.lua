-- PCF8563 RTC Auto-Sync Monitor
-- Connects a PCF8563 Real-Time Clock via I2C on the CH347 bridge.
-- Polls time every 1 second. If the RTC time differs from system time by >1s,
-- the script automatically synchronizes the RTC with the host system time.
--
-- PCF8563 RTC (I2C Slave)
-- 7-bit Address: 0x51
-- Register Map:
--   0x00: Control 1 (Bit 5 = STOP)
--   0x02: Seconds (BCD, Bit 7 = Voltage Low Flag)
--   0x03: Minutes (BCD)
--   0x04: Hours   (BCD)
--   0x05: Days    (BCD)
--   0x06: Weekdays(BCD)
--   0x07: Months  (BCD, Bit 7 = Century: 0=20xx, 1=19xx)
--   0x08: Years   (BCD)

local SLAVE_ADDR = 0x51
local START_REG  = 0x02
local POLL_MS    = 1000
local last_poll  = 0

-- Helper: Convert BCD byte to Decimal
local function bcd_to_dec(val)
    if not val then return 0 end
    return (bit.rshift(bit.band(val, 0xF0), 4) * 10) + bit.band(val, 0x0F)
end

-- Helper: Convert Decimal to BCD byte
local function dec_to_bcd(val)
    return bit.bor(bit.lshift(math.floor(val / 10), 4), (val % 10))
end

-- Function to write current system time to RTC
local function sync_rtc(sys_ts)
    log("warn", "RTC out of sync. Synchronizing to system time...")
    local t = os.date("*t", sys_ts)
    
    -- 1. Set STOP bit (Register 0x00, Bit 5) to freeze the divider chain
    i2c_write(SLAVE_ADDR, 0x00, string.char(0x20))
    
    -- 2. Write BCD time data to registers 0x02 to 0x08
    -- Order: Sec, Min, Hour, Day, Weekday, Month, Year
    local bcd_data = string.char(
        dec_to_bcd(t.sec),          -- 02h (VL bit becomes 0)
        dec_to_bcd(t.min),          -- 03h
        dec_to_bcd(t.hour),         -- 04h
        dec_to_bcd(t.day),          -- 05h
        dec_to_bcd(t.wday - 1),     -- 06h (Lua wday is 1-7, PCF is 0-6)
        dec_to_bcd(t.month),        -- 07h (Century bit 7 = 0 for 20xx)
        dec_to_bcd(t.year % 100)    -- 08h
    )
    i2c_write(SLAVE_ADDR, 0x02, bcd_data)
    
    -- 3. Clear STOP bit to start the clock
    i2c_write(SLAVE_ADDR, 0x00, string.char(0x00))
    log("info", string.format("RTC synced to %04d-%02d-%02d %02d:%02d:%02d", 
        t.year, t.month, t.day, t.hour, t.min, t.sec))
end

function on_timer(now_ms)
    if now_ms - last_poll >= POLL_MS then
        -- Read 7 bytes starting from Seconds (0x02) to Years (0x08)
        i2c_read(SLAVE_ADDR, START_REG, 7, 0)
        last_poll = now_ms
    end
end

function on_receive()
    -- Filter for I2C read responses
    if message:get_metadata("i2c_op") ~= "read_response" then return false end
    
    local reg = tonumber(message:get_metadata("i2c_register"))
    if reg ~= START_REG then return false end

    local payload = message.payload
    if not payload or #payload < 7 then return false end

    -- Parse registers
    local sec_raw   = string.byte(payload, 1)
    local min_raw   = string.byte(payload, 2)
    local hour_raw  = string.byte(payload, 3)
    local day_raw   = string.byte(payload, 4)
    -- Index 5 is Weekday (0x06) - we skip it for timestamping
    local month_raw = string.byte(payload, 6)
    local year_raw  = string.byte(payload, 7)

    local seconds = bcd_to_dec(bit.band(sec_raw, 0x7F))
    local minutes = bcd_to_dec(bit.band(min_raw, 0x7F))
    local hours   = bcd_to_dec(bit.band(hour_raw, 0x3F))
    local days    = bcd_to_dec(bit.band(day_raw, 0x3F))
    local months  = bcd_to_dec(bit.band(month_raw, 0x1F))
    local years   = bcd_to_dec(year_raw)
    
    -- PCF8563 Century logic: Bit 7 of month register. 0 = 20xx, 1 = 19xx.
    local century_val = bit.band(month_raw, 0x80) == 0 and 2000 or 1900
    local full_year = century_val + years

    -- Check Voltage Low flag (Bit 7 of Seconds)
    local vl_flag = bit.band(sec_raw, 0x80) ~= 0
    
    -- Convert RTC time to Unix Timestamp for comparison
    -- Note: os.time returns UTC epoch; table components are assumed local.
    local rtc_ts = os.time({
        year = full_year,
        month = months,
        day = days,
        hour = hours,
        min = minutes,
        sec = seconds
    })
    
    -- Get current System Unix Timestamp (seconds)
    local sys_ts = math.floor(message.timestamp / 1000000)
    
    -- Check for drift or corruption
    local diff = math.abs(rtc_ts - sys_ts)
    if vl_flag or diff > 1 then
        sync_rtc(sys_ts)
    end

    -- Update UI values
    local status_str = vl_flag and "[CORRUPT/LOW VOLTAGE]" or "[OK]"
    log("info", string.format("RTC: %04d-%02d-%02d %02d:%02d:%02d %s (Diff: %ds)", 
        full_year, months, days, hours, minutes, seconds, status_str, rtc_ts - sys_ts))

    message:add_int_value("rtc_ts", rtc_ts)
    message:add_int_value("sys_ts", sys_ts)
    message:add_int_value("time_drift", rtc_ts - sys_ts)

    return true
end


--[[
{
  "version": "2.2.1",
  "name": "CH347 I2C PCF8563 RTC Auto-Sync Monitor",
  "description": "Connects a PCF8563 Real-Time Clock via I2C on the CH347 bridge.",
  "configs": [
    {
      "app": {
        "app_transport": "ch347_transport",
        "app_codec": "timeout_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "ch347_transport": {
        "ch347_transport_device": "/dev/ch34x_pis0",
        "ch347_transport_i2c_speed": "2",
        "ch347_transport_spi_mode": "3",
        "ch347_transport_spi_clock": "4"
      },
      "timeout_codec": {
        "with_receive_timeout": 50
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "12MNKVHQ",
      "name": "RTC Timekeeping",
      "inputs": [
        {
          "input_type": "i2c",
          "id": "12ZIL6G4",
          "name": "Read Current Time",
          "op": "read",
          "connection_id": 0,
          "address": 81,
          "register": 2,
          "length": 7,
          "raw_value": "",
          "is_hex": true,
          "addr_start": 3,
          "addr_end": 119
        }
      ]
    }
  ]
}
]]
