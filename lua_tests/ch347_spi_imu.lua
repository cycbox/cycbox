-- LSM6DSR IMU SPI Controller
-- Uses a CH347 USB-to-SPI bridge to initialize and poll an LSM6DSR inertial sensor.
-- Verifies device ID on start, configures ODR/Scale, and polls Accel/Gyro data every 100ms.
--
-- Device: LSM6DSR (6-Axis IMU)
--   SPI Config: Mode 3, MSB first, Register address | 0x80 for Reads.
--   Slave Address (WHO_AM_I): 0x0F (Expected: 0x6B)
--   Data Registers: 0x22 (Gyro X Low) through 0x2D (Accel Z High)
--
-- Scaling (at ±2g and ±2000dps):
--   Accel: raw * (2.0 / 32768) [g]
--   Gyro:  raw * (2000.0 / 32768) [dps]

local CS = 0
local CONN_ID = 0

-- Command constants
local CMD_READ = 0x80
local REG_WHO_AM_I = 0x0F
local REG_CTRL1_XL = 0x10
local REG_CTRL2_G  = 0x11
local REG_CTRL3_C  = 0x12
local REG_DATA     = 0x22

-- Scales
local ACCEL_FS_2G = 2.0 / 32768.0
local GYRO_FS_2000 = 2000.0 / 32768.0

function on_start()
    log("info", "Initializing LSM6DSR on CS0...")
    
    -- 1. Check identity
    spi_read(CS, string.char(bit.bor(REG_WHO_AM_I, CMD_READ)), 1, CONN_ID)
    
    -- 2. Configure: 416Hz ODR, ±2g, ±2000dps, BDU enabled, Auto-increment enabled
    -- CTRL1_XL: 0x60 (416Hz, 2g)
    spi_write(CS, string.char(REG_CTRL1_XL, 0x60), CONN_ID)
    -- CTRL2_G: 0x60 (416Hz, 2000dps)
    spi_write(CS, string.char(REG_CTRL2_G, 0x60), CONN_ID)
    -- CTRL3_C: 0x44 (BDU=1, IF_INC=1)
    spi_write(CS, string.char(REG_CTRL3_C, 0x44), CONN_ID)
    
    log("info", "Initialization commands sent.")
end

function on_timer(now_ms)
    spi_read(CS, string.char(bit.bor(REG_DATA, CMD_READ)), 12, CONN_ID)
end

function on_receive()
    local op = message:get_metadata("spi_op")
    if op ~= "read_response" then return false end
    
    local p = message.payload
    if not p then return false end

    -- Handle WHO_AM_I response
    if #p == 1 then
        local id = read_u8(p, 1)
        log("info", string.format("LSM6DSR WHO_AM_I: 0x%02X", id))
        message:add_int_value("who_am_i", id)
        message.highlighted = (id == 0x6B)
        return true
    end

    -- Handle IMU Data response (12 bytes)
    if #p == 12 then
        local gx_raw = read_i16_le(p, 1)
        local gy_raw = read_i16_le(p, 3)
        local gz_raw = read_i16_le(p, 5)
        local ax_raw = read_i16_le(p, 7)
        local ay_raw = read_i16_le(p, 9)
        local az_raw = read_i16_le(p, 11)
        

        message:add_float_value("ax", ax_raw * ACCEL_FS_2G)
        message:add_float_value("ay", ay_raw * ACCEL_FS_2G)
        message:add_float_value("az", az_raw * ACCEL_FS_2G)
        
        message:add_float_value("gx", gx_raw * GYRO_FS_2000)
        message:add_float_value("gy", gy_raw * GYRO_FS_2000)
        message:add_float_value("gz", gz_raw * GYRO_FS_2000)
        
        return true
    end

    return false
end


--[[
{
  "version": "2.2.1",
  "name": "CH347 LSM6DSR SPI Reader",
  "description": "Configured CH347 SPI transport for LSM6DSR on CS0 (7.5MHz, Mode 3) with CBRT decoding and transformation for 6-channel IMU data.",
  "configs": [
    {
      "app": {
        "app_transport": "ch347_transport",
        "app_codec": "cbrt_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "ch347_transport": {
        "ch347_transport_device": "/dev/ch34x_pis0",
        "ch347_transport_i2c_speed": "2",
        "ch347_transport_spi_mode": "3",
        "ch347_transport_spi_clock": "3"
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "12Q2EJCW",
      "name": "LSM6DSR Setup",
      "inputs": [
        {
          "input_type": "spi",
          "id": "12JOOVQM",
          "name": "Check WHO_AM_I (0x6B)",
          "op": "read",
          "connection_id": 0,
          "cs": 0,
          "length": 1,
          "raw_value": "8F",
          "is_hex": true
        },
        {
          "input_type": "spi",
          "id": "12K7WOTA",
          "name": "Software Reset",
          "op": "write",
          "connection_id": 0,
          "cs": 0,
          "length": 1,
          "raw_value": "12 01",
          "is_hex": true
        },
        {
          "input_type": "spi",
          "id": "12WJ39VL",
          "name": "Set Accel ODR (6.66kHz)",
          "op": "write",
          "connection_id": 0,
          "cs": 0,
          "length": 1,
          "raw_value": "10 A0",
          "is_hex": true
        },
        {
          "input_type": "spi",
          "id": "12S8NWWJ",
          "name": "Set Gyro ODR (6.66kHz, 2000dps)",
          "op": "write",
          "connection_id": 0,
          "cs": 0,
          "length": 1,
          "raw_value": "11 AC",
          "is_hex": true
        },
        {
          "input_type": "spi",
          "id": "12EGAI49",
          "name": "Set BDU & IF_INC",
          "op": "write",
          "connection_id": 0,
          "cs": 0,
          "length": 1,
          "raw_value": "12 44",
          "is_hex": true
        },
        {
          "input_type": "spi",
          "id": "12V2K9EX",
          "name": "Disable I2C Interface",
          "op": "write",
          "connection_id": 0,
          "cs": 0,
          "length": 1,
          "raw_value": "13 04",
          "is_hex": true
        }
      ]
    },
    {
      "id": "12Y0Q6C7",
      "name": "LSM6DSR Manual Readout",
      "inputs": [
        {
          "input_type": "spi",
          "id": "12GE5PXW",
          "name": "Read Accelerometer (Raw)",
          "op": "read",
          "connection_id": 0,
          "cs": 0,
          "length": 6,
          "raw_value": "A8",
          "is_hex": true
        },
        {
          "input_type": "spi",
          "id": "12S0CM2J",
          "name": "Read Gyroscope (Raw)",
          "op": "read",
          "connection_id": 0,
          "cs": 0,
          "length": 6,
          "raw_value": "A2",
          "is_hex": true
        },
        {
          "input_type": "spi",
          "id": "126H90HG",
          "name": "Read FIFO Status",
          "op": "read",
          "connection_id": 0,
          "cs": 0,
          "length": 2,
          "raw_value": "BA",
          "is_hex": true
        }
      ]
    }
  ],
  "dashboards": [
    {
      "schema_version": 2,
      "widgets": [
        {
          "id": "12T51WGC",
          "name": "ax, ay, az",
          "widget_type": "lineChart",
          "colspan": 6,
          "rowspan": 3,
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
          "id": "12RK2IYX",
          "name": "gx, gy, gz",
          "widget_type": "lineChart",
          "colspan": 6,
          "rowspan": 3,
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
        }
      ]
    }
  ]
}
]]
