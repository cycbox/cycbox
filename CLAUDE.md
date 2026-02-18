# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

This guide explains how to use the CycBox MCP server to configure and debug IoT device connections. CycBox connects to
devices via serial, TCP, WebSocket, MQTT, and UDP, then processes data through codecs, transformers, and LuaJIT scripts.

**IMPORTANT**: The MCP server provides read/write access to configurations and scripts. If the user has enabled agent
engine control and/or agent Lua reload permissions, you can directly start/stop the engine and reload scripts. Otherwise,
lifecycle management is handled through the CycBox UI — ask the user to apply changes there.

## Tools & Resources

### Tools

**Configuration Management:**

| Tool          | Purpose                                                                               |
|---------------|---------------------------------------------------------------------------------------|
| `get_configs` | Read current connection configurations                                                |
| `set_configs` | Update connection configurations (user must apply in CycBox UI afterwards)            |

**Lua Script Management:**

| Tool             | Purpose                                                                            |
|------------------|------------------------------------------------------------------------------------|
| `get_lua_script` | Read the current Lua processing script                                             |
| `set_lua_script` | Set a new Lua script (syntax validated, user must reload in CycBox UI afterwards)  |

**Engine Control (permission-gated):**

| Tool                 | Purpose                                                                          |
|----------------------|----------------------------------------------------------------------------------|
| `start_engine`       | Start the engine with current config (requires agent engine control permission)  |
| `stop_engine`        | Stop the running engine (requires agent engine control permission)               |
| `restart_engine`     | Stop and restart the engine (requires agent engine control permission)           |
| `reload_lua_script`  | Hot-reload the Lua script without restarting connections (requires agent Lua reload permission) |

**Permission Requirements:**

- **Engine control tools** (`start_engine`, `stop_engine`, `restart_engine`): Require the user to enable "agent engine control" permission in CycBox UI
- **Lua reload tool** (`reload_lua_script`): Requires the user to enable "agent Lua reload" permission in CycBox UI
- If permission is denied, these tools return an error asking you to tell the user to perform the action in the CycBox UI

**Tool Behavior:**

- `start_engine`: Validates that engine is not already running, starts engine with stored configuration, waits 3 seconds, then returns with:
  - Status and human-readable message
  - Array of connection IDs that were started
  - Initial startup logs (max 100 entries) for debugging
- `stop_engine`: Validates that engine is running, stops all connections, returns success status
- `restart_engine`: Stops engine if running, starts with stored configuration, waits 3 seconds, then returns startup logs and connection IDs
- `reload_lua_script`: Validates that engine is running, hot-reloads Lua script from stored manifest, waits 3 seconds, then returns:
  - Status and human-readable message
  - Reload logs (max 100 entries) including any `on_start()` hook output

**Important Notes:**

- All lifecycle tools check engine state before proceeding (e.g., `start_engine` fails if already running)
- `start_engine` and `restart_engine` wait 3 seconds after starting to collect initial logs and verify connections
- `reload_lua_script` creates a new Lua VM (all globals lost, `on_start()` hook called again) but preserves connection state
- Use `get_logs` anytime to retrieve the most recent 100 log entries

**Debugging:**

| Tool       | Purpose                                                                 |
|------------|-------------------------------------------------------------------------|
| `get_logs` | Retrieve recent Lua script logs and hook errors (max 100 entries)       |

### Resources

| URI Pattern                        | Description                                      |
|------------------------------------|--------------------------------------------------|
| `cycbox://config-schema/{name}`    | Schema docs for each transport/codec/transformer |
| `cycbox://docs/en/lua-api/{topic}` | Lua scripting API documentation                  |
| `cycbox://examples/{name}/script`  | Example Lua scripts for common devices           |
| `cycbox://examples/{name}/config`  | Example JSON configurations matching the scripts |

**Available topics**: lua-api, hooks, message, global, data-reading, http, mqtt, modbus, udp, redis, influxdb,
timescaledb

**Available examples**: pms9103m, pms9103m_file, pms9103m_mqtt, pms9103m_influxdb2, pms9103m_influxdb3, pms9103m_redis,
pms9103m_timescaledb, modbus_eid041

## Connection Model

Each session supports **multiple simultaneous connections**. Each connection has:

```
Transport (required) → Codec (conditional) → Transformer (optional)
```

- **Byte-stream transports** (serial, tcp_client, tcp_server, udp) **require** a codec for framing
- **Message-based transports** (websocket_client, websocket_server, mqtt_client) do not require a codec
- **Transformer** is always optional (numeric extraction, JSON parsing)

### Configuration Structure (ConfigToolCall)

```json
{
  "configs": [
    {
      "transport": {
        "name": "serial",
        "fields": {
          "serial_port": "/dev/ttyUSB0",
          "serial_baud_rate": 9600,
          ...
        }
      },
      "codec": {
        "name": "frame_codec",
        "fields": {
          "frame_codec_prefix": "42 4d",
          ...
        }
      },
      "transformer": null
    }
  ]
}
```

