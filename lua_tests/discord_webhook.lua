-- Discord Webhook
-- Demonstrates both blocking and async Discord webhook sending

-- Replace with your actual Discord webhook URL
local WEBHOOK_URL = "https://discord.com/api/webhooks/YOUR_WEBHOOK_ID/YOUR_WEBHOOK_TOKEN"

function on_start()
    log("info", "Discord webhook example started")

    -- Example 1: Basic message (blocking, with error handling)
    local ok, err = discord_send(WEBHOOK_URL, "Hello from CycBox Pro!", nil, nil)
    if ok then
        log("info", "Discord message sent successfully")
    else
        log("error", "Failed to send Discord message: " .. (err or "unknown error"))
    end

    -- Example 2: Message with custom username and avatar (blocking)
    local ok2, err2 = discord_send(
        WEBHOOK_URL,
        "This is a custom message!",
        "CycBox Bot",  -- custom username
        "https://example.com/avatar.png"  -- custom avatar URL
    )
    if not ok2 then
        log("error", "Failed: " .. (err2 or "unknown"))
    end

    -- Example 3: Fire-and-forget message (async, no error handling)
    local queued = discord_send_async(WEBHOOK_URL, "Quick notification!", nil, nil)
    if queued then
        log("debug", "Discord async message queued")
    else
        log("warn", "Failed to queue Discord async message")
    end
end

--[[
{
  "version": "1.10.0",
  "name": "Discord Webhook",
  "description": "Demonstrates both blocking and async Discord webhook sending",
  "configs": [
    {
      "app": {
        "app_transport": "udp",
        "app_codec": "timeout_codec",
        "app_transformer": "disable",
        "app_encoding": "UTF-8"
      },
      "udp": {
        "udp_bind_address": "0.0.0.0",
        "udp_bind_port": 5000,
        "udp_enable_broadcast": false,
        "udp_enable_multicast": false,
        "udp_multicast_groups": "239.255.0.1\nff02::1\n",
        "udp_multicast_ttl": 1,
        "udp_multicast_hop_limit": 1,
        "udp_multicast_loopback": true
      },
      "timeout_codec": {
        "with_receive_timeout": 100
      }
    }
  ]
}
]]
