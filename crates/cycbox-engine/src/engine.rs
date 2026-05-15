use crate::command::Command;
use crate::error::EngineError;
use crate::state::EngineState;
use crate::tasks::start_engine_task;
use async_trait::async_trait;
use cycbox_sdk::lua::LuaEngine;
use cycbox_sdk::message::UNKNOW_CONNECTION_ID;
use cycbox_sdk::{
    Color, Content, ContentType, Decoration, MESSAGE_TYPE_LOG, Manifest, Message, MessageBuilder,
    RunMode, Value,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot};

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

impl LogLevel {
    fn color(&self) -> Color {
        match self {
            LogLevel::Info => Color::Primary,
            LogLevel::Warning => Color::Tertiary,
            LogLevel::Error => Color::Error,
            LogLevel::Debug => Color::OnSurfaceVariant,
        }
    }

    fn prefix(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Debug => "DEBUG",
        }
    }
}

/// Public API handle for the engine, implemented with the actor pattern.
///
/// `Engine` is cheaply cloneable — all clones share the same underlying mpsc sender
/// and broadcast channel. Every async method that needs a response sends a `Command`
/// with an embedded `oneshot::Sender` and awaits the reply from the engine task.
///
/// # Blocking vs non-blocking
/// Methods that await a `oneshot` reply (`get_state`, `start`, `stop`, `command`, …)
/// **must not** be called from inside the engine task itself — doing so will deadlock
/// because the task is busy waiting for its own reply. Use [`EngineRef`] instead for
/// code that runs inside the engine task (connection tasks, Lua hooks, etc.).
#[derive(Clone)]
pub struct Engine {
    sender: mpsc::Sender<Command>,
    message_broadcast: broadcast::Sender<Message>,
    is_debug: bool,
}
impl Engine {
    pub fn new(run_mode: Arc<dyn RunMode>, is_debug: bool) -> Self {
        let (sender, receiver) = mpsc::channel(10000);
        let (message_broadcast, _) = broadcast::channel(10000);

        let engine = Engine {
            sender,
            message_broadcast,
            is_debug,
        };
        // engine task should exit when sender is dropped
        start_engine_task(EngineRef(engine.clone()), run_mode, receiver);
        engine
    }

