-- HTTP POST Example for CycBox Lua Scripting
-- Demonstrates fire-and-forget http_post() for outbound data reporting.
-- http_post() returns immediately; non-200 responses and errors are logged automatically.

-- Configuration
local REPORT_URL = "https://httpbin.org/post"

function on_start()
    log("info", "HTTP Example Script Started")

    -- Example 1: POST with default Content-Type (application/octet-stream)
    local ok = http_post(REPORT_URL, "hello")
    log("info", "Raw POST queued: " .. tostring(ok))

    -- Example 2: POST JSON with custom headers
    local json = '{"sensor": "temperature", "value": 23.5}'
    local headers = {
        ["Content-Type"] = "application/json",
        ["X-Device-Id"] = "sensor-01"
    }
    ok = http_post(REPORT_URL, json, headers)
    log("info", "JSON POST queued: " .. tostring(ok))
end

-- Called when receiving messages from the transport.
-- Forward each received payload to the HTTP endpoint.
function on_receive(message)
    local data = message:get_data()
    if data and #data > 0 then
        http_post(REPORT_URL, data, {["Content-Type"] = "application/octet-stream"})
    end
    return false
end


--[[
id: "serial_assistant"
version: "1.10.0"
name: "Serial Assistant"
configs:
  - # Config 0
    app:
      app_transport: udp
      app_codec: timeout_codec
      app_transformer: disable
      app_encoding: UTF-8
    udp:
      udp_bind_address: 0.0.0.0
      udp_bind_port: 5000
      udp_enable_broadcast: false
      udp_enable_multicast: false
      udp_multicast_groups: |
        239.255.0.1
        ff02::1
      udp_multicast_ttl: 1
      udp_multicast_hop_limit: 1
      udp_multicast_loopback: true
    timeout_codec:
      with_receive_timeout: 100
message_input_groups:
  - key: "default"
    name: "Default"
    inputs:
      -
        type: single
        id: fa50dccc-d3f1-45f2-adbd-436ec5acfd83
        name: Message
        text: ''
        is_hex_mode: false
        auto_append: none
        connection_id: 0
]]
