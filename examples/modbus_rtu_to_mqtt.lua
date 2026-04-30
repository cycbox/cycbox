-- MA01-AXCX4020 DI/DO Polling, MQTT & Hardware Toggle Script
-- Uses serial_port_transport with modbus_rtu_codec to poll IO states,
-- and mqtt_transport to publish states and receive commands.
-- Queries device info (Model & Firmware) once at startup.
-- Polls 2 DIs and 2 DOs every 1000ms, publishes to "cycbox/state" as JSON.
-- Adds local hardware toggle: DI1 (NO) toggles Green LED, DI2 (NC) toggles Red LED.
-- LEDs are mutually exclusive (only one on at a time) using a single Write Multiple Coils request.
--
-- Commands recognized (string payload in MQTT message):
--   "green_on"  : Turns Green LED (DO1) ON, Red LED (DO2) OFF
--   "green_off" : Turns Green LED (DO1) OFF
--   "red_on"    : Turns Red LED (DO2) ON, Green LED (DO1) OFF
--   "red_off"   : Turns Red LED (DO2) OFF
--
-- MA01-AXCX4020 Device
-- Slave Address: 32 (0x20)
-- Discrete 0x0000: DI1 (NO Button)
-- Discrete 0x0001: DI2 (NC Button)
-- Coil 0x0000: DO1 (Green LED)
-- Coil 0x0001: DO2 (Red LED)
-- Holding 0x07D0: Module Model (7 words)
-- Holding 0x07DC: Firmware Version (2 words)

local SLAVE_ID = 32
local POLL_INTERVAL_MS = 400
local timer_ms = 0
local MQTT_CONN_ID = 1

-- State tracking for edge detection and toggling
local last_di1 = nil
local last_di2 = nil
local current_do1 = false
local current_do2 = false

function on_start()
    log("info", "Starting MA01-AXCX4020 MQTT polling and toggle script")
    -- Query Device Info on startup
    -- 0x07D0: Module Model (7 words)
    modbus_rtu_read_holding_registers(SLAVE_ID, 0x07D0, 7, 100, 0)
    -- 0x07DC: Firmware Version (2 words)
    modbus_rtu_read_holding_registers(SLAVE_ID, 0x07DC, 2, 200, 0)
end

function on_timer(now_ms)
    timer_ms = timer_ms + 100
    
    if timer_ms >= POLL_INTERVAL_MS then
        -- Read 2 Discrete Inputs (DI1-DI2) starting at address 0x0000
        modbus_rtu_read_discrete_inputs(SLAVE_ID, 0x0000, 2, 0, 0)
        
        -- Read 2 Coils (DO1-DO2) starting at address 0x0000
        -- Stagger by 200ms to allow the device to respond to the previous request
        modbus_rtu_read_coils(SLAVE_ID, 0x0000, 2, 200, 0)
        
        timer_ms = 0
    end
end