    /// Subscribe to the broadcast channel to receive all messages and events.
    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.message_broadcast.subscribe()
    }

    /// Fetch a snapshot of the current engine state (manifest, running flag, connection count).
    pub async fn get_state(&self) -> Result<EngineState, EngineError> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.sender
            .send(Command::GetState { result_sender })
            .await?;
        let result = result_receiver.await?;
        Ok(result)
    }

    /// Replace the stored manifest and broadcast the updated state. Does not restart connections.
    pub async fn set_manifest(
        &self,
        module: impl Into<String>,
        manifest: Manifest,
    ) -> Result<(), EngineError> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.sender
            .send(Command::SetManifest {
                manifest,
                result_sender,
            })
            .await?;
        let result = result_receiver.await?;
        let message = with_module_action(result.into(), module, "set_manifest").build();
        self.broadcast(message);
        Ok(())
    }

    /// Update Lua script source and/or its enabled state, optionally rebuilding the live script.
    ///
    /// - `lua_script = Some(code)` replaces the manifest's stored script.
    /// - `enabled = Some(flag)` updates the `lua_enabled` flag (persisted on engine state).
    /// - The active script is rebuilt (`on_stop` → recompile → `on_start`) when either
    ///   `reload` is true, or `enabled` actually changed while the engine is running.
    ///   Connections are not restarted.
    pub async fn set_lua(
        &self,
        module: impl Into<String>,
        lua_script: Option<String>,
        enabled: Option<bool>,
        reload: bool,
    ) -> Result<(), EngineError> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.sender
            .send(Command::SetLua {
                lua_script,
                enabled,
                reload,
                result_sender,
            })
            .await?;
        let result = result_receiver.await?;
        let state = result?;
        let message = with_module_action(state.into(), module, "set_lua").build();
        self.broadcast(message);
        Ok(())
    }

    /// Start the engine and spawn connection tasks.
    ///
    /// If `manifest` is `Some`, it replaces the stored manifest before starting.
    /// Initializes the Lua script and broadcasts the new state.
    /// Returns an error if the engine is already running.
    pub async fn start(
        &self,
        module: impl Into<String>,
        manifest: Option<Manifest>,
    ) -> Result<(), EngineError> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.sender
            .send(Command::Start {
                manifest,
                result_sender,
            })
            .await?;
        let result = result_receiver.await?;
        let state = result?;
        let message = with_module_action(state.into(), module, "start").build();
        self.broadcast(message);
        Ok(())
    }

    /// Stop the engine, cancel all connection tasks, and broadcast the new state.
    /// Calls the Lua `on_stop` hook before tearing down connections.
    pub async fn stop(&self, module: impl Into<String>) -> Result<(), EngineError> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.sender.send(Command::Stop { result_sender }).await?;
        let result = result_receiver.await?;
        let state = result?;
        let message = with_module_action(state.into(), module, "stop").build();
        self.broadcast(message);
        Ok(())
    }

    /// Format a log message and broadcast it as a `MESSAGE_TYPE_LOG` message to all subscribers.
    /// Debug-level messages are suppressed unless `is_debug` is `true`.
    pub fn log(&self, level: LogLevel, message: &str) {
        if matches!(level, LogLevel::Debug) && !self.is_debug {
            return;
        }
        let decoration = Decoration {
            bold: matches!(level, LogLevel::Error | LogLevel::Warning),
            italic: false,
            underline: false,
            strikethrough: false,
            color: level.color(),
            background: Color::Transparent,
        };
        let log_content = format!("[{}] {}", level.prefix(), message);
        let content = Content {
            content_type: ContentType::RichText,
            decoration,
            payload: log_content.into_bytes(),
            label: None,
        };
        let msg = MessageBuilder::new()
            .message_type(MESSAGE_TYPE_LOG)
            .contents(vec![content])
            .build();
        self.broadcast(msg);
    }

    /// Queue messages to be sent via connections.
    ///
    /// Fire-and-forget: queues a `Command::SendMessages` and returns immediately.
    /// The engine task runs the `on_send` Lua hook, then routes each message either
    /// to the delay queue (if `timestamp > now + 500µs`) or directly to the connection sender.
    pub async fn send_messages(&self, messages: Vec<Message>) {
        // log error if any message has UNKNOW_CONNECTION_ID
        for message in &messages {
            if message.connection_id == UNKNOW_CONNECTION_ID {
                log::error!(
                    "Trying to send message with UNKNOW_CONNECTION_ID: {:?}",
                    message
                );
            }
        }
        if let Err(e) = self.sender.send(Command::SendMessages { messages }).await {
            log::error!("Failed to send messages: {}", e);
        }
    }

    /// Start a repeating message batch and return its batch ID.
    ///
    /// Each `(Duration, Message)` pair is sent in sequence with the given inter-message delay,
    /// then the sequence repeats indefinitely until stopped. Returns a `batch_id` that can be
    /// passed to [`stop_repeating_messages`].
    pub async fn send_repeating_messages(
        &self,
        messages_with_delays: Vec<(Duration, Message)>,
    ) -> Result<u64, EngineError> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.sender
            .send(Command::SendRepeatingMessages {
                messages: messages_with_delays,
                result_sender,
            })
            .await?;
        result_receiver.await?
    }

    /// Stop repeating batches.
    ///
    /// If `batch_ids` is empty, cancels every active repeating batch. Otherwise cancels each
    /// listed batch ID (any ID that has already ended is silently ignored).
    pub async fn stop_repeating_messages(&self, batch_ids: Vec<u64>) -> Result<(), EngineError> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.sender
            .send(Command::StopRepeatingMessages {
                batch_ids,
                result_sender,
            })
            .await?;
        result_receiver.await?;
        Ok(())
    }

    /// Send a command message to a connection and wait for its response.
    ///
    /// Use `connection_id = SYSTEM_CONNECTION_ID` to broadcast to all connections and
    /// return the last non-empty response. Returns an error if no connection responds.
    pub async fn command(&self, command: Message) -> Result<Message, EngineError> {
        let (result_sender, result_receiver) = oneshot::channel();
        self.sender
            .send(Command::Command {
                command,
                result_sender,
            })
            .await?;
        result_receiver.await?
    }

    /// Broadcast a message directly to all subscribers without going through the engine task.
    /// Non-blocking — never waits for a oneshot reply.
    pub(crate) fn broadcast(&self, message_or_event: Message) {
        let _ = self.message_broadcast.send(message_or_event);
    }

    /// Notify the engine that a message was successfully transmitted by a connection.
    ///
    /// Fire-and-forget: the engine task runs the `on_send_confirm` Lua hook and broadcasts
    /// the confirmation message. Does not wait for a reply.
    pub(crate) async fn send_confirm(&self, message: Message) {
        if let Err(e) = self.sender.send(Command::SendConfirm(message)).await {
            log::error!("Failed to send confirm message: {}", e);
        }
    }

    /// Hand an inbound message to the engine task for Lua hook processing and broadcast.
    ///
    /// Fire-and-forget: queues a `Command::ReceiveMessage` and returns immediately.
    /// The engine task runs the `on_receive` Lua hook and then calls [`broadcast`].
    pub(crate) async fn receive_message(&self, message: Message) {
        // log error if message has UNKNOW_CONNECTION_ID
        if message.connection_id == UNKNOW_CONNECTION_ID {
            log::error!("Received message with UNKNOW_CONNECTION_ID: {:?}", message);
        }

        if let Err(e) = self.sender.send(Command::ReceiveMessage(message)).await {
            log::error!("Failed to send receive message: {}", e);
        }
    }
}

