-- CycBox Lua Script
-- Documentation: https://cycbox.io/docs/lua-script/

-- ME31-XDXX0400 4-way temperature acquisition module PT100 thermal resistor RS485 network port to Modbus K-type thermocouple

--[[
id: "serial_assistant"
version: "1.9.0"
name: "Serial Assistant"
configs:
  - # Config 0
    app:
      app_transport: serial
      app_codec: modbus_rtu_codec
      app_transformer: disable
      app_encoding: UTF-8
    serial:
      serial_port: /dev/ttyUSB0
      serial_baud_rate: 9600
      serial_data_bits: 8
      serial_parity: none
      serial_stop_bits: "1"
      serial_flow_control: none
    modbus_rtu_codec:
      with_receive_timeout: 20
  - # Config 1
    app:
      app_transport: tcp_client
      app_codec: modbus_tcp_codec
      app_transformer: disable
      app_encoding: UTF-8
    tcp_client:
      tcp_client_host: 192.168.3.7
      tcp_client_port: 502
      tcp_client_timeout: 5000
      tcp_client_keepalive: true
      tcp_client_nodelay: true
    modbus_tcp_codec:
      unit_id: 2
message_input_groups:
  - key: "default"
    name: "Default"
    inputs:
      -
        type: modbus_rtu
        id: dba8ae4f-ead5-4548-add0-d37d40796c85
        name: TempRTU
        slave_address: 2
        function_code: read_input_registers
        start_address: 400
        quantity: 4
        data_value: ''
        connection_id: 0
        start_address_hex_mode: false
        data_value_hex_mode: true
      -
        type: modbus_tcp
        id: af3d5e3b-24e8-4e84-bd44-6d410457144f
        name: TempTCP
        function_code: read_input_registers
        start_address: 400
        quantity: 4
        data_value: ''
        connection_id: 1
        data_value_hex_mode: true
      -
        type: modbus_rtu
        id: f6b2bc50-52e4-4e04-9d2b-062f9c59d73f
        name: ReadRTU
        slave_address: 2
        function_code: read_holding_registers
        start_address: 2014
        quantity: 10
        data_value: ''
        connection_id: 0
        start_address_hex_mode: false
        data_value_hex_mode: true
      -
        type: modbus_tcp
        id: 84b2685e-3872-4ec0-8754-82858f43e682
        name: WriteTCP
        function_code: write_multiple_registers
        start_address: 2014
        quantity: 10
        data_value: 43 79 63 42 6F 78 2D 76 31 2E 31 30 2E 30 00 00 00 00 00 00
        connection_id: 1
        data_value_hex_mode: true
]]