Always read the config schema resource (e.g. `cycbox://config-schema/serial`) for exact field names, types, defaults,
and options before building configurations.

## Lua Scripting (LuaJIT / Lua 5.1)

Scripts process messages in the data pipeline. One LuaJIT VM is shared across all connections — use
`message.connection_id` to distinguish sources.

### Hook Functions

| Hook                   | When Called                           | Return                         |
|------------------------|--------------------------------------------------|--------------------------------|
| `on_start()`           | Connection starts or script reloads              | —                              |
| `on_receive()`         | Each RX message after codec/transform            | `true` if values were added    |
| `on_send()`            | Before TX (before encoding)                      | `true` to proceed with sending |
| `on_send_confirm()`    | After successful TX by transport                 | `true` if message was modified |
| `on_timer(elapsed_ms)` | Every 100ms; elapsed_ms is ms since engine start | —                              |
| `on_stop()`            | Before shutdown or script reload                 | —                              |

### Message API

```lua
-- Properties (read/write unless noted)
message.payload          -- Pure data content (protocol overhead removed)
                         -- Examples: "Hello" (Line), [01 03 00 0A 00 02] (Modbus RTU w/o CRC)
message.frame            -- Complete wire format with all protocol overhead
                         -- Examples: "Hello\n" (Line), [01 03 00 0A 00 02 C5 CD] (Modbus RTU w/ CRC)
message.connection_id    -- Source/destination connection ID
message.timestamp        -- Unix timestamp in μs (read-only)
message.values_json      -- All values as JSON string (read-only)

-- Methods
message:add_int_value(id, value)     -- Add 64-bit integer value
message:add_float_value(id, value)   -- Add 64-bit float value
message:add_string_value(id, value)  -- Add string value
message:add_bool_value(id, value)    -- Add boolean value
message:get_value(id)                -- Retrieve a value by ID
```

**Payload vs Frame:**

- **payload** = Pure data after codec removes framing (use this for parsing in `on_receive()`, building in `on_send()`)
- **frame** = Complete bytes on the wire (use this for debugging protocol issues)

| Codec       | Frame Example                                    | Payload Example                     |
|-------------|--------------------------------------------------|-------------------------------------|
| Line        | `"Hello\n"` (with delimiter)                     | `"Hello"` (without delimiter)       |
| Modbus RTU  | `[01 03 00 0A 00 02 C5 CD]` (with CRC)           | `[01 03 00 0A 00 02]` (without CRC) |
| Modbus TCP  | `[00 01...01 03 00 0A 00 02]` (with MBAP header) | `[03 00 0A 00 02]` (without header) |
| Frame Codec | `[42 4D 00 1C <data> 03 B0]` (full frame)        | `<data>` (just the data section)    |

### Binary Data Readers (1-based offset)

```lua
read_u8(payload, offset)          read_i8(payload, offset)
read_u16_be(payload, offset)      read_u16_le(payload, offset)
read_i16_be(payload, offset)      read_i16_le(payload, offset)
read_u32_be(payload, offset)      read_u32_le(payload, offset)
read_i32_be(payload, offset)      read_i32_le(payload, offset)
read_float_be(payload, offset)    read_float_le(payload, offset)
read_double_be(payload, offset)   read_double_le(payload, offset)
```

### Global Functions

```lua
log(level, message)                          -- level: "debug"|"info"|"warn"|"error"
get_env(var_name)                            -- Read environment variable
get_connection_count()                       -- Total active connections
get_transport(connection_id)                 -- Transport type string
get_codec(connection_id)                     -- Codec type string
send_after(payload, delay_ms, connection_id) -- Schedule delayed send
send_repeating(payload, interval_ms, conn_id) -- Returns batch_id
stop_repeating(batch_id)                     -- Stop repeating send

-- Protocol helpers (read docs resources for full signatures)
mqtt_publish(topic, payload, qos, retain, delay_ms, connection_id)
modbus_rtu_read_input_registers(slave, start, qty, delay, connection_id)
modbus_rtu_read_holding_registers(slave, start, qty, delay, connection_id)
-- Also: http_request, udp_send, redis_*, influxdb_*, timescaledb_*
```

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