function on_receive()
    -- 1. Handle MQTT Commands from connection 1
    if message.connection_id == MQTT_CONN_ID then
        local topic = message:get_metadata("mqtt_topic")
        if topic == "cycbox/commands" and message.payload then
            local cmd = string.lower(message.payload)
            log("info", "Received MQTT command: " .. cmd)
            
            -- Mutually exclusive logic: use write_multiple_coils to set both simultaneously
            if string.find(cmd, "green_on") then
                modbus_rtu_write_multiple_coils(SLAVE_ID, 0x0000, {true, false}, 0, 0)
                current_do1 = true
                current_do2 = false
            elseif string.find(cmd, "green_off") then
                modbus_rtu_write_multiple_coils(SLAVE_ID, 0x0000, {false, current_do2}, 0, 0)
                current_do1 = false
            elseif string.find(cmd, "red_on") then
                modbus_rtu_write_multiple_coils(SLAVE_ID, 0x0000, {false, true}, 0, 0)
                current_do1 = false
                current_do2 = true
            elseif string.find(cmd, "red_off") then
                modbus_rtu_write_multiple_coils(SLAVE_ID, 0x0000, {current_do1, false}, 0, 0)
                current_do2 = false
            end
        end
        return false
    end
    
    -- 2. Handle Modbus Responses from connection 0
    if message.connection_id == 0 then
        local modified = false
        
        -- Parse Device Info (Model Name)
        local model_parts = {}
        local has_model = false
        for i = 0, 6 do
            local reg = message:get_value(string.format("modbus_rtu_%d:holding_%04X", SLAVE_ID, 0x07D0 + i))
            if reg then
                has_model = true
                local h = bit.rshift(reg, 8)
                local l = bit.band(reg, 0xFF)
                if h > 0 then table.insert(model_parts, string.char(h)) end
                if l > 0 then table.insert(model_parts, string.char(l)) end
            end
        end
        if has_model and #model_parts > 0 then
            message:add_string_value("Module_Model", table.concat(model_parts))
            modified = true
        end

        -- Parse Firmware Version
        local fw1 = message:get_value(string.format("modbus_rtu_%d:holding_07DC", SLAVE_ID))
        local fw2 = message:get_value(string.format("modbus_rtu_%d:holding_07DD", SLAVE_ID))
        if fw1 and fw2 then
            message:add_string_value("Firmware_Version", string.format("%04X-%04X", fw1, fw2))
            modified = true
        end

        -- Codec automatically parses valid Modbus responses into values mapped by string ID
        local di1 = message:get_value(string.format("modbus_rtu_%d:discrete_0000", SLAVE_ID))
        local di2 = message:get_value(string.format("modbus_rtu_%d:discrete_0001", SLAVE_ID))
        local do1 = message:get_value(string.format("modbus_rtu_%d:coil_0000", SLAVE_ID))
        local do2 = message:get_value(string.format("modbus_rtu_%d:coil_0001", SLAVE_ID))

        -- Keep local state synced with actual hardware reads
        if do1 ~= nil then current_do1 = do1 end
        if do2 ~= nil then current_do2 = do2 end

        -- Local Hardware Toggle: DI1 (Normally Open) Edge Detection (false -> true)
        if di1 ~= nil then
            if last_di1 ~= nil and last_di1 == false and di1 == true then
                log("info", "DI1 NO button pressed, toggling Green LED")
                if current_do1 then
                    -- If ON, turn OFF
                    modbus_rtu_write_multiple_coils(SLAVE_ID, 0x0000, {false, current_do2}, 0, 0)
                    current_do1 = false
                else
                    -- If OFF, enforce mutual exclusion (turn Green ON, Red OFF)
                    modbus_rtu_write_multiple_coils(SLAVE_ID, 0x0000, {true, false}, 0, 0)
                    current_do1 = true
                    current_do2 = false
                end
            end
            last_di1 = di1
        end

        -- Local Hardware Toggle: DI2 (Normally Closed) Edge Detection (true -> false)
        if di2 ~= nil then
            if last_di2 ~= nil and last_di2 == true and di2 == false then
                log("info", "DI2 NC button pressed, toggling Red LED")
                if current_do2 then
                    -- If ON, turn OFF
                    modbus_rtu_write_multiple_coils(SLAVE_ID, 0x0000, {current_do1, false}, 0, 0)
                    current_do2 = false
                else
                    -- If OFF, enforce mutual exclusion (turn Green OFF, Red ON)
                    modbus_rtu_write_multiple_coils(SLAVE_ID, 0x0000, {false, true}, 0, 0)
                    current_do1 = false
                    current_do2 = true
                end
            end
            last_di2 = di2
        end

        local json_parts = {}

        if di1 ~= nil then
            message:add_bool_value("Button_NO_DI1", di1)
            table.insert(json_parts, string.format('"button_di1":%s', di1 and "true" or "false"))
            modified = true
        end
        if di2 ~= nil then
            message:add_bool_value("Button_NC_DI2", di2)
            table.insert(json_parts, string.format('"button_di2":%s', di2 and "true" or "false"))
            modified = true
        end
        if do1 ~= nil then
            message:add_bool_value("LED_Green_DO1", do1)
            table.insert(json_parts, string.format('"led_green":%s', do1 and "true" or "false"))
            modified = true
        end
        if do2 ~= nil then
            message:add_bool_value("LED_Red_DO2", do2)
            table.insert(json_parts, string.format('"led_red":%s', do2 and "true" or "false"))
            modified = true
        end

        -- Publish a combined JSON message to MQTT if any new values were parsed
        if modified and #json_parts > 0 then
            local json_payload = "{" .. table.concat(json_parts, ",") .. "}"
            mqtt_publish("cycbox/state", json_payload, 0, false, 0, MQTT_CONN_ID)
        end

        return modified
    end

    return false
