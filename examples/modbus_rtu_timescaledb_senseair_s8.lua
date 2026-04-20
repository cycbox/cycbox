-- Senseair S8 CO2 Sensor - Modbus RTU with TimescaleDB Storage
-- Reads CO2 (Input Register IR4, address 0x0003) every 10 seconds
-- Slave address: 0xFE (254, "any sensor")
--
-- Required TimescaleDB schema:
--   CREATE TABLE IF NOT EXISTS co2_readings (
--       time   TIMESTAMPTZ DEFAULT NOW(),
--       sensor TEXT        NOT NULL,
--       co2    INTEGER     NOT NULL
--   );
--   SELECT create_hypertable('co2_readings', 'time', if_not_exists => TRUE);

local SLAVE_ADDR     = 0xFE  -- 254, "any sensor" address
local CO2_REG_ADDR   = 0x0003  -- IR4 starting address (register number - 1)
local CO2_REG_QTY    = 1

local CONNSTR        = "host=localhost port=5432 dbname=cycbox user=postgres password=xxxxxx sslmode=disable"
local POOL_SIZE      = 3

local POLL_INTERVAL  = 10000  -- 10 seconds in ms
local timer_counter  = 0

function on_start()
    -- Connect to TimescaleDB
    local ok, err = timescaledb_connect(CONNSTR, POOL_SIZE)
    if not ok then
        log("error", "Failed to connect to TimescaleDB: " .. (err or "unknown error"))
    else
        log("info", "Connected to TimescaleDB")
    end

    -- Send first read immediately
    modbus_rtu_read_input_registers(SLAVE_ADDR, CO2_REG_ADDR, CO2_REG_QTY, 0, 0)
    log("info", "Senseair S8 polling started")
end

function on_receive()
    if message.connection_id ~= 0 then
        return false
    end

    -- Auto-parsed value ID for input register IR4 (address 0x0003)
    -- Logical key: modbus_rtu_254:30004 (30001 + 3)
    -- Protocol-type key: modbus_rtu_254:input_0003
    local co2 = message:get_value("modbus_rtu_254:30004")
    if co2 == nil then
        log("warn", "No CO2 value in response")
        return false
    end

    log("info", string.format("CO2: %d ppm", co2))
    message:add_int_value("CO2", co2)

    -- Async insert to TimescaleDB
    timescaledb_insert_async("co2_readings", {"sensor", "co2"}, {"senseair_s8", co2})

    return true
end

function on_timer(timestamp_ms)
    timer_counter = timer_counter + 100
    if timer_counter < POLL_INTERVAL then
        return
    end
    timer_counter = 0

    -- Poll CO2 register every 10 seconds
    modbus_rtu_read_input_registers(SLAVE_ADDR, CO2_REG_ADDR, CO2_REG_QTY, 0, 0)
end

function on_stop()
    timescaledb_disconnect()
    log("info", "Senseair S8 polling stopped")
end

--[[
{
  "version": "2.0.0",
  "name": "Senseair S8 CO2 Sensor to TimescaleDB",
  "description": "Modbus RTU with TimescaleDB Storage",
  "configs": [
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "modbus_rtu_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "serial_port_transport": {
        "serial_port_transport_port": "/dev/ttyUSB0",
        "serial_port_transport_baud_rate": 9600,
        "serial_port_transport_data_bits": 8,
        "serial_port_transport_parity": "none",
        "serial_port_transport_stop_bits": "1",
        "serial_port_transport_flow_control": "none"
      },
      "modbus_rtu_codec": {
        "with_receive_timeout": 20
      }
    }
  ]
}
]]
