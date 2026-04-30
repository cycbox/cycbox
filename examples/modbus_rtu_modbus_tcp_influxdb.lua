-- ME31-XDXX0400 PT100 Temperature Monitor
-- Configured for Modbus RTU (serial) and Modbus TCP. A flag toggles between the two.
-- Polls CH1 temperature every 1 second, applies an exponential moving average (EMA) filter,
-- and saves the filtered data to InfluxDB every 10 seconds. Device info is read once at startup.
--
-- Device: ME31-XDXX0400 4-Channel PT100 Module
-- Protocol: Modbus RTU (Baud: 9600 8N1) or Modbus TCP
-- Modbus Address: Slave 1 (RTU), Unit 2 (TCP, per config)
--
-- Register Map:
--   0x0190  CH1_TEMP_INT  Input Reg  Temperature integer (*0.1 °C). 0xFC19 (-999) = Disconnected
--   0x07DC  FW_VERSION    Holding    Firmware Version

local CONNECTION_MODE = "RTU" -- Change to "TCP" to use Modbus TCP
local CONN_ID = (CONNECTION_MODE == "TCP") and 1 or 0
local SLAVE_ADDR = 2
local TCP_UNIT_ID = 2

local POLL_MS = 1000
local INFLUX_INTERVAL_MS = 10000
local ALPHA = 0.2 -- Smoothing factor for EMA filter

local last_poll_ms = 0
local last_influx_ms = 0
local filtered_temp = nil

local INFLUX_URL = get_env("INFLUX_URL") or "http://localhost:8181"
local INFLUX_TOKEN = get_env("INFLUX_TOKEN") or "your-token"
local INFLUX_DB = "cycbox"

function on_start()
    log("info", "Starting ME31-XDXX0400 monitoring. Mode: " .. CONNECTION_MODE)

    -- Read Firmware Version at startup
    if CONNECTION_MODE == "TCP" then
        modbus_tcp_read_holding_registers(0x07DC, 1, 100, CONN_ID)
    else
        modbus_rtu_read_holding_registers(SLAVE_ADDR, 0x07DC, 1, 100, CONN_ID)
    end
end

function on_timer(now_ms)
    -- Poll CH1 Temperature
    if now_ms - last_poll_ms >= POLL_MS then
        if CONNECTION_MODE == "TCP" then
            modbus_tcp_read_input_registers(0x0190, 1, 0, CONN_ID)
        else
            modbus_rtu_read_input_registers(SLAVE_ADDR, 0x0190, 1, 0, CONN_ID)
        end
        last_poll_ms = now_ms
    end

    -- Save to InfluxDB periodically
    if now_ms - last_influx_ms >= INFLUX_INTERVAL_MS then
        if filtered_temp ~= nil then
            local line_data = string.format("me31_temp,channel=1 temperature=%.2f", filtered_temp)
            influxdb_write_v3_async(INFLUX_URL, INFLUX_TOKEN, INFLUX_DB, line_data, "auto", true, false)
        end
        last_influx_ms = now_ms
    end
end