**Multi-byte value construction** (when `read_*` helpers aren't suitable):

```lua
-- Big-endian uint16 from bytes
local b1, b2 = string.byte(data, 1, 2)
local value = bit.bor(bit.lshift(b1, 8), b2)
```

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
2. **Analyze device documentation**: extract connection parameters, protocol structure, data frame format, register
   maps, timing requirements, CRC/checksum methods
3. **Read current state**: `get_configs` and `get_lua_script` to see existing setup
4. **Clarify gaps**: ask user about missing parameters (port, baud rate, IP, etc.)

### Phase 2: Design & Implement

5. **Study schemas and examples**: read relevant `cycbox://config-schema/*` and `cycbox://examples/*` resources
6. **Configure connections**: choose transport, codec (for byte-stream), and optional transformer
7. **Write Lua script** if custom parsing is needed:
    - `on_receive()` — parse incoming data, extract values with `message:add_*_value()`
    - `on_timer()` — implement periodic polling (e.g., Modbus register reads)
    - `on_send()` — format outgoing commands
    - `on_start()` — initialize state, send startup commands

### Phase 3: Deploy & Verify

8. **Apply changes**:
    - Use `set_configs` to update connection configurations
    - Use `start_engine` or `restart_engine` to apply config changes (if permitted), otherwise **ask user to click "Apply" or "Start" in the CycBox UI**
    - Use `set_lua_script` to update script (syntax validated)
    - Use `reload_lua_script` to hot-reload (if permitted), otherwise **ask user to click "Reload Script" in the CycBox UI**
9. **Debug and iterate**:
    - Use `get_logs` to retrieve script output and hook errors (max 100 entries)
    - Refine parsing logic with `set_lua_script`, then `reload_lua_script` (or ask user to reload in UI)
    - Adjust codec parameters with `set_configs`, then `restart_engine` (or ask user to apply in UI)
10. **Ask user to verify** in CycBox UI: connection status, incoming messages, parsed values

**Script Hot Reload Behavior:**

- Hot reload (via `reload_lua_script` or user clicking "Reload Script" in UI) creates a new Lua VM (all globals lost, `on_start()` hook called again)
- Connections remain active during reload (no transport restart required)
- Use `get_logs` immediately after reload to check for errors

### Common Tasks

**Value extraction from binary protocol**: Use `on_receive()` with `read_*` helpers to parse payload bytes, then
`message:add_*_value()` to expose parsed values for charting.

**Message forwarding (cross-connection routing)**: In `on_receive()`, read from one connection, then
`send_after(new_payload, 0, target_connection_id)` to forward to another.

**MQTT publishing of parsed data**: Parse in `on_receive()`, then call
`mqtt_publish(topic, message.values_json, qos, retain, 0, mqtt_connection_id)`.

**Periodic Modbus polling**: Use `on_timer(elapsed_ms)` with a time check to call `modbus_rtu_read_input_registers()` at
desired intervals.

### Applying Changes

**After `set_configs` (configuration changes):**

- Configuration changes DO NOT take effect immediately
- If engine control is permitted: use `start_engine` (if stopped) or `restart_engine` (if running) to apply
- Otherwise: ask user to click "Apply" or "Start" button in the CycBox UI
- This will restart the engine with the new configuration
- Use when changing: transport, codec, transformer settings, or adding/removing connections

**After `set_lua_script` (script changes):**

- Script changes DO NOT take effect immediately
- If Lua reload is permitted and engine is running: use `reload_lua_script` to hot-reload
- Otherwise: ask user to click "Reload Script" (if running) or "Start" (if stopped) in the CycBox UI
- Hot reload preserves connection state (no transport restart)
- Creates new Lua VM (all globals lost, `on_start()` hook called again)
- Use for: updating parsing logic, modifying hooks, changing periodic tasks

**Best practice:** Use `get_logs` after applying changes to verify successful reload and check for runtime errors (max 100 entries shown).

## Engine Lifecycle Management

### Permission-Based Workflows

**If agent permissions are enabled:**

You can directly control the engine lifecycle using the permission-gated tools. This provides faster iteration:

1. **Initial setup**: `set_configs` → `start_engine` (waits 3s, returns logs)
2. **Update config**: `set_configs` → `restart_engine` (waits 3s, returns logs)
3. **Update script**: `set_lua_script` → `reload_lua_script` (waits 3s, returns logs)
4. **Stop debugging**: `stop_engine`

**If agent permissions are NOT enabled:**

The tools will return permission errors. Fall back to asking the user to manage lifecycle in the CycBox UI:

1. **Initial setup**: `set_configs` → **ask user to click "Start" in CycBox UI**
2. **Update config**: `set_configs` → **ask user to click "Apply" or "Restart" in CycBox UI**
3. **Update script**: `set_lua_script` → **ask user to click "Reload Script" in CycBox UI**
4. **Stop debugging**: **ask user to click "Stop" in CycBox UI**

### Error Handling

**Permission errors:**
```
"Engine control is not permitted. The user has not enabled agent engine control.
 Please ask the user to start the engine from the CycBox UI."
```

**State errors:**
- `start_engine` when already running → "Engine is already running. Use restart_engine to restart, or stop_engine to stop first."
- `stop_engine` when not running → "Engine is not running."
- `reload_lua_script` when not running → "Engine is not running. Start the engine first."
- Missing configuration → "No configuration stored. Use set_configs to configure the engine first."

### Log Collection

All lifecycle tools that start/reload return logs for immediate debugging:

- `start_engine`: Returns initial 100 log entries after 3-second startup delay
- `restart_engine`: Returns initial 100 log entries after 3-second restart delay
- `reload_lua_script`: Returns 100 log entries after 3-second reload delay (includes `on_start()` output)
- Use `get_logs` anytime to retrieve the most recent 100 entries

**Workflow tip:** When you receive logs from lifecycle tools, check them for errors before proceeding. If you see Lua errors or connection failures, debug before continuing.
