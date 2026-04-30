-- LSM6DSR IMU Burst Packet Parser and Forwarder
-- Frame Codec over Serial Port (Conn 0): parses 776-byte batched IMU payloads from an LSM6DSR sensor.
-- TCP Client (Conn 1): Passthrough Codec to forward the raw frames.
-- Decodes 64 ACC/GYR samples per batch (6664 Hz) and aligns MCU microsecond timestamps to the receive time.
-- Incoming raw frames from the serial port are forwarded out via the TCP connection.
--
-- Packet Layout (Payload only - 776 bytes):
--   [1]     type
--   [2]     count: 64
--   [3-4]   seq  (LE uint16)
--   [5-8]   ts_us (LE uint32, MCU µs clock)
--   [9-776] 64 × {ax, ay, az, gx, gy, gz} (int16 LE, 12 bytes/sample)
--
-- Scales (LSM6DSR):
--   Accel ±2 g   → 0.061 mg/LSB → ~0.000598 m/s²
--   Gyro  ±2000 dps → 70 mdps/LSB → 0.07 dps

local PAYLOAD_SIZE = 776
local SAMPLE_SIZE  = 12

local INTERVAL_US  = 1000000 / 6664

-- LSM6DSR: 0.061 mg/LSB at ±2 g
local ACCEL_SCALE  = 0.061e-3 * 9.80665
-- LSM6DSR: 70 mdps/LSB at ±2000 dps
local GYRO_SCALE   = 0.07

local anchor_recv_us = nil
local anchor_mcu_us  = nil

local function mcu_delta(current, anchor)
    local d = current - anchor
    if d < 0 then
        d = d + 4294967296
    end
    return d
end

function on_receive()
    -- Only process and forward messages originating from the Serial connection (ID 0)
    if message.connection_id ~= 0 then
        return false
    end

    local p = message.payload

    if p == nil or #p ~= PAYLOAD_SIZE then
        return false
    end

    if message.checksum_valid == false then
        log("warn", "IMU: checksum invalid.")
        return false
    end

    -- Forward the original complete wire frame to the TCP connection (ID 1)
    if message.frame then
        send_after(message.frame, 0, 1)
    end

    local ptype = read_u8(p, 1)
    local count  = read_u8(p, 2)
    local seq    = read_u16_le(p, 3)
    local ts_us  = read_u32_le(p, 5)

    local recv_us = tonumber(message.timestamp)
    local newest_mcu_us = (ts_us + (count - 1) * INTERVAL_US) % 4294967296

    if anchor_recv_us == nil then
        anchor_recv_us = recv_us
        anchor_mcu_us  = newest_mcu_us
        log("info", string.format("IMU: anchor set — recv=%s mcu=%u", tostring(recv_us), newest_mcu_us))
    end

    local oldest_us = anchor_recv_us + mcu_delta(ts_us, anchor_mcu_us)

    for i = 0, count - 1 do
        local off = 9 + i * SAMPLE_SIZE

        local ax = read_i16_le(p, off)
        local ay = read_i16_le(p, off + 2)
        local az = read_i16_le(p, off + 4)
        local gx = read_i16_le(p, off + 6)
        local gy = read_i16_le(p, off + 8)
        local gz = read_i16_le(p, off + 10)

        local ts = math.floor(oldest_us + i * INTERVAL_US)

        message:add_float_value("ax", ax * ACCEL_SCALE, ts)
        message:add_float_value("ay", ay * ACCEL_SCALE, ts)
        message:add_float_value("az", az * ACCEL_SCALE, ts)
        
        message:add_float_value("gx", gx * GYRO_SCALE, ts)
        message:add_float_value("gy", gy * GYRO_SCALE, ts)
        message:add_float_value("gz", gz * GYRO_SCALE, ts)
    end

    message:add_int_value("count", count)
    message:add_int_value("seq", seq)
    message:add_int_value("mcu_ts_us", ts_us)

    return true
end

--[[
{
  "version": "2.0.0",
  "name": "LSM6DSR IMU Burst Packet Parser and Forwarder",
  "description": "Added a TCP Client connection to support data forwarding from the existing serial port.",
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
        "serial_port_transport_baud_rate": 115200,
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
    },
    {
      "app": {
        "app_transport": "tcp_client_transport",
        "app_codec": "passthrough_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "tcp_client_transport": {
        "tcp_client_transport_host": "127.0.0.1",
        "tcp_client_transport_port": 8080,
        "tcp_client_transport_timeout": 5000,
        "tcp_client_transport_keepalive": true,
        "tcp_client_transport_nodelay": true
      }
    }
  ],
  "dashboards": [
    {
      "widgets": [
        {
          "id": "29FVCMOY",
          "name": "Accelerometer",
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
              "unit": ""
            },
            {
              "data_value_id": "ay",
              "label": "ay",
              "color": 4293874512,
              "width": 1,
              "dash_pattern": "solid",
              "unit": ""
            },
            {
              "data_value_id": "az",
              "label": "az",
              "color": 4284922730,
              "width": 1,
              "dash_pattern": "solid",
              "unit": ""
            }
          ]
        },
        {
          "id": "29LXMM1O",
          "name": "Gyroscope",
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
              "unit": ""
            },
            {
              "data_value_id": "gy",
              "label": "gy",
              "color": 4293874512,
              "width": 1,
              "dash_pattern": "solid",
              "unit": ""
            },
            {
              "data_value_id": "gz",
              "label": "gz",
              "color": 4284922730,
              "width": 1,
              "dash_pattern": "solid",
              "unit": ""
            }
          ]
        },
        {
          "id": "29VCOLHH",
          "name": "FFT: ax",
          "widget_type": "fftChart",
          "colspan": 2,
          "rowspan": 2,
          "data_value_id": "ax",
          "color": 4282557941,
          "unit": "",
          "remove_dc": false
        },
        {
          "id": "29SQ730H",
          "name": "FFT: gy",
          "widget_type": "fftChart",
          "colspan": 2,
          "rowspan": 2,
          "data_value_id": "gy",
          "color": 4294201630,
          "unit": "",
          "remove_dc": false
        }
      ]
    }
  ]
}
]]
