-- PMS9103M Air Quality Sensor Parser
-- This script parses PMS9103M particulate matter sensor data
--
-- Frame format:
--   Prefix: 0x42 0x4D ("BM")
--   Length: 2 bytes (big-endian, length of payload + checksum)
--   Payload: 26 bytes of sensor data
--   Checksum: 2 bytes (Sum16 big-endian)
--
-- Payload structure (all u16 big-endian):
--   Bytes 0-1:   PM1.0 concentration (CF=1) in μg/m³
--   Bytes 2-3:   PM2.5 concentration (CF=1) in μg/m³
--   Bytes 4-5:   PM10 concentration (CF=1) in μg/m³
--   Bytes 6-7:   PM1.0 concentration (atmospheric) in μg/m³
--   Bytes 8-9:   PM2.5 concentration (atmospheric) in μg/m³
--   Bytes 10-11: PM10 concentration (atmospheric) in μg/m³
--   Bytes 12-13: Particles >0.3μm per 0.1L air
--   Bytes 14-15: Particles >0.5μm per 0.1L air
--   Bytes 16-17: Particles >1.0μm per 0.1L air
--   Bytes 18-19: Particles >2.5μm per 0.1L air
--   Bytes 20-21: Particles >5.0μm per 0.1L air
--   Bytes 22-23: Particles >10μm per 0.1L air

function on_receive()
  -- Access the payload directly as a field (not a method)
  if message.connection_id ~=0 then
    return false
  end
  local payload = message.payload

  -- PMS9103M payload should be 26 bytes
  if #payload ~= 26 then
    log("warn", string.format("PMS9103M payload should be 26 bytes, got %d", #payload))
    return false
  end

  -- Parse PM concentrations (CF=1, standard particles) in μg/m³
  local pm1_0_cf1 = read_u16_be(payload, 1)   -- offset 1 = byte 0 in 0-indexed
  local pm2_5_cf1 = read_u16_be(payload, 3)
  local pm10_cf1 = read_u16_be(payload, 5)

  -- Parse PM concentrations (atmospheric environment) in μg/m³
  local pm1_0_atm = read_u16_be(payload, 7)
  local pm2_5_atm = read_u16_be(payload, 9)
  local pm10_atm = read_u16_be(payload, 11)

  -- Parse particle counts (number of particles per 0.1L air)
  local particles_0_3um = read_u16_be(payload, 13)
  local particles_0_5um = read_u16_be(payload, 15)
  local particles_1_0um = read_u16_be(payload, 17)
  local particles_2_5um = read_u16_be(payload, 19)
  local particles_5_0um = read_u16_be(payload, 21)
  local particles_10um = read_u16_be(payload, 23)

  -- Add PM1.0 values to chart (CF=1 vs Atmospheric)
  message:add_int_value("PM1.0-CF1", pm1_0_cf1)
  message:add_int_value("PM1.0-ATM", pm1_0_atm)

  -- Add PM2.5 values to chart (CF=1 vs Atmospheric)
  message:add_int_value("PM2.5-CF1", pm2_5_cf1)
  message:add_int_value("PM2.5-ATM", pm2_5_atm)

  -- Add PM10 values to chart (CF=1 vs Atmospheric)
  message:add_int_value("PM10-CF1", pm10_cf1)
  message:add_int_value("PM10-ATM", pm10_atm)

  -- Add particle counts to chart
  message:add_int_value("Particles >0.3μm", particles_0_3um)
  message:add_int_value("Particles >0.5μm", particles_0_5um)
  message:add_int_value("Particles >1.0μm", particles_1_0um)
  message:add_int_value("Particles >2.5μm", particles_2_5um)
  message:add_int_value("Particles >5.0μm", particles_5_0um)
  message:add_int_value("Particles >10μm", particles_10um)

  -- Log parsed data
  local values_json = message.values_json
  mqtt_publish("cycbox/pms9013m", values_json, 1, false, 0, 1)
  log("info", values_json)

  -- Return true because we added values to the message
  return true
end

--[[
id: "serial_assistant"
version: "1.8.1"
name: "串口调试助手"
configs:
  - # Config 0
    app:
      app_transport: serial
      app_codec: frame_codec
      app_transformer: disable
      app_encoding: UTF-8
    serial:
      serial_port: /dev/ttyUSB0
      serial_baud_rate: 9600
      serial_data_bits: 8
      serial_parity: none
      serial_stop_bits: "1"
      serial_flow_control: none
    frame_codec:
      frame_codec_prefix: 42 4d
      frame_codec_header_size: 0
      frame_codec_tailer_length: 0
      frame_codec_suffix: ''
      frame_codec_length_mode: u16_be
      frame_codec_fixed_payload_size: 32
      frame_codec_length_meaning: payload_checksum
      frame_codec_checksum_algo: sum16_be
      frame_codec_checksum_scope: prefix_header_length_payload
  - # Config 1
    app:
      app_transport: mqtt
      app_codec: timeout_codec
      app_transformer: disable
      app_encoding: UTF-8
    mqtt:
      mqtt_broker_url: "mqtt://broker.emqx.io:1883"
      mqtt_client_id: cycbox_a4290f76-ab3c-4ed1-906e-7cf8807a3b8f
      mqtt_username: ''
      mqtt_password: ''
      mqtt_use_tls: false
      mqtt_ca_path: ''
      mqtt_client_cert_path: ''
      mqtt_client_key_path: ''
      mqtt_subscribe_topics: "cycbox/#"
      mqtt_subscribe_qos: 1
    timeout_codec:
      with_receive_timeout: 100
message_input_groups:
  - key: "default"
    name: "Default"
    inputs:
      -
        type: single
        id: 39648271-f8dc-4114-b1ab-67354d3de995
        name: Message
        text: ''
        is_hex_mode: false
        auto_append: none
        connection_id: 0
]]
