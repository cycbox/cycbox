# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

This guide explains how to use the CycBox MCP server to configure and debug IoT device connections. CycBox connects to
devices via serial, TCP, WebSocket, MQTT, and UDP, then processes data through codecs, transformers, and LuaJIT scripts.

## Connection Model

Each connection:

```
Transport (required) → Codec (conditional) → Transformer (optional)
```

- **Byte-stream transports** (serial, tcp_client, tcp_server, udp) **require** a codec
- **Message-based transports** (websocket_client, websocket_server, mqtt_client) do not require a codec
- **Transformer** is always optional

Multiple connections can run simultaneously, sharing one Lua VM.

## Configuration Workflow

1. `get_config_schema(["app"])` → see available transports/codecs/transformers
2. `get_config_schema(["serial_port_transport", "frame_codec", ...])` → get field schemas for chosen components
3. `get_configs()` → read current state
4. `set_configs(...)` → apply changes
5. Start/restart engine (via tool if available, or ask user to apply in UI)

## Lua Scripting (LuaJIT / Lua 5.1)

One LuaJIT VM is shared across all connections — use `message.connection_id` to distinguish sources.

### Hook Functions

| Hook                     | When Called                               | Return                         |
|--------------------------|-------------------------------------------|--------------------------------|
| `on_start()`             | Connection starts or script reloads       | —                              |
| `on_receive()`           | Each RX message after codec/transform     | `true` if values were added    |
| `on_send()`              | Before TX (before encoding)               | `true` to proceed with sending |
| `on_send_confirm()`      | After successful TX                       | `true` if message was modified |
| `on_timer(timestamp_ms)` | Every 100ms; current Unix timestamp in ms | —                              |
| `on_stop()`              | Before shutdown or script reload          | —                              |

### Message API

```lua
-- Properties (read/write unless noted)
message.payload          -- Pure data content (protocol overhead removed)
message.frame            -- Complete wire format with all protocol overhead
message.checksum_valid   -- Whether frame checksum is valid (read-only, defaults true)
message.connection_id    -- Source/destination connection ID (0-based)
message.timestamp        -- Unix timestamp in μs (read-only)
message.values_json      -- All values as JSON string (read-only)

-- Methods
message:add_int_value(id, value [, timestamp_us])     -- Add 64-bit integer value
message:add_float_value(id, value [, timestamp_us])   -- Add 64-bit float value
message:add_string_value(id, value [, timestamp_us])  -- Add string value
message:add_bool_value(id, value [, timestamp_us])    -- Add boolean value
message:get_value(id)                -- Retrieve a value by ID
```

**Payload vs Frame:**

- **payload** = Pure data after codec removes framing (use for parsing in `on_receive()`, building in `on_send()`)
- **frame** = Complete bytes on the wire (use for debugging protocol issues)

| Codec       | Frame Example                                    | Payload Example                     |
|-------------|--------------------------------------------------|-------------------------------------|
| Line        | `"Hello\n"` (with delimiter)                     | `"Hello"` (without delimiter)       |
| Modbus RTU  | `[01 03 00 0A 00 02 C5 CD]` (with CRC)           | `[01 03 00 0A 00 02]` (without CRC) |
| Modbus TCP  | `[00 01...01 03 00 0A 00 02]` (with MBAP header) | `[03 00 0A 00 02]` (without header) |
| Frame Codec | `[42 4D 00 1C <data> 03 B0]` (full frame)        | `<data>` (just the data section)    |

### LuaJIT 5.1 Constraints (CRITICAL)

**Bitwise operations** — native operators (`&`, `|`, `~`, `<<`, `>>`) do NOT exist:

```lua
bit.band(a, b)     -- AND (not a & b)
bit.bor(a, b)      -- OR  (not a | b)
bit.bxor(a, b)     -- XOR (not a ~ b)
bit.bnot(a)        -- NOT (not ~a)
bit.lshift(a, n)   -- Left shift  (not a << n)
bit.rshift(a, n)   -- Right shift (not a >> n)
-- All bit ops work on 32-bit integers only
```

**Missing Lua 5.3+ features**:

- No `string.pack()` / `string.unpack()` — use `read_*` helpers or manual `string.byte()` + `bit.*`
- No integer division `//` — use `math.floor(a / b)`
- No integer type — `1` and `1.0` are identical (64-bit doubles)

**String building** — never concatenate in loops (`s = s .. data`), use table + `table.concat()`:

```lua
local parts = {}
for i = 1, n do parts[#parts + 1] = chunk end
local result = table.concat(parts)
```

**Runtime behavior**:

- Scripts are hot-reloaded (new VM created, all globals lost)
- Multiple connections share one VM — always check `message.connection_id`

## IoT Debugging Workflow

### Phase 1: Discovery

1. **Understand the task**: new device setup, troubleshooting, or adding functionality?
2. **Analyze device datasheet**: extract connection parameters, protocol structure, register maps, timing, CRC methods
3. **Read current state**: `get_configs` and `get_lua_script`
4. **Clarify gaps**: ask user about missing parameters (port, baud rate, IP, etc.)

### Phase 2: Design & Implement

5. **Study schemas and examples**: `get_config_schema` + read relevant `cycbox://example/*` resources
6. **Configure connections**: transport, codec (for byte-stream), and optional transformer
7. **Write Lua script** if custom parsing or message routing is needed

### Phase 3: Deploy & Verify

8. **Apply changes**:
    - `set_configs` to update connection configurations
    - `set_lua_script` to update script (syntax validated)
    - If `start_engine` / `restart_engine` / `reload_lua_script` tools are available, use them directly. Otherwise, ask
      user to apply in CycBox UI.

9. **Debug and iterate**:
    - `get_logs` to retrieve script output and hook errors
    - Refine with `set_lua_script` then `reload_lua_script` (or ask user to reload)
    - Adjust codec parameters with `set_configs` then `restart_engine` (or ask user to apply)

10. **Ask user to verify** in CycBox UI: connection status, incoming messages, parsed values
