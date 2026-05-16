# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`cycbox-engine` is the runtime core of CycBox. It implements an actor-based async engine that manages transport
connections, message routing, Lua script execution, and scheduled delivery.

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
Never call `Engine` methods that await a oneshot reply from within the engine task — use `EngineRef` instead to avoid
deadlocks.

## Key Dependencies on cycbox-sdk

- **Traits**: `RunMode`, `Codec`, `Transformer`, `MessageTransport`, `Configurable`, `Manifestable`, `LuaEngine`,
  `LuaFunctionRegistry`
- **Types**: `Message`, `Manifest`, `FormGroup`, `Value`, `MessageBuilder`
- **Constants**: `MESSAGE_TYPE_RX`, `MESSAGE_TYPE_TX`, `MESSAGE_TYPE_LOG`, `MESSAGE_TYPE_EVENT`

## Global Runtime

A single multi-threaded Tokio runtime is lazily initialized in `lib.rs` (`RUNTIME: Lazy<Runtime>`). All async work runs
on this runtime. 
