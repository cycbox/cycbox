# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

The SDK crate (`cycbox-sdk`) is the foundational library for CycBox. It defines **only** the core traits, message types,
manifest schemas, and utilities used by the engine and all protocol plugins. It contains no concrete implementations —
all codecs, transports, transformers, and formatters live in their own crates. The dependency flows one way: engine/plugins → SDK.

## Building and Testing

```bash
cargo build -p cycbox-sdk
cargo test -p cycbox-sdk
```

## Architecture

### Core Trait Hierarchy

All plugin traits extend `Configurable + Manifestable + Send + Sync`:

1. **Transport** (`src/transport.rs`): Creates connections that produce `MessageTransport` instances
    - `connect(configs, codec, timeout) → Box<dyn MessageTransport>`
    - Two implementation patterns: Stream-based and Message-based

2. **MessageTransport** (`src/transport.rs`): High-level message-oriented interface
    - `recv() → Option<Message>`, `send(message)`, `handle_command()`
    - This is what the engine works with — all transports ultimately produce this

3. **Codec** (`src/codec.rs`): Stateful frame encoding/decoding
    - `decode()`, `encode()`, `decode_timeout()`, `decode_eof()`, `reset()`, `handle_command()`
    - Single instance shared across RX/TX — stateful protocols rely on this

4. **Transformer** (`src/transformer.rs`): Message enrichment pipeline
    - `on_receive()` (post-decode), `on_send()` (pre-encode)
    - Extracts structured values for charts/tables

5. **RunMode** (`src/run_mode.rs`): Protocol factory
    - `create_transport(id, configs, codec, timeout)`, `create_codec(id, configs)`, `create_transformer(id, configs)`
    - `message_input_registry()` — returns `MessageInputRegistry` for message input dispatch
    - `lua_helper_registry()` — returns `LuaFunctionRegistry` for protocol-specific Lua functions
    - Used by the engine to instantiate all plugin components by ID

### Transport Implementation Patterns

There are two ways to implement a `Transport`:

**1. Stream-based (byte stream + codec)**: For transports that carry raw bytes (TCP, Serial, WebSocket, etc.)

- Implement `TransportIO` (AsyncRead + AsyncWrite) for the raw byte stream
- Wrap with `CodecTransport` (`src/transport.rs`) to get a `MessageTransport`
- `CodecTransport` reads into a 100KB buffer, decodes multiple messages per read (VecDeque buffering), handles
  timeout-based decoding, EOF flushing, and codec reset
- The `codec` parameter passed to `connect()` is used here

**2. Message-based (native framing)**: For transports with built-in message framing (MQTT, etc.)

- Implement `MessageTransport` directly — no codec involved
- `recv()` returns fully-formed `Message` objects; `send()` publishes them
- The `codec` parameter passed to `connect()` is ignored
- Set a hidden `{id}_requires_codec: false` form field in the manifest to signal the UI that no codec selection is
  needed

### Lua Extension System

The `lua` module (`src/lua.rs`) defines the extension points for protocol-specific Lua scripting:

- **LuaEngine**: Engine capabilities exposed to Lua — `send_message()` and logging functions
- **LuaFunctionRegistrar**: Trait for registering protocol-specific Lua globals (e.g., `mqtt_publish`, `modbus_read`)
- **LuaFunctionRegistry**: Collects registrars from `RunMode` and registers them all at Lua script startup
  - Non-fatal registration: failures are collected but don't block other helpers

Lua message interop lives in `src/message/lua_functions.rs` (helper functions) and `src/message/lua_user_data.rs`
(Message as Lua UserData).

### Message Input System

The `message_input` module (`src/message_input/`) handles user-initiated message creation:

- **MessageInput**: Raw JSON wrapper with `input_type` discriminator field — protocol-agnostic
- **MessageInputConverter** trait: Protocol plugins implement this to convert JSON → `Vec<Message>`
- **MessageInputRegistry**: Dispatches conversion by `input_type` string (e.g., "simple", "mqtt", "modbus")
    - Plugins register their converters; no built-in converters in the SDK
