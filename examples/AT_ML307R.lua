-- CycBox Lua Script
-- Documentation: https://cycbox.io/docs/lua-script/

-- ML307R 4G Cat1 communication module


--[[
id: "serial_assistant"
version: "1.9.0"
name: "Serial Assistant"
configs:
  - # Config 0
    app:
      app_transport: serial
      app_codec: at_codec
      app_transformer: disable
      app_encoding: UTF-8
    serial:
      serial_port: /dev/ttyUSB0
      serial_baud_rate: 9600
      serial_data_bits: 8
      serial_parity: none
      serial_stop_bits: "1"
      serial_flow_control: none
message_input_groups:
  - key: "default"
    name: "Default"
    inputs:
      -
        type: batch
        id: 359e184f-fe22-45bd-91d3-9ede78a1e283
        name: Device Info
        items: 
        - 
          message_input: 
            type: single
            id: b38d6b14-02b4-4134-a71c-9a7934f9ae0d
            name: Message
            text: AT
            is_hex_mode: false
            auto_append: none
            connection_id: 0
          delay_ms: 1000.0
        - 
          message_input: 
            type: single
            id: 026f8deb-ccbb-4926-aacd-0ff421d2c286
            name: Message
            text: AT+CGMR
            is_hex_mode: false
            auto_append: none
            connection_id: 0
          delay_ms: 1000.0
        - 
          message_input: 
            type: single
            id: 0988562d-f7a5-49b7-8c53-65a6e7c18613
            name: Message
            text: AT+CEREG?
            is_hex_mode: false
            auto_append: none
            connection_id: 0
          delay_ms: 1000.0
        - 
          message_input: 
            type: single
            id: 5377480b-6eea-44db-b3c4-21ba21b4cad1
            name: Message
            text: AT+CSQ
            is_hex_mode: false
            auto_append: none
            connection_id: 0
          delay_ms: 1000.0
        repeat: false
      -
        type: single
        id: eac0c0fa-578d-4d14-97aa-4a9cc278a409
        name: MQTT Info
        text: AT+MQTTCFG=?
        is_hex_mode: false
        auto_append: none
        connection_id: 0
      -
        type: single
        id: 27a5d1f3-695d-4fd0-90a1-8c727ebd3a29
        name: MQTT Connect
        text: AT+MQTTCONN=0,"broker.emqx.io",1883,"bfdba077fee0"
        is_hex_mode: false
        auto_append: none
        connection_id: 0
      -
        type: single
        id: cfdfe19f-c10f-439f-bcec-da7c73131147
        name: MQTT Pub
        text: AT+MQTTPUB=0,"cycbox",1,0,0,4,"3242"
        is_hex_mode: false
        auto_append: crlf
        connection_id: 0
      -
        type: single
        id: 4a85213d-2657-455d-ab01-33e589754c77
        name: MQTT Sub
        text: AT+MQTTSUB=0,"cycbox",1
        is_hex_mode: false
        auto_append: none
        connection_id: 0
]]
