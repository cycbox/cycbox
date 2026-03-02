-- CycBox Lua Script
-- TCP CSV Client: Sends CSV lines with mixed value types every 100ms

math.randomseed(os.time())

local counter = 0
local temperature = 25.0
local humidity = 50
local string_pool = {"hello", "world", "foo", "bar", "cycbox", "test", "ok", "error", "running", "idle"}

function on_timer(elapsed_ms)
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
id: "tcp_csv_client"
version: "1.11.0"
name: "TCP CSV Client"
configs:
  - # Config 0
    app:
      app_transport: tcp_client
      app_codec: line_codec
      app_transformer: csv_transformer
      app_encoding: UTF-8
    tcp_client:
      tcp_client_host: 127.0.0.1
      tcp_client_port: 9000
      tcp_client_timeout: 5000
      tcp_client_keepalive: true
      tcp_client_nodelay: true
    line_codec:
      line_codec_line_ending: lf
message_input_groups:
  - key: "default"
    name: "Default"
    inputs:
      -
        type: single
        id: 1dfe021f-8b75-4d91-9815-8c3ee0a2ad34
        name: Message
        text: ''
        is_hex_mode: false
        auto_append: none
        connection_id: 0
]]