- **BatchMessageInput**: Ordered list of items with per-item `delay_ms` and `repeat` support
- **MessageInputGroup**: Groups of inputs stored in manifest, used for UI templates and Lua config persistence

### Message Flow

```
RX: Transport.recv() [bytes] → CodecTransport [buffered decode] → Codec.decode() → Message → Transformer.on_receive() → UI
TX: UI → MessageInput → Message → Transformer.on_send() → Codec.encode() → CodecTransport.send() → Transport [bytes]
```

### Message Structure

The `Message` type (`src/message/mod.rs`) is the universal data container:

- **connection_id**: Identifies source/target connection (9999=system, 9998=unknown)
- **timestamp**: Microseconds since epoch
- **message_type**: `"rx"`, `"tx"`, `"log"`, `"event"`, `"request"`, `"response"`
- **frame**: Raw frame bytes (or command name for request/response)
- **payload**: Decoded payload bytes (or seq_id for request/response)
- **contents** / **hex_contents**: Display content with rich text decorations (color, bold, etc.)
- **values**: Structured typed data for charts/tables
- **metadata**: Transport-specific metadata (MQTT topic, timeout, etc.)

`MessageBuilder` provides a fluent API with constructors: `event()`, `tx()`, `rx()`, `request()`, `response_success()`,
`response_error()`.

### Manifest System

Dynamic configuration uses a form schema (`src/manifest/`):

- **FormGroup**: Logical sections with title, description, collapsible panels
- **FormField**: Input types (`TextInput`, `IntegerInput`, `Code`, `FileInput`, etc.), choice chips, dropdowns
    - Conditional visibility via `FormCondition`
    - Grid layout with 12-column spans
- **FormUtils** (`src/manifest/form_utils.rs`): Type-safe config accessors (`get_text_value`, `get_integer_value`, etc.)
- **ManifestValues** (`src/manifest/manifest_values.rs`): Lightweight config snapshot for persistence/FFI — includes
  migration logic for renamed transport keys

### Value System

`Value` (`src/message/value.rs`) provides strongly-typed data:

- Scalars: `Boolean`, `Int8`–`Int64`, `UInt8`–`UInt64`, `Float32`, `Float64`, `String`
- Arrays: `Int8Array`–`Float64Array`
- Builder pattern: `Value::builder("id").u32(123).build()`
- Always little-endian in Protobuf serialization

### Localization

Fluent-based i18n (`src/l10n.rs`):

- `LocaleProvider` trait for supplying `.ftl` files
- `L10n::get(locale, key)` with fallback to "en"
- Locale files are provided by plugin crates, not the SDK itself

## Error Handling

`CycBoxError` (`src/error.rs`):

- `Connection(String)` — triggers reconnection in the engine
- `InvalidValue { field, reason }`, `MissingField`, `InvalidFormat` — config validation
- `Io`, `Json`, `Parse`, `Pending`, `Unsupported`, `Other` — general errors

## Important Constraints

1. **Pure SDK**: This crate contains only traits, types, and utilities — no concrete codec/transport/transformer implementations.
2. **Stateful codecs**: Single instance shared across RX/TX. `reset()` called on reconnection.
3. **Little-endian values**: Value payloads always little-endian in Protobuf, regardless of protocol endianness.
4. **Async trait bounds**: All traits require `Send + Sync` for tokio multi-threaded runtime.
5. **Transport pattern**: Choose based on protocol framing — stream-based transports implement `TransportIO` + wrap with
   `CodecTransport`; message-based transports (MQTT, etc.) implement `MessageTransport` directly and ignore the codec.
6. **MessageInput JSON**: `MessageInput` is protocol-agnostic JSON — the `input_type` field selects which converter
   processes it.
7. **Lua registration is non-fatal**: `LuaFunctionRegistry::register_all()` collects errors but continues registering other helpers.
