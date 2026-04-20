-- IMU Burst Packet Parser
-- Packet: 780 bytes, fixed (offsets below are 0-based; Lua read_* calls use 1-based)
--   [0-1]   sync: 0xAA 0x55
--   [2]     type: 0x01
--   [3]     count: 64
--   [4-5]   seq  (LE uint16)
--   [6-9]   ts_us (LE uint32, MCU µs clock, wraps ~71 min)
--   [10-777] 64 × {ax,ay,az,gx,gy,gz} (int16 LE, 12 bytes/sample)
--   [778-779] CRC-16/CCITT-FALSE (validated by frame codec, skipped here)
--
-- Timestamps: ts_us is MCU clock of the OLDEST sample.
-- The first valid message establishes an anchor mapping the newest
-- sample's MCU time to message.timestamp (receive time).
-- Subsequent messages derive Unix timestamps from the MCU clock delta,
-- eliminating receive-time jitter.
--
-- Scales (ISM330DHCX defaults — adjust for your chip/config):
--   Accel ±2 g   → 0.061 mg/LSB → m/s²
--   Gyro  ±2000 dps → 70 mdps/LSB → dps

local PACKET_SIZE  = 780
local SAMPLE_SIZE  = 12  -- 6 × int16

-- 1 000 000 µs / 6664 Hz ≈ 150 µs between samples
local INTERVAL_US  = 150

-- ISM330DHCX: 0.061 mg/LSB at ±2 g
local ACCEL_SCALE  = 0.061e-3 * 9.80665   -- m/s² per LSB
-- ISM330DHCX: 70 mdps/LSB at ±2000 dps
local GYRO_SCALE   = 0.07                  -- dps per LSB

-- Anchor: maps the newest sample's MCU time to receive time
local anchor_recv_us = nil   -- Unix µs of first message receive (≈ newest sample time)
local anchor_mcu_us  = nil   -- MCU µs of first message's newest sample (as plain number)

-- uint32 wrap-safe delta: (current - anchor) mod 2^32
local function mcu_delta(current, anchor)
    local d = current - anchor
    if d < 0 then
        d = d + 4294967296  -- 2^32
    end
    return d
end

