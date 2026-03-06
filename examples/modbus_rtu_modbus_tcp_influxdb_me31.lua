-- ME31-XDXX0400 PT100 Temperature Acquisition to InfluxDB v3
-- Reads CH1 temperature via Modbus RTU (connection 0) or Modbus TCP (connection 1),
-- uses codec-parsed register values (modbus_rtu_X / modbus_tcp_X), and writes to InfluxDB v3 async.
--
-- ME31 temperature register: input register 0x0190, function code 0x04
-- Value is signed 16-bit integer, divide by 10 to get °C (e.g. 0x012C = 300 → 30.0°C)
-- Value 0xFC19 (-999) means sensor not connected.
--
-- Configure USE_RTU below to switch between RTU and TCP reading.

-- InfluxDB v3 configuration
local INFLUXDB_URL   = get_env("INFLUXDB_URL")   or "http://localhost:8181"
local INFLUXDB_TOKEN = get_env("INFLUXDB_TOKEN") or ""
local INFLUXDB_DB    = get_env("INFLUXDB_DB")    or "temperature"

-- ME31 Modbus settings
local SLAVE_ADDRESS  = 2       -- Modbus slave address
local TEMP_REG_START = 400     -- 0x0190: CH1 temperature integer register
local USE_RTU        = true    -- true: read via RTU (connection 0), false: read via TCP (connection 1)

-- Timing
local READ_INTERVAL  = 5000   -- read temperature every 5 seconds
local FLUSH_INTERVAL = 30000  -- flush batch every 30 seconds
local timer_counter  = 0
local read_counter   = 0

-- Batch buffer
local pending_lines = {}

function on_start()
    if INFLUXDB_TOKEN == "" then
        log("warn", "INFLUXDB_TOKEN is not set – writes will return 401")
    end
    local mode = USE_RTU and "RTU (connection 0)" or "TCP (connection 1)"
    log("info", string.format("ME31 temperature reader: %s, slave=%d", mode, SLAVE_ADDRESS))
    log("info", string.format("InfluxDB v3 target: %s  db=%s", INFLUXDB_URL, INFLUXDB_DB))
end