fn with_module_action(
    builder: MessageBuilder,
    module: impl Into<String>,
    action: impl Into<String>,
) -> MessageBuilder {
    builder.add_values(vec![
        Value::new_string("module", module),
        Value::new_string("action", action),
    ])
}

/// A restricted, deadlock-safe handle to the engine for use inside the engine task.
///
/// Connection tasks and Lua hooks run on the same Tokio runtime as the engine task.
/// If they called the full [`Engine`] API (which awaits `oneshot` replies from the engine task),
/// they would deadlock because the engine task is blocked waiting for its own reply.
///
/// `EngineRef` exposes only the subset of operations that are safe to call from within:
/// - **Fire-and-forget** async methods that push to the mpsc channel without waiting for a reply.
/// - **Direct** broadcast/subscribe operations (bypass the engine task entirely).
/// - **Synchronous** log helpers.
#[derive(Clone)]
pub(crate) struct EngineRef(Engine);

impl EngineRef {
    /// Broadcast a message to all subscribers (non-blocking)
    pub(crate) fn broadcast(&self, message: Message) {
        self.0.broadcast(message);
    }

    /// Fire-and-forget: queue a received message for processing by the engine task
    pub(crate) async fn receive_message(&self, message: Message) {
        self.0.receive_message(message).await;
    }

    /// Fire-and-forget: queue a single message to be sent via connections
    pub(crate) async fn send_message(&self, message: Message) {
        self.0.send_messages(vec![message]).await
    }

    /// Fire-and-forget: notify the engine that a message was successfully sent
    pub(crate) async fn send_confirm(&self, message: Message) {
        self.0.send_confirm(message).await;
    }

    pub(crate) fn debug(&self, message: &str) {
        self.0.log(LogLevel::Debug, message);
    }

    pub(crate) fn info(&self, message: &str) {
        self.0.log(LogLevel::Info, message);
    }

    pub(crate) fn warn(&self, message: &str) {
        self.0.log(LogLevel::Warning, message);
    }

    pub(crate) fn error(&self, message: &str) {
        self.0.log(LogLevel::Error, message);
    }
}

#[async_trait]
impl LuaEngine for EngineRef {
    async fn send_message(&self, message: Message) {
        self.0.send_messages(vec![message]).await
    }

    fn log(&self, level: &str, message: &str) {
        match level.to_lowercase().as_str() {
            "debug" => self.0.log(LogLevel::Debug, message),
            "info" => self.0.log(LogLevel::Info, message),
            "warn" | "warning" => self.0.log(LogLevel::Warning, message),
            "error" => self.0.log(LogLevel::Error, message),
            _ => self.0.log(LogLevel::Info, message), // default to info if unrecognized
        }
    }
}
