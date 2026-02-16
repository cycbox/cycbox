-- ntfy Push Notification Example
-- Demonstrates both blocking and async push notification sending via ntfy.sh or self-hosted ntfy
-- Documentation: https://docs.ntfy.sh/

-- Configuration
-- Option 1: Use public ntfy.sh (no registration required)
local NTFY_SERVER = "https://ntfy.sh"
-- Option 2: Use self-hosted ntfy server
-- local NTFY_SERVER = "https://ntfy.example.com"

-- Choose a unique topic name to avoid conflicts with other users
-- Anyone can subscribe to this topic, so use something hard to guess
local NTFY_TOPIC = "cycbox_alerts"  -- Replace with your unique topic name

function on_start()
    log("info", "ntfy push notification example started")

    -- Example 1: Basic notification (blocking, with error handling)
    local ok, err = ntfy_send({
        server = NTFY_SERVER,
        topic = NTFY_TOPIC,
        message = "Hello from CycBox Pro!"
    })
    if ok then
        log("info", "ntfy notification sent successfully")
    else
        log("error", "Failed to send ntfy notification: " .. (err or "unknown error"))
    end

    -- Example 2: Rich notification with title and priority (blocking)
    local ok2, err2 = ntfy_send({
        server = NTFY_SERVER,
        topic = NTFY_TOPIC,
        message = "Temperature sensor reading: 25°C",
        title = "Temperature Alert",
        priority = "default",
        tags = "thermometer"
    })
    if not ok2 then
        log("error", "Failed: " .. (err2 or "unknown"))
    end

    -- Example 3: Fire-and-forget notification (async, no error handling)
    local queued = ntfy_send_async({
        topic = NTFY_TOPIC,
        message = "Quick status update: All systems operational",
        priority = "low",
        tags = "information_source"
    })
    if queued then
        log("debug", "ntfy async notification queued")
    else
        log("warn", "Failed to queue ntfy async notification")
    end
end

--[[
id: "ntfy_example"
version: "1.0.0"
name: "ntfy Push Notification Example"
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