end


--[[
{
  "version": "2.0.0",
  "name": "Modbus and MQTT Connection Setup",
  "description": "Added an MQTT connection to the existing Modbus RTU serial configuration.",
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
        "app_transport": "mqtt_transport",
        "app_codec": "timeout_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "mqtt_transport": {
        "mqtt_transport_broker_url": "mqtt://localhost:1883",
        "mqtt_transport_client_id": "cycbox-300JSYB6",
        "mqtt_transport_username": "",
        "mqtt_transport_password": "",
        "mqtt_transport_use_tls": false,
        "mqtt_transport_ca_path": "",
        "mqtt_transport_client_cert_path": "",
        "mqtt_transport_client_key_path": "",
        "mqtt_transport_subscribe_topics": "cycbox/commands",
        "mqtt_transport_subscribe_qos": 1
      },
      "timeout_codec": {
        "with_receive_timeout": 100
      }
    },
    {
      "app": {
        "app_transport": "mqtt_server_transport",
        "app_codec": "timeout_codec",
        "app_transformer": "disable_transformer",
        "app_encoding": "UTF-8"
      },
      "mqtt_server_transport": {
        "mqtt_server_transport_bind_address": "0.0.0.0",
        "mqtt_server_transport_bind_port": 1883,
        "mqtt_server_transport_use_tls": false,
        "mqtt_server_transport_tls_ca_path": "",
        "mqtt_server_transport_tls_cert_path": "",
        "mqtt_server_transport_tls_key_path": "",
        "mqtt_server_transport_credentials": "",
        "mqtt_server_transport_topic_filter": "#"
      },
      "timeout_codec": {
        "with_receive_timeout": 100
      }
    }
  ],
  "message_input_groups": [
    {
      "id": "304MXF5L",
      "name": "LED Control (Mutual Exclusivity)",
      "inputs": [
        {
          "input_type": "modbus_rtu",
          "id": "30GIGJPG",
          "name": "Switch to Green (DO1 ON, DO2 OFF)",
          "slave_address": 32,
          "function_code": "write_multiple_coils",
          "start_address": 0,
          "quantity": 2,
          "data_value": "01",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "30NJKR3Y",
          "name": "Switch to Red (DO1 OFF, DO2 ON)",
          "slave_address": 32,
          "function_code": "write_multiple_coils",
          "start_address": 0,
          "quantity": 2,
          "data_value": "02",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "303YFM7U",
          "name": "Turn All LEDs OFF",
          "slave_address": 32,
          "function_code": "write_multiple_coils",
          "start_address": 0,
          "quantity": 2,
          "data_value": "00",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "30R01Q22",
          "name": "Turn Green LED (DO1) OFF",
          "slave_address": 32,
          "function_code": "write_single_coil",
          "start_address": 0,
          "quantity": 0,
          "data_value": "0000",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "30IEPQDX",
          "name": "Turn Red LED (DO2) OFF",
          "slave_address": 32,
          "function_code": "write_single_coil",
          "start_address": 1,
          "quantity": 0,
          "data_value": "0000",
          "connection_id": 0
        }
      ]
    },
    {
      "id": "30G08IIH",
      "name": "Monitoring",
      "inputs": [
        {
          "input_type": "modbus_rtu",
          "id": "30NA4N5A",
          "name": "Read Button States (DI1-DI2)",
          "slave_address": 32,
          "function_code": "read_discrete_inputs",
          "start_address": 0,
          "quantity": 2,
          "data_value": "",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "30JSYK83",
          "name": "Read LED Status (DO1-DO2)",
          "slave_address": 32,
          "function_code": "read_coils",
          "start_address": 0,
          "quantity": 2,
          "data_value": "",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "30UEOZQQ",
          "name": "Read Button Counters",
          "slave_address": 32,
          "function_code": "read_holding_registers",
          "start_address": 2527,
          "quantity": 2,
          "data_value": "",
          "connection_id": 0
        }
      ]
    },
    {
      "id": "30AM24P4",
      "name": "MQTT Control",
      "inputs": [
        {
          "input_type": "mqtt",
          "id": "30VJBUTR",
          "name": "Green ON",
          "topic": "cycbox/commands",
          "qos": "at_least_once",
          "retain": false,
          "raw_value": "green_on",
          "is_hex": false,
          "connection_id": 1
        },
        {
          "input_type": "mqtt",
          "id": "3085L9WL",
          "name": "Red ON",
          "topic": "cycbox/commands",
          "qos": "at_least_once",
          "retain": false,
          "raw_value": "red_on",
          "is_hex": false,
          "connection_id": 1
        },
        {
          "input_type": "mqtt",
          "id": "30NT3964",
          "name": "Green OFF",
          "topic": "cycbox/commands",
          "qos": "at_least_once",
          "retain": false,
          "raw_value": "green_off",
          "is_hex": false,
          "connection_id": 1
        },
        {
          "input_type": "mqtt",
          "id": "30QHX02W",
          "name": "Red OFF",
          "topic": "cycbox/commands",
          "qos": "at_least_once",
          "retain": false,
          "raw_value": "red_off",
          "is_hex": false,
          "connection_id": 1
        }
      ]
    },
    {
      "id": "30S38ZFU",
      "name": "Device Information",
      "inputs": [
        {
          "input_type": "modbus_rtu",
          "id": "30KTYQ26",
          "name": "Read Model Name",
          "slave_address": 32,
          "function_code": "read_holding_registers",
          "start_address": 2000,
          "quantity": 7,
          "data_value": "",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "303SVWEC",
          "name": "Read Firmware Version",
          "slave_address": 32,
          "function_code": "read_holding_registers",
          "start_address": 2012,
          "quantity": 2,
          "data_value": "",
          "connection_id": 0
        },
        {
          "input_type": "modbus_rtu",
          "id": "30VXTIVL",
          "name": "Read Custom Module Name",
          "slave_address": 32,
          "function_code": "read_holding_registers",
          "start_address": 2014,
          "quantity": 10,
          "data_value": "",
          "connection_id": 0
        }
      ]
    }
  ],
  "dashboards": [
    {
      "widgets": [
        {
          "id": "309GNHYE",
          "name": "IO",
          "widget_type": "valueCard",
          "colspan": 3,
          "rowspan": 2,
          "items": [
            {
              "data_value_id": "modbus_rtu_32:coil_0000",
              "label": "DO1",
              "display_type": "boolean",
              "unit": "",
              "decimal_places": 2,
              "icon_code_point": 59126,
              "on_true_message_id": "30GIGJPG",
              "on_false_message_id": "303YFM7U"
            },
            {
              "data_value_id": "modbus_rtu_32:coil_0001",
              "label": "DO2",
              "display_type": "boolean",
              "unit": "",
              "decimal_places": 2,
              "icon_code_point": 57482,
              "on_true_message_id": "30NJKR3Y",
              "on_false_message_id": "303YFM7U"
            },
            {
              "data_value_id": "modbus_rtu_32:discrete_0000",
              "label": "DI1",
              "display_type": "boolean",
              "unit": "",
              "decimal_places": 2,
              "icon_code_point": 58925
            },
            {
              "data_value_id": "modbus_rtu_32:discrete_0001",
              "label": "DI2",
              "display_type": "boolean",
              "unit": "",
              "decimal_places": 2,
              "icon_code_point": 58924
            }
          ]
        }
      ]
    }
  ]
}
]]
