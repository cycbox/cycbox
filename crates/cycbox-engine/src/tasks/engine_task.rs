use crate::EngineError;
use crate::command::Command;
use crate::engine::EngineRef;
use crate::lua::{DEFAULT_LUA_SCRIPT, LuaScript};
use crate::state::EngineState;
use crate::tasks::start_delay_queue_task;
use cycbox_sdk::message::SYSTEM_CONNECTION_ID;
use cycbox_sdk::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;

/// Spawn the engine task and return its `JoinHandle`.
///
/// The engine task is the central command loop. It owns all mutable engine state and
/// processes `Command` messages from the mpsc channel one at a time, eliminating the need
/// for locks on shared state. It also drives the 100 ms Lua timer tick.
///
/// The task exits when the mpsc `receiver` is closed (i.e. all `Engine` handles are dropped),
/// at which point it cancels all child tasks and waits briefly for them to finish.
pub(crate) fn start_engine_task(
    engine: EngineRef,
    run_mode: Arc<dyn RunMode>,
    mut receiver: mpsc::Receiver<Command>,
) -> JoinHandle<()> {
    crate::RUNTIME.spawn(async move {
        // Root token — cancelling it shuts down every child task at once when the engine exits.
        let cancellation_token = CancellationToken::new();

        // Delay-queue task: holds a priority queue of future-timestamped messages and delivers
        // them when their scheduled time arrives.
        let (delay_sender, delay_receiver) = mpsc::channel(100);
        let delay_queue_ctx = cancellation_token.child_token();
        let delay_queue_handle =
            start_delay_queue_task(engine.clone(), delay_receiver, delay_queue_ctx);

        // Connection tasks — one per config entry in the manifest.
        // `connections_ctx` is replaced with a fresh child token on each Start/Stop so that
        // only connection tasks are cancelled without affecting the delay-queue task.
        let mut connections_ctx = cancellation_token.child_token();
        let mut connection_handlers: Vec<JoinHandle<()>> = Vec::new();
        // TX channel: engine → connection (fire-and-forget messages)
        let mut connection_senders: Vec<Sender<Message>> = Vec::new();
        // Command channel: engine → connection (request/response pairs)
        let mut connection_command_senders: Vec<Sender<(Message, tokio::sync::oneshot::Sender<Option<Message>>)>> = Vec::new();

        // Repeating-message tasks — one per active batch.
        // `batch_id` is a monotonically increasing ID handed back to callers.
        // `repeating_ctx` is replaced on StopAll so individual batches can also be aborted.
        let batch_id = AtomicU64::new(1);
        let mut repeating_ctx = cancellation_token.child_token();
        let mut repeating_handlers: HashMap<u64, JoinHandle<()>> = HashMap::new();

        let mut manifest = run_mode.manifest("en").await;
        if manifest.lua_script.is_none() {
            manifest.lua_script = Some(DEFAULT_LUA_SCRIPT.to_string());
        }
        let mut state = EngineState {
            manifest,
            running: false,
            connection_count: 0,
            lua_enabled: true,
        };

        // Lua script state — always present; initialised to an empty (no-op) script so that
        // on_timer / on_receive calls before Start are safely ignored.
        let lua_helper_registry = run_mode.lua_helper_registry();
        let mut lua_script = LuaScript::new("", &[], engine.clone(), lua_helper_registry, vec![])
            .expect("Failed to create empty Lua script");
        // 100 ms periodic tick drives LuaScript::on_timer.
        let mut timer_interval = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                request = receiver.recv() => {
                    let request = match request {
                        Some(r) => r,
                        None => break, // sender dropped, exit loop
                    };
                    match request {
                        Command::GetState { result_sender } => {
                            let _ = result_sender.send(state.clone());
                        }
                        Command::SetManifest {
                            manifest,
                            result_sender,
                        } => {
                            state.manifest = manifest;
                            let _ = result_sender.send(state.clone());
                        }
                        Command::Start {
                            manifest,
                            result_sender,
                        } => {
                            let connection_manifest = if let Some(m) = manifest {
                                state.manifest = m.clone();
                                m
                            } else {
                                state.manifest.clone()
                            };
                            if state.running {
                                let _= result_sender.send(Err(EngineError::Engine("Engine is already running".to_string())));
                            } else {
                                state.running = true;
                                connections_ctx.cancel();
                                connections_ctx = cancellation_token.child_token();
                                // track all connection task and abort
                                connection_handlers.iter().for_each(|h| h.abort());
                                connection_handlers = vec![];
                                connection_senders = vec![];
                                connection_command_senders = vec![];
                                for (i, config) in connection_manifest.configs.iter().enumerate() {
                                    let (connection_sender, connection_receiver) = mpsc::channel(100);
                                    let (cmd_sender, cmd_receiver) = mpsc::channel(16);
                                    connection_senders.push(connection_sender);
                                    connection_command_senders.push(cmd_sender);
                                    let handle = crate::tasks::start_connection(
                                        i as u32,
                                        config.clone(),
                                        engine.clone(),
                                        run_mode.clone(),
                                        connection_receiver,
                                        cmd_receiver,
                                        connections_ctx.clone(),
                                    );
                                    connection_handlers.push(handle);
                                }
                                state.connection_count = connection_handlers.len();

                                // Initialize Lua script (empty if disabled)
                                let lua_code = if state.lua_enabled {
                                    state.manifest.lua_script.as_deref().unwrap_or("")
                                } else {
                                    ""
                                };
                                let configs_refs: Vec<&[FormGroup]> = connection_manifest
                                    .configs
                                    .iter()
                                    .map(|c| c.as_slice())
                                    .collect();
                                match LuaScript::new(lua_code, &configs_refs, engine.clone(), lua_helper_registry, connection_command_senders.clone()) {
                                    Ok(mut script) => {
                                        if let Err(e) = script.on_start().await {
                                            engine.error(&format!("Lua on_start error: {e}"));
                                        }
                                        lua_script = script;
                                    }
                                    Err(e) => {
                                        engine.error(&format!("Failed to create Lua script: {e}"));
                                    }
                                }
                                let _ = result_sender.send(Ok(state.clone()));
                            }
                        }
                        Command::Stop { result_sender } => {
                            if let Err(e) = lua_script.on_stop().await {
                                engine.error(&format!("Lua on_stop error: {e}"));
                            }
                            // Replace with an empty script so on_timer ticks between Stop and the
                            // next Start do not execute stale user code.
                            lua_script = LuaScript::new("", &[], engine.clone(), lua_helper_registry, vec![])
                                .expect("Failed to create empty Lua script");

                            state.running = false;
                            repeating_ctx.cancel();
                            repeating_ctx = cancellation_token.child_token();
                            repeating_handlers.values().for_each(|h| h.abort());
                            repeating_handlers.clear();

                            connections_ctx.cancel();
                            connections_ctx = cancellation_token.child_token();
                            tokio::time::sleep(Duration::from_millis(15)).await; // wait for tasks to notice cancellation
                            connection_handlers.iter().for_each(|h| h.abort());
                            connection_handlers = vec![];
                            connection_senders = vec![];
                            connection_command_senders = vec![];

                            state.connection_count = 0;
                            let _ = result_sender.send(Ok(state.clone()));
                        }
                        Command::ReceiveMessage(mut message) => {
                            if let Err(e) = lua_script.on_receive(&mut message).await {
                                engine.error(&format!("Lua on_receive error: {e}"));
                            }

                            // Check for auto-response (e.g. Modbus server codec)
                            let auto_response = if let Some(response_value) = message
                                .metadata_value("auto_response")
                            {
                                let response_frame =response_value.as_bytes();
                                let conn_id = message.connection_id;
                                message.remove_metadata("auto_response");
                                Some((response_frame, conn_id))
                            } else {
                                None
                            };

                            engine.broadcast(message);

                            // Send auto-response
                            if let Some((response_frame, conn_id)) = auto_response {
                                let tx_msg = MessageBuilder::tx(
                                    conn_id,
                                    PayloadType::Binary,
                                    response_frame.clone(),
                                    response_frame,
                                )
                                .build();
                                engine.send_message(tx_msg).await;
                            }
                        }
                        Command::SendConfirm(mut message) => {
                            if let Err(e) = lua_script.on_send_confirm(&mut message).await {
                                engine.error(&format!("Lua on_send_confirm error: {e}"));
                            }
                            engine.broadcast(message);
                        }
                        Command::SendMessages { messages } => {
                            if !state.running {
                                engine.warn("Cannot send messages: engine is not running");
                            } else {
                                let current_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_micros() as u64;
                                for mut message in messages {
                                    if let Err(e) = lua_script.on_send(&mut message).await {
                                        engine.error(&format!("Lua on_send error: {e}"));
                                    }
                                    // Messages scheduled more than 500 µs in the future go to the
                                    // delay queue; anything within that window is treated as immediate.
                                    if message.timestamp > (current_time + 500) {
                                        let _ = delay_sender.send(message).await;
                                    } else if let Some(sender) =
                                        connection_senders.get(message.connection_id as usize)
                                    {
                                        let _ = sender.send(message).await;
                                    }
                                }
                            }
                        }
                        Command::SendRepeatingMessages {
                            messages,
                            result_sender,
                        } => {
                            if !state.running {
                                let _ = result_sender.send(Err(EngineError::Engine(
                                    "Engine is not running".to_string(),
                                )));
                            } else {
                                let new_batch_id =
                                    batch_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                let handle = crate::tasks::start_repeating_message_task(
                                    engine.clone(),
                                    messages,
                                    repeating_ctx.child_token(),
                                );
                                repeating_handlers.insert(new_batch_id, handle);
                                let _ = result_sender.send(Ok(new_batch_id));
                            }
                        }
                        Command::StopRepeatingMessages {
                            batch_id,
                            result_sender,
                        } => {
                            if let Some(handle) = repeating_handlers.remove(&batch_id) {
                                handle.abort();
                            }
                            let _ = result_sender.send(());
                        }
                        Command::StopAllRepeatingMessages { result_sender } => {
                            repeating_ctx.cancel();
                            repeating_ctx = cancellation_token.child_token();
                            repeating_handlers.values().for_each(|h| h.abort());
                            repeating_handlers.clear();
                            let _ = result_sender.send(());
                        }
                        Command::SetLuaScript {
                            lua_script: new_script_code,
                            reload,
                            result_sender,
                        } => {
                            // Update manifest if new code was provided
                            if let Some(new_code) = new_script_code {
                                state.manifest.lua_script = Some(new_code);
                            }

                            if reload {
                                // Stop existing script
                                if let Err(e) = lua_script.on_stop().await {
                                    engine.error(&format!("Lua on_stop error during reload: {e}"));
                                }

                                // Reload with current manifest script (or empty if disabled)
                                let lua_code = if state.lua_enabled {
                                    state.manifest.lua_script.as_deref().unwrap_or("")
                                } else {
                                    ""
                                };
                                let configs_refs: Vec<&[FormGroup]> = state
                                    .manifest
                                    .configs
                                    .iter()
                                    .map(|c| c.as_slice())
                                    .collect();
                                match LuaScript::new(lua_code, &configs_refs, engine.clone(), lua_helper_registry, connection_command_senders.clone()) {
                                    Ok(mut script) => {
                                        if let Err(e) = script.on_start().await {
                                            engine.error(&format!("Lua on_start error after reload: {e}"));
                                        }
                                        lua_script = script;
                                        let _ = result_sender.send(Ok(state.clone()));
                                    }
                                    Err(e) => {
                                        engine.error(&format!("Failed to reload Lua script: {e}"));
                                        let _ = result_sender.send(Err(EngineError::Engine(format!(
                                            "Failed to reload Lua script: {e}"
                                        ))));
                                    }
                                }
                            } else {
                                let _ = result_sender.send(Ok(state.clone()));
                            }
                        }
                        Command::SetLuaEnabled { enabled, result_sender } => {
                            if state.lua_enabled == enabled {
                                let _ = result_sender.send(Ok(state.clone()));
                            } else {
                                state.lua_enabled = enabled;

                                if state.running {
                                    // Tear down the current script
                                    if let Err(e) = lua_script.on_stop().await {
                                        engine.error(&format!(
                                            "Lua on_stop error during set_lua_enabled: {e}"
                                        ));
                                    }

                                    let lua_code = if enabled {
                                        state.manifest.lua_script.as_deref().unwrap_or("")
                                    } else {
                                        ""
                                    };
                                    let configs_refs: Vec<&[FormGroup]> = state
                                        .manifest
                                        .configs
                                        .iter()
                                        .map(|c| c.as_slice())
                                        .collect();
                                    match LuaScript::new(
                                        lua_code,
                                        &configs_refs,
                                        engine.clone(),
                                        lua_helper_registry,
                                        connection_command_senders.clone(),
                                    ) {
                                        Ok(mut script) => {
                                            if let Err(e) = script.on_start().await {
                                                engine.error(&format!(
                                                    "Lua on_start error after set_lua_enabled: {e}"
                                                ));
                                            }
                                            lua_script = script;
                                            let _ = result_sender.send(Ok(state.clone()));
                                        }
                                        Err(e) => {
                                            engine.error(&format!(
                                                "Failed to rebuild Lua script: {e}"
                                            ));
                                            let _ = result_sender.send(Err(EngineError::Engine(
                                                format!("Failed to rebuild Lua script: {e}"),
                                            )));
                                        }
                                    }
                                } else {
                                    let _ = result_sender.send(Ok(state.clone()));
                                }
                            }
                        }
                        Command::Command { command, result_sender } => {
                            if command.connection_id == SYSTEM_CONNECTION_ID {
                                // Broadcast to all connections, keep the last non-empty response
                                let mut last_response: Option<Message> = None;
                                for sender in &connection_command_senders {
                                    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                                    let _ = sender.send((command.clone(), resp_tx)).await;
                                    if let Ok(Some(resp)) = resp_rx.await {
                                        last_response = Some(resp);
                                    }
                                }
                                match last_response {
                                    Some(resp) => { let _ = result_sender.send(Ok(resp)); }
                                    None => {
                                        let err = format!("Command '{}' returned no response from any connection", command.get_command());
                                        engine.error(&err);
                                        let _ = result_sender.send(Err(EngineError::Engine(err)));
                                    }
                                }
                            } else {
                                match connection_command_senders.get(command.connection_id as usize) {
                                    Some(sender) => {
                                        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                                        let _ = sender.send((command.clone(), resp_tx)).await;
                                        match resp_rx.await {
                                            Ok(Some(resp)) => { let _ = result_sender.send(Ok(resp)); }
                                            _ => {
                                                let err = format!("Command '{}' returned no response from connection {}", command.get_command(), command.connection_id);
                                                engine.error(&err);
                                                let _ = result_sender.send(Err(EngineError::Engine(err)));
                                            }
                                        }
                                    }
                                    None => {
                                        let err = format!("Command '{}' connection {} not found", command.get_command(), command.connection_id);
                                        let _ = result_sender.send(Err(EngineError::Engine(err)));
                                    }
                                }
                            }
                        }
                    }
                }
                _ = timer_interval.tick() => {
                    let timestamp_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    if let Err(e) = lua_script.on_timer(timestamp_ms).await {
                        engine.warn(&format!("Lua on_timer error: {e}"));
                    }
                }
            }
        }
        // All Engine handles have been dropped; shut down every child task.
        cancellation_token.cancel();
        // Brief grace period so child tasks can observe the cancellation before we abort.
        tokio::time::sleep(Duration::from_millis(15)).await;
        delay_queue_handle.abort();
    })
}