function on_receive()
    -- Read codec-parsed register value instead of parsing raw payload directly.
    -- The Modbus RTU codec creates values with ID: "modbus_rtu_{slave}:input_{30001 + register_address}"
    -- The Modbus TCP codec creates values with ID: "modbus_tcp_{slave}:input_{30001 + register_address}"
    -- For slave address 2 and register 400: input register number = 30001 + 400 = 30401
    local register_num = 30001 + TEMP_REG_START
    local raw_value
    if USE_RTU then
        raw_value = message:get_value(string.format("modbus_rtu_%d:input_%d", SLAVE_ADDRESS, register_num))
    else
        raw_value = message:get_value(string.format("modbus_tcp_%d:input_%d", SLAVE_ADDRESS, register_num))
    end

    if raw_value == nil then
        return false
    end

    -- Convert unsigned 16-bit to signed: values >= 0x8000 are negative
    if raw_value >= 0x8000 then
        raw_value = raw_value - 0x10000
    end

    -- -999 means sensor not connected
    if raw_value == -999 then
        log("warn", "CH1 sensor not connected (value = -999)")
        return false
    end

    local temp = raw_value / 10.0
    log("info", string.format("CH1 temperature: %.1f°C (raw=%d)", temp, raw_value))

    -- Add chart value
    message:add_float_value("CH1_Temp", temp)

    -- Async single write for real-time dashboard
    influxdb_write_v3_async(INFLUXDB_URL, INFLUXDB_TOKEN, INFLUXDB_DB,
        string.format("temperature,sensor=me31,channel=ch1 value=%.1f", temp),
        "auto", true, true)

    -- Buffer for batch flush
    pending_lines[#pending_lines + 1] =
        string.format("temperature,sensor=me31,channel=ch1 value=%.1f", temp)

    return true
end

-- Periodically send Modbus read request and flush InfluxDB batch
function on_timer(elapsed_ms)
    timer_counter = timer_counter + 100
    read_counter = read_counter + 100

    -- Send Modbus read request at READ_INTERVAL
    if read_counter >= READ_INTERVAL then
        read_counter = 0
        if USE_RTU then
            modbus_rtu_read_input_registers(SLAVE_ADDRESS, TEMP_REG_START, 1, 0, 0)
        else
            modbus_tcp_read_input_registers(TEMP_REG_START, 1, 0, 1)
        end
    end

    -- Flush batch at FLUSH_INTERVAL
    if timer_counter >= FLUSH_INTERVAL then
        timer_counter = 0
        if #pending_lines == 0 then
            return
        end
        local ok = influxdb_batch_write_v3_async(
            INFLUXDB_URL, INFLUXDB_TOKEN, INFLUXDB_DB,
            pending_lines, "auto", true, true)
        if ok then
            log("info", string.format("[InfluxDB v3] batch flush queued: %d lines", #pending_lines))
        else
            log("error", "[InfluxDB v3] batch flush queuing failed")
        end
        pending_lines = {}
    end
end

function on_stop()
    if #pending_lines == 0 then
        log("info", "ME31 temperature writer stopped (no pending data)")
        return
    end
    local line_count = #pending_lines
    local ok, result = pcall(influxdb_batch_write_v3,
        INFLUXDB_URL, INFLUXDB_TOKEN, INFLUXDB_DB,
        pending_lines, "auto", true, false)
    pending_lines = {}
    if not ok then
        log("error", "[InfluxDB v3] shutdown flush failed: " .. tostring(result))
        return
    end
    if result == 204 then
        log("info", string.format("[InfluxDB v3] shutdown flush: %d lines OK", line_count))
    else
        log("warn", string.format("[InfluxDB v3] shutdown flush: %d lines, HTTP %d", line_count, result))
    end
end

--[[
{
  "version": "1.0.0",
  "name": "ME31 PT100 Temperature to InfluxDB",
  "description": "Read PT100 temperature from ME31 module via Modbus RTU or TCP, publish to InfluxDB v3",
  "configs": [
    {
      "app": {
        "app_transport": "serial",
        "app_codec": "modbus_rtu_codec",
        "app_transformer": "disable",
        "app_encoding": "UTF-8"
      },
      "serial": {
        "serial_port": "/dev/ttyUSB0",
        "serial_baud_rate": 9600,
        "serial_data_bits": 8,
        "serial_parity": "none",
        "serial_stop_bits": "1",
        "serial_flow_control": "none"
      },
      "modbus_rtu_codec": {
        "with_receive_timeout": 20
      }
    },
    {
      "app": {
        "app_transport": "tcp_client",
        "app_codec": "modbus_tcp_codec",
        "app_transformer": "disable",
        "app_encoding": "UTF-8"
      },
      "tcp_client": {
        "tcp_client_host": "192.168.7.7",
        "tcp_client_port": 502,
        "tcp_client_timeout": 5000,
        "tcp_client_keepalive": true,
        "tcp_client_nodelay": true
      },
      "modbus_tcp_codec": {
        "unit_id": 2
      }
    }
  ],
  "message_input_groups": [
    {
      "key": "default",
      "name": "Default",
      "inputs": [
        {
          "type": "modbus_rtu",
          "id": "dba8ae4f-ead5-4548-add0-d37d40796c85",
          "name": "TempRTU",
          "slave_address": 2,
          "function_code": "read_input_registers",
          "start_address": 400,
          "quantity": 1,
          "data_value": "",
          "connection_id": 0,
          "start_address_hex_mode": false,
          "data_value_hex_mode": true
        },
        {
          "type": "modbus_tcp",
          "id": "af3d5e3b-24e8-4e84-bd44-6d410457144f",
          "name": "TempTCP",
          "function_code": "read_input_registers",
          "start_address": 400,
          "quantity": 1,
          "data_value": "",
          "connection_id": 1,
          "data_value_hex_mode": true
        }
      ]
    }
  ]
}
]]