function on_receive()
    -- Only process messages from the active connection
    if message.connection_id ~= CONN_ID then return false end
    local modified = false

    -- Check Firmware Version
    local fw_key = (CONNECTION_MODE == "TCP") and string.format("modbus_tcp_%d:holding_07DC", TCP_UNIT_ID)
                                              or string.format("modbus_rtu_%d:holding_07DC", SLAVE_ADDR)
    local fw_ver = message:get_value(fw_key)
    if fw_ver then
        message:add_int_value("Firmware_Version", fw_ver)
        modified = true
    end

    -- Check CH1 Temperature
    local temp_key = (CONNECTION_MODE == "TCP") and string.format("modbus_tcp_%d:input_0190", TCP_UNIT_ID)
                                                or string.format("modbus_rtu_%d:input_0190", SLAVE_ADDR)
    local raw_temp = message:get_value(temp_key)
    if raw_temp then
        -- Convert uint16 to int16 (Two's complement)
        if raw_temp > 32767 then
            raw_temp = raw_temp - 65536
        end

        if raw_temp == -999 then
            log("warn", "CH1 Sensor Disconnected")
        else
            local temp_c = raw_temp * 0.1

            -- Apply EMA smoothing filter
            if filtered_temp == nil then
                filtered_temp = temp_c
            else
                filtered_temp = ALPHA * temp_c + (1 - ALPHA) * filtered_temp
            end

            message:add_float_value("CH1_Temperature_Filtered", filtered_temp)
            message:add_float_value("CH1_Temperature_Raw", temp_c)
            modified = true
        end
    end

    return modified
end


--[[
{
  "version": "2.0.0",
  "name": "Dual Modbus Acquisition (RTU/TCP)",
  "description": "Configures serial RTU on /dev/ttyUSB0 and TCP client at 192.168.7.7 for multi-mode sensor polling.",
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
    },
    {
      "app": {
        "app_transport": "tcp_client_transport",
        "app_codec": "modbus_tcp_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "tcp_client_transport": {
        "tcp_client_transport_host": "192.168.7.7",
        "tcp_client_transport_port": 502,
        "tcp_client_transport_timeout": 5000,
        "tcp_client_transport_keepalive": true,
        "tcp_client_transport_nodelay": true
      },
      "modbus_tcp_codec": {
        "unit_id": 2
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "308NP7DN",
      "name": "Temperature Monitoring (CH1)",
      "inputs": [
        {
          "input_type": "modbus_rtu",
          "id": "30XFTQQ3",
          "name": "Read CH1 Temp (Integer) - RTU",
          "slave_address": 2,
          "function_code": "read_input_registers",
          "start_address": 400,
          "quantity": 1,
          "data_value": "",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "30S7L68I",
          "name": "Read CH1 Temp (Float) - RTU",
          "slave_address": 2,
          "function_code": "read_input_registers",
          "start_address": 450,
          "quantity": 2,
          "data_value": "",
          "connection_id": 0
        },
        {
          "input_type": "modbus_tcp",
          "id": "3041SW2V",
          "name": "Read CH1 Temp (Integer) - TCP",
          "function_code": "read_input_registers",
          "start_address": 400,
          "quantity": 1,
          "data_value": "",
          "connection_id": 1
        },
        {
          "input_type": "modbus_tcp",
          "id": "30IEJNCA",
          "name": "Read CH1 Temp (Float) - TCP",
          "function_code": "read_input_registers",
          "start_address": 450,
          "quantity": 2,
          "data_value": "",
          "connection_id": 1
        }
      ]
    },
    {
      "id": "30NAJVHH",
      "name": "Calibration (CH1 Offset)",
      "inputs": [
        {
          "input_type": "modbus_rtu",
          "id": "30LBUF2B",
          "name": "Read Offset - RTU",
          "slave_address": 2,
          "function_code": "read_holding_registers",
          "start_address": 9000,
          "quantity": 1,
          "data_value": "",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "30P2CN7H",
          "name": "Set Offset - RTU (0.1°C units)",
          "slave_address": 2,
          "function_code": "write_single_register",
          "start_address": 9000,
          "quantity": 0,
          "data_value": "0000",
          "connection_id": 0
        },
        {
          "input_type": "modbus_tcp",
          "id": "302W00AO",
          "name": "Read Offset - TCP",
          "function_code": "read_holding_registers",
          "start_address": 9000,
          "quantity": 1,
          "data_value": "",
          "connection_id": 1
        },
        {
          "input_type": "modbus_tcp",
          "id": "30I7AEE6",
          "name": "Set Offset - TCP (0.1°C units)",
          "function_code": "write_single_register",
          "start_address": 9000,
          "quantity": 0,
          "data_value": "0000",
          "connection_id": 1
        }
      ]
    },
    {
      "id": "30F24UI4",
      "name": "System Control",
      "inputs": [
        {
          "input_type": "modbus_rtu",
          "id": "30VWE1G2",
          "name": "Read Model & Version - RTU",
          "slave_address": 2,
          "function_code": "read_holding_registers",
          "start_address": 2000,
          "quantity": 13,
          "data_value": "",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "30GZLS47",
          "name": "Soft Restart - RTU",
          "slave_address": 2,
          "function_code": "write_single_register",
          "start_address": 2026,
          "quantity": 0,
          "data_value": "0001",
          "connection_id": 0
        },
        {
          "input_type": "modbus_tcp",
          "id": "30B45KN0",
          "name": "Read Model & Version - TCP",
          "function_code": "read_holding_registers",
          "start_address": 2000,
          "quantity": 13,
          "data_value": "",
          "connection_id": 1
        },
        {
          "input_type": "modbus_tcp",
          "id": "30KMFTZA",
          "name": "Soft Restart - TCP",
          "function_code": "write_single_register",
          "start_address": 2026,
          "quantity": 0,
          "data_value": "0001",
          "connection_id": 1
        }
      ]
    }
  ]
}
]]
