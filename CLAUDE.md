# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

This guide explains how to use the CycBox MCP server to configure and debug IoT device connections. CycBox connects to
devices via serial, TCP, WebSocket, MQTT, and UDP, then processes data through codecs, transformers, and LuaJIT scripts.


## Connection Model

Each session supports **multiple simultaneous connections**. Each connection has:

```
Transport (required) → Codec (conditional) → Transformer (optional)
```

- **Byte-stream transports** (serial, tcp_client, tcp_server, udp) **require** a codec for framing
- **Message-based transports** (websocket_client, websocket_server, mqtt_client) do not require a codec
- **Transformer** is always optional (numeric extraction, JSON parsing)

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


