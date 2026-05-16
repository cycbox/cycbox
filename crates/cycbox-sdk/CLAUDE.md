# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`cycbox-sdk` defines the core traits, message types, manifest schemas, and utilities used by the engine and all
protocol plugins. It contains **no concrete implementations** — codecs, transports, transformers, and formatters
live in their own crates. Dependency flows one way: engine/plugins → SDK.

## Core Traits

All plugin traits extend `Configurable + Manifestable + Send + Sync`.

- **Transport** (`src/transport.rs`): `connect(configs, codec, timeout) → Box<dyn MessageTransport>`.
- **MessageTransport** (`src/transport.rs`): High-level interface the engine works with — `recv()`, `send()`,
  `handle_command()`.
- **Codec** (`src/codec.rs`): Stateful frame encode/decode. **Single instance shared across RX/TX**; `reset()` on
  reconnect.
- **Transformer** (`src/transformer.rs`): `on_receive()` / `on_send()` for enrichment and extracting structured values.
- **RunMode** (`src/run_mode.rs`): Protocol factory — creates transports/codecs/transformers by id; exposes
  `message_input_registry()` and `lua_helper_registry()`.

## Transport Patterns

Two ways to implement `Transport`:

1. **Stream-based** (TCP, Serial, WebSocket): Implement `TransportIO` (AsyncRead + AsyncWrite), wrap with
   `CodecTransport` to get a `MessageTransport`. The `codec` parameter is used here.
2. **Message-based** (MQTT, etc.): Implement `MessageTransport` directly; the `codec` parameter is ignored. Set a
   hidden `{id}_requires_codec: false` form field so the UI hides codec selection.

## Message Flow

```
RX: Transport.recv() → CodecTransport → Codec.decode() → Message → Transformer.on_receive() → UI
TX: UI → MessageInput → Message → Transformer.on_send() → Codec.encode() → CodecTransport.send() → Transport
```

## Message Type

`Message` (`src/message/mod.rs`) is the universal data container with `connection_id`, `timestamp` (µs), `message_type`
(`rx`/`tx`/`log`/`event`/`request`/`response`), `frame`, `payload`, `contents`, `values`, `metadata`.
Use `MessageBuilder` (`event()`, `tx()`, `rx()`, `request()`, `response_success()`, `response_error()`).

System connection ids: `9999` = system, `9998` = unknown.

## Subsystems

- **Manifest** (`src/manifest/`): `FormGroup` + `FormField` describe dynamic UI config. Use `FormUtils` for type-safe
  accessors. `ManifestValues` is the persisted/FFI snapshot.
- **Value** (`src/message/value.rs`): Strongly-typed scalars and arrays, **always little-endian** in Protobuf.
  Builder: `Value::builder("id").u32(123).build()`.
- **MessageInput** (`src/message_input/`): Protocol-agnostic JSON with `input_type` discriminator; plugins register
  `MessageInputConverter` implementations.
- **Lua** (`src/lua.rs`): `LuaEngine` exposes engine capabilities; plugins register protocol-specific globals via
  `LuaFunctionRegistrar`. Registration is non-fatal — failures collected, others continue.
- **L10n** (`src/l10n.rs`): Fluent-based i18n; locale files supplied by plugin crates.
- **Error** (`src/error.rs`): `CycBoxError::Connection` triggers reconnect; `InvalidValue`/`MissingField`/
  `InvalidFormat` for config validation.
