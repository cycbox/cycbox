-- CycBox Lua Script
-- TCP CSV Client: Sends CSV lines with mixed value types every 100ms

math.randomseed(os.time())

local counter = 0
local temperature = 25.0
local humidity = 50
local string_pool = {"hello", "world", "foo", "bar", "cycbox", "test", "ok", "error", "running", "idle"}

function on_timer(timestamp_ms)
    counter = counter + 1

    temperature = temperature + (math.random(-5, 5) / 10.0)
    temperature = math.max(-40, math.min(80, temperature))
    humidity = humidity + math.random(-1, 1)
    humidity = math.max(0, math.min(100, humidity))
    local word = string_pool[math.random(1, #string_pool)]
    local active = math.random(0, 1) == 1

    -- CSV line: integer, float, integer, string, boolean
    local csv = string.format(
        '%d,%.1f,%d,%s,%s',
        counter, temperature, humidity, word, tostring(active)
    )

    send_after(csv, 0, 0)
end

--[[
{
  "version": "1.11.0",
  "name": "TCP CSV Client",
  "description": "Sends CSV lines with mixed value types every 100ms",
  "configs": [
    {
      "app": {
        "app_transport": "tcp_client_transport",
        "app_codec": "line_codec",
        "app_transformer": "csv_transformer",
        "app_encoding": "UTF-8"
      },
      "tcp_client_transport": {
        "tcp_client_transport_host": "127.0.0.1",
        "tcp_client_transport_port": 8080,
        "tcp_client_transport_timeout": 5000,
        "tcp_client_transport_keepalive": true,
        "tcp_client_transport_nodelay": true
      },
      "line_codec": {
        "line_codec_line_ending": "lf"
      }
    }
  ]
}
]]