function on_receive()
    local p = message.frame

    if #p ~= PACKET_SIZE then
        log("warn", string.format("IMU: bad size %d (expected %d)", #p, PACKET_SIZE))
        return false
    end

    if p.check_valid == false then
        log("warn", "IMU: checksum invalid.")
        return false
    end

    -- Type check
    local ptype = read_u8(p, 3)
    if ptype ~= 0x01 then
        log("warn", string.format("IMU: unknown type 0x%02X", ptype))
        return false
    end

    local count  = read_u8(p, 4)
    local seq    = read_u16_le(p, 5)
    -- ts_us: MCU µs timestamp of oldest sample (Lua offset 7, LE uint32)
    -- tonumber() to ensure plain Lua double (read_u32_le may return cdata)
    local ts_us  = read_u32_le(p, 7)

    -- message.timestamp may be a LuaJIT uint64 cdata; tonumber() converts it
    -- to a regular Lua double so arithmetic stays in normal number space.
    local recv_us = message.timestamp

    -- MCU time of the newest sample in this packet (wrap to uint32 range)
    local newest_mcu_us = (ts_us + (count - 1) * INTERVAL_US) % 4294967296

    -- Establish anchor on first valid message:
    -- recv_us ≈ newest sample time + constant receive delay
    if anchor_recv_us == nil then
        anchor_recv_us = recv_us
        anchor_mcu_us  = newest_mcu_us
        log("info", string.format("IMU: anchor set — recv=%s mcu=%u", tostring(recv_us), newest_mcu_us))
    end

    -- Derive oldest sample's Unix timestamp from MCU clock delta
    local oldest_us = anchor_recv_us + mcu_delta(ts_us, anchor_mcu_us)

    -- Each sample is INTERVAL_US apart from the oldest
    for i = 0, count - 1 do
        local off = 11 + i * SAMPLE_SIZE   -- Lua offset of this sample's ax field

        local ax = read_i16_le(p, off)
        local ay = read_i16_le(p, off + 2)
        local az = read_i16_le(p, off + 4)
        local gx = read_i16_le(p, off + 6)
        local gy = read_i16_le(p, off + 8)
        local gz = read_i16_le(p, off + 10)

        local ts = oldest_us + i * INTERVAL_US

        message:add_float_value("ax", ax * ACCEL_SCALE, ts)
        message:add_float_value("ay", ay * ACCEL_SCALE, ts)
        message:add_float_value("az", az * ACCEL_SCALE, ts)
        
        message:add_float_value("gx", gx * GYRO_SCALE, ts)
        message:add_float_value("gy", gy * GYRO_SCALE, ts)
        message:add_float_value("gz", gz * GYRO_SCALE, ts)
    end

    message:add_int_value("count", count)
    -- Sequence number for drop detection
    message:add_int_value("seq", seq)
    -- MCU oldest-sample timestamp (raw, for clock drift analysis)
    message:add_int_value("mcu_ts_us", ts_us)

    return true
end

--[[
{
  "version": "2.0.0",
  "name": "Serial Frame: Batch Values (IMU)",
  "description": "Parse multiple IMU samples from a single serial frame, with MCU-clock-based timestamp reconstruction",
  "configs": [
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "frame_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "serial_port_transport": {
        "serial_port_transport_port": "/dev/ttyACM0",
        "serial_port_transport_baud_rate": 460800,
        "serial_port_transport_data_bits": 8,
        "serial_port_transport_parity": "none",
        "serial_port_transport_stop_bits": "1",
        "serial_port_transport_flow_control": "none"
      },
      "frame_codec": {
        "frame_codec_prefix": "AA 55",
        "frame_codec_header_size": 0,
        "frame_codec_tailer_length": 0,
        "frame_codec_suffix": "",
        "frame_codec_length_mode": "fixed",
        "frame_codec_fixed_payload_size": 776,
        "frame_codec_length_meaning": "payload_only",
        "frame_codec_checksum_algo": "crc16_ccitt",
        "frame_codec_checksum_scope": "payload"
      }
    }
  ],
  "dashboards": [
    {
      "widgets": [
        {
          "id": "19M21QFT",
          "name": "accel",
          "widget_type": "lineChart",
          "colspan": 4,
          "rowspan": 2,
          "lines": [
            {
              "data_value_id": "ax",
              "label": "ax",
              "color": 4282557941,
              "width": 1,
              "dash_pattern": "solid",
              "unit": "m/s²"
            },
            {
              "data_value_id": "ay",
              "label": "ay",
              "color": 4293874512,
              "width": 1,
              "dash_pattern": "solid",
              "unit": "m/s²"
            },
            {
              "data_value_id": "az",
              "label": "az",
              "color": 4284922730,
              "width": 1,
              "dash_pattern": "solid",
              "unit": "m/s²"
            }
          ]
        },
        {
          "id": "19JZOCLP",
          "name": "Gyro",
          "widget_type": "lineChart",
          "colspan": 4,
          "rowspan": 2,
          "lines": [
            {
              "data_value_id": "gx",
              "label": "gx",
              "color": 4282557941,
              "width": 1,
              "dash_pattern": "solid",
              "unit": "dps"
            },
            {
              "data_value_id": "gy",
              "label": "gy",
              "color": 4293874512,
              "width": 1,
              "dash_pattern": "solid",
              "unit": "dps"
            },
            {
              "data_value_id": "gz",
              "label": "gz",
              "color": 4284922730,
              "width": 1,
              "dash_pattern": "solid",
              "unit": "dps"
            }
          ]
        },
        {
          "id": "19KX6V7X",
          "name": "FFT-AX",
          "widget_type": "fftChart",
          "colspan": 2,
          "rowspan": 2,
          "data_value_id": "ax",
          "color": 4282557941,
          "unit": "",
          "remove_dc": false
        },
        {
          "id": "193K7Q60",
          "name": "FFT-GX",
          "widget_type": "fftChart",
          "colspan": 2,
          "rowspan": 2,
          "data_value_id": "gx",
          "color": 4282557941,
          "unit": "",
          "remove_dc": false
        }
      ]
    }
  ]
}
]]
