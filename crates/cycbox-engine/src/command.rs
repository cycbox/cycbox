use crate::EngineError;
use crate::state::EngineState;
use cycbox_sdk::prelude::*;
use std::time::Duration;
use tokio::sync::oneshot;

pub(crate) enum Command {
    GetState {
        result_sender: oneshot::Sender<EngineState>,
    },

    SetManifest {
        manifest: Manifest,
        result_sender: oneshot::Sender<EngineState>,
    },

    /// Start connections, if manifest is none, use the stored manifest,
    /// otherwise use the provided manifest (and update stored manifest)
    Start {
        manifest: Option<Manifest>,
        result_sender: oneshot::Sender<Result<EngineState, EngineError>>,
    },

    /// Stop all connections and clear runtimes, also clear pending messages and batches
    Stop {
        result_sender: oneshot::Sender<Result<EngineState, EngineError>>,
    },

    /// Set and/or reload the Lua script.
    /// If `lua_script` is Some, updates the stored script first.
    /// If `reload` is true, reloads (recompiles/reinitializes) the script after any update.
    SetLuaScript {
        lua_script: Option<String>,
        reload: bool,
        result_sender: oneshot::Sender<Result<EngineState, EngineError>>,
    },

    /// Enable or disable the Lua script.
    /// When disabled, calls `on_stop` (if running) and replaces the active script with an
    /// empty no-op script so hooks are skipped. When re-enabled while the engine is running,
    /// rebuilds the script from the stored manifest and calls `on_start`.
    SetLuaEnabled {
        enabled: bool,
        result_sender: oneshot::Sender<Result<EngineState, EngineError>>,
    },

    /// Hand an inbound message to the engine task for Lua `on_receive` hook processing and broadcast.
    ReceiveMessage(Message),
    /// Notify the engine that a message was successfully transmitted so the Lua `on_send_confirm`
    /// hook can run and the confirmation can be broadcast to subscribers.
    SendConfirm(Message),

    /// Send one or more messages, routed by `connection_id`.
    ///
    /// Messages whose `timestamp` is more than 500 µs in the future are queued in the delay
    /// queue; otherwise they are dispatched immediately to the matching connection sender.
    SendMessages { messages: Vec<Message> },

    /// Start a repeating message batch.
    ///
    /// Each `(Duration, Message)` pair is sent to the matching connection in sequence, waiting
    /// the given duration before each send. After the last message the sequence restarts
    /// indefinitely until the batch is stopped.
    SendRepeatingMessages {
        messages: Vec<(Duration, Message)>,
        result_sender: oneshot::Sender<Result<u64, EngineError>>,
    },
    /// Cancel a single repeating batch by its ID. No-op if the batch has already ended.
    StopRepeatingMessages {
        batch_id: u64,
        result_sender: oneshot::Sender<()>,
    },
    /// Cancel all active repeating batches at once.
    StopAllRepeatingMessages { result_sender: oneshot::Sender<()> },

    /// Send a command message to a connection and wait for its response.
    ///
    /// Use `connection_id = SYSTEM_CONNECTION_ID` to broadcast to all connections and collect
    /// the last non-empty response. Returns an error if no connection responds.
    #[allow(clippy::enum_variant_names)]
    Command {
        command: Message,
        result_sender: oneshot::Sender<Result<Message, EngineError>>,
    },
}
