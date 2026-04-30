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
  -- `message` has 32 bytes frame and payload of 26 bytes, we should access the payload directly
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
  log("info", values_json)

  -- Return true because we added values to the message
  return true
end

--[[
{
  "version": "2.0.0",
  "name": "PMS9103M Air Quality Sensor",
  "description": "PMS9103M Air Quality Sensor Parser",
  "configs": [
    {
      "app": {
        "app_transport": "serial_port_transport",
        "app_codec": "frame_codec",
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
      "frame_codec": {
        "frame_codec_prefix": "42 4d",
        "frame_codec_header_size": 0,
        "frame_codec_tailer_length": 0,
        "frame_codec_suffix": "",
        "frame_codec_length_mode": "u16_be",
        "frame_codec_fixed_payload_size": 32,
        "frame_codec_length_meaning": "payload_checksum",
        "frame_codec_checksum_algo": "sum16_be",
        "frame_codec_checksum_scope": "prefix_header_length_payload"
      }
    }
  ],
  "dashboards": [
    {
      "widgets": [
        {
          "id": "27J3J04F",
          "name": "PM2.5",
          "widget_type": "lineChart",
          "colspan": 6,
          "rowspan": 2,
          "lines": [
            {
              "data_value_id": "PM2.5-CF1",
              "label": "PM2.5-CF1",
              "color": 4282557941,
              "width": 2,
              "dash_pattern": "solid",
              "unit": ""
            }
          ]
        },
        {
          "id": "27F0N3OK",
          "name": "PM1.0-CF1",
          "widget_type": "lineChart",
          "colspan": 6,
          "rowspan": 2,
          "lines": [
            {
              "data_value_id": "PM1.0-CF1",
              "label": "PM1.0-CF1",
              "color": 4294675456,
              "width": 2,
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
