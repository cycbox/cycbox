# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`cycbox-engine` is the runtime core of CycBox. It implements an actor-based async engine that manages transport
connections, message routing, Lua script execution, and scheduled delivery. This crate contains only engine-related
code — protocol-specific Lua helpers (HTTP, Redis, SMTP, etc.) live in `cycbox-runtime`.

## Build & Test

```bash
cargo build -p cycbox-engine
cargo clippy -p cycbox-engine
```

## Architecture

### Actor Model

`Engine` is the public API. It holds an `mpsc::Sender<Command>` for sending commands and a `broadcast::Sender<Message>`
for distributing received messages to subscribers.

```
Engine (public API — cheaply cloneable)
  │
  ├─ mpsc channel ──► engine_task (command loop)
  │                     ├─► connection_task (per config group)
  │                     ├─► delay_queue_task (scheduled messages)
  │                     └─► repeating_message_task (periodic batches)
  │
  └─ broadcast channel ◄── all tasks post messages here
```

`EngineRef` is a non-blocking handle for code running **inside** the engine task (connection tasks, Lua hooks).
Never call `Engine` methods that await a oneshot reply from within the engine task — use `EngineRef` instead to avoid deadlocks.

### Command Flow

All interactions go through `Command` enum variants sent via the mpsc channel:

- `Start(Manifest)` → creates connection tasks from manifest config groups
- `Stop` → cancels all child tasks via `CancellationToken`
- `SendMessages` → routes to delay queue (if timestamp > now + 500µs) or direct to connection senders
- `SendRepeatingMessages` / `StopRepeatingMessages` → manages periodic message batches with atomic batch IDs
- `GetState` / `SetManifest` / `ReceiveMessage` → state queries and updates

### Connection Pipeline

Each connection wraps a `Box<dyn MessageTransport>` with an optional `Box<dyn Transformer>`:

- **RX**: transport.recv() → transformer.on_receive() → format/hexdump → broadcast
- **TX**: transformer.on_send() → codec encode → format → send via transport

`CycBoxRunMode` (in `cycbox-runtime`) implements the SDK's `RunMode` trait as a factory, creating transport/codec/transformer instances from config.

### Task Hierarchy

- **engine_task**: Main command loop. Spawns and manages all child tasks. Drives the 100ms Lua timer tick.
- **connection_task**: Per-connection RX/TX loops.
- **delay_queue_task**: Binary heap priority queue with high-resolution timers for scheduled message delivery.
- **repeating_message_task**: Infinite loop sending message sequences with configurable delays between each.

### High-Resolution Timers (`delay.rs`)

Cross-platform precise timing for sub-second delays:

- **Linux**: `timerfd` with `CLOCK_MONOTONIC` via nix crate
- **Windows**: High-resolution waitable timers via `windows-sys`
- **Fallback**: `tokio::time::sleep`
- Threshold: delays < 1000ms use platform-specific high-res timers

### Lua Scripting (`lua.rs`)

`LuaScript` hosts the Lua VM and provides the script lifecycle. It registers built-in utility functions
(`log`, `send_after`, `get_env`, `get_transport`, `get_codec`, `get_connection_count`) and detects user-defined
hooks (`on_start`, `on_stop`, `on_receive`, `on_send`, `on_send_confirm`, `on_timer`).

Protocol-specific Lua helpers (HTTP, Redis, InfluxDB, SMTP, ntfy, Discord, TimescaleDB) are registered via
`LuaFunctionRegistry` from `cycbox-sdk` — the actual implementations live in `cycbox-runtime`.

## Key Dependencies on cycbox-sdk

- **Traits**: `RunMode`, `Codec`, `Transformer`, `MessageTransport`, `Configurable`, `Manifestable`, `LuaEngine`, `LuaFunctionRegistry`
- **Types**: `Message`, `Manifest`, `FormGroup`, `Value`, `MessageBuilder`
- **Constants**: `MESSAGE_TYPE_RX`, `MESSAGE_TYPE_TX`, `MESSAGE_TYPE_LOG`, `MESSAGE_TYPE_EVENT`

## State Management

`EngineState` tracks: manifest, running flag, connection count. State changes are broadcast as `Message`
events so the UI can react.

## Localization

Uses Fluent-based i18n with embedded `.ftl` files from `locales/`. Global lazy `L10n` instance via `get_l10n()`.

## Global Runtime

A single multi-threaded Tokio runtime is lazily initialized in `lib.rs` (`RUNTIME: Lazy<Runtime>`). All async work runs
on this runtime. Also initializes the rustls crypto provider for TLS support.
