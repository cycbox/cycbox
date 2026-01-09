-- This example demonstrates a powerful multi-protocol bridge
-- Routes data from a serial port to both TCP and MQTT destinations.

-- ============================================================================
-- Configuration

-- id: "serial_assistant"
-- version: "1.8.0"
-- name: "Serial Assistant"
-- configs:
--   - # Config 0
--     app:
--       app_transport: serial
--       app_codec: frame_codec
--       app_transformer: disable
--       app_encoding: UTF-8
--     serial:
--       serial_port: /dev/ttyUSB0
--       serial_baud_rate: 9600
--       serial_data_bits: 8
--       serial_parity: none
--       serial_stop_bits: "1"
--       serial_flow_control: none
--     frame_codec:
--       frame_codec_prefix: 42 4d
--       frame_codec_header_size: 0
--       frame_codec_length_mode: u16_be
--       frame_codec_fixed_payload_size: 32
--       frame_codec_checksum_algo: sum16_be
--       frame_codec_checksum_scope: prefix_header_length_payload
--       frame_codec_tailer_length: 0
--       frame_codec_length_meaning: payload_checksum
--       frame_codec_suffix: ''
--   - # Config 1
--     app:
--       app_transport: tcp_client
--       app_codec: passthrough_codec
--       app_transformer: disable
--       app_encoding: UTF-8
--     tcp_client:
--       tcp_client_host: 127.0.0.1
--       tcp_client_port: 9000
--       tcp_client_timeout: 5000
--       tcp_client_keepalive: true
--       tcp_client_nodelay: true
--   - # Config 2
--     app:
--       app_transport: mqtt
--       app_codec: passthrough_codec
--       app_transformer: disable
--       app_encoding: UTF-8
--     mqtt:
--       mqtt_broker_url: "mqtt://broker.emqx.io:1883"
--       mqtt_client_id: cycbox_1bd79588-9987-4a21-987d-48eb449d225d
--       mqtt_username: ''
--       mqtt_password: ''
--       mqtt_use_tls: false
--       mqtt_ca_path: ''
--       mqtt_client_cert_path: ''
--       mqtt_client_key_path: ''
--       mqtt_subscribe_topics: cycbox/rx
--       mqtt_subscribe_qos: 0
-- ============================================================================

-- Called for each received message
-- Access global 'message' object to read/modify message fields
-- MUST return true if message was modified, false otherwise
function on_receive()
  local conn_id = message.connection_id
  if conn_id == 0 then
    local frame = message.frame
    send_after(frame, 0, 1)
  end
  local payload = message.payload
  if #payload == 26 then
    local pm2_5_cf1 = read_u16_be(payload, 3)
    message:add_int_value("PM2.5", pm2_5_cf1)
    local values_json = message.values_json
    mqtt_publish("cycbox/serial2mqtt", values_json, 0, false, 10, 2)
  end
  return true
end
