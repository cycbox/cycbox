use crate::connection::Connection;
use crate::engine::EngineRef;
use crate::formatter::get_encoding_from_name;
use cycbox_sdk::manifest::FormUtils;
use cycbox_sdk::prelude::*;
use log::warn;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

/// Maximum number of messages buffered per connection while the codec
/// reports `CycBoxError::Pending`. On overflow the oldest entry is dropped.
const MAX_OUTBOX_LEN: usize = 64;
/// First-time warning threshold for a growing outbox.
const OUTBOX_WARN_THRESHOLD: usize = 8;
/// Retry interval for draining queued outbound messages.
const OUTBOX_RETRY_INTERVAL: Duration = Duration::from_millis(100);

enum DrainOutcome {
    Idle,
    Reconnect,
}

pub(crate) fn start_connection(
    connection_id: u32,
    config: Vec<FormGroup>,
    engine: EngineRef,
    run_mode: Arc<dyn RunMode>,
    mut receiver: Receiver<Message>,
    mut command_receiver: Receiver<(Message, oneshot::Sender<Option<Message>>)>,
    ctx: CancellationToken,
) -> JoinHandle<()> {
    crate::RUNTIME.spawn(async move {
        // Extract config values
        let transport_id = match FormUtils::get_text_value(&config, "app", "app_transport") {
            Some(id) => id.to_string(),
            None => {
                engine.error(&format!("Connection {}: missing transport config", connection_id));
                return;
            }
        };
        let codec_id = match FormUtils::get_text_value(&config, "app", "app_codec") {
            Some(id) => id.to_string(),
            None => {
                engine.error(&format!("Connection {}: missing codec config", connection_id));
                return;
            }
        };
        let transformer_id =
            FormUtils::get_text_value(&config, "app", "app_transformer").map(|s| s.to_string());
        let encoding = FormUtils::get_text_value(&config, "app", "app_encoding")
            .map(get_encoding_from_name)
            .unwrap_or_else(|| get_encoding_from_name("utf-8"));
        let timeout = FormUtils::get_receive_timeout(&config);

        // Reconnection loop with exponential backoff
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(10);
        let mut reconnecting = false;

        'outer: loop {
            if ctx.is_cancelled() {
                break;
            }

            // Apply backoff delay (skip on first attempt)
            if reconnecting {
                tokio::select! {
                    _ = ctx.cancelled() => break,
                    _ = tokio::time::sleep(backoff) => {}
                }
                backoff = (backoff * 2).min(max_backoff);
            }

            // Create codec
            let codec = match run_mode.create_codec(&codec_id, &config).await {
                Ok(c) => c,
                Err(e) => {
                    engine.error(&format!("Failed to create codec: {e}"));
                    break;
                }
            };

            // Create transport
            let transport = match run_mode
                .create_transport(&transport_id, &config, codec, timeout)
                .await
            {
                Ok(t) => t,
                Err(e) => {
                    // Only reconnect for IO/connection failures; config errors are fatal
                    if matches!(e, CycBoxError::Connection(_)) {
                        warn!("Connection {connection_id} transport connection error: {e}, reconnecting...");
                        engine.warn(&format!("Connection {connection_id} transport error: {e}, reconnecting..."));
                        reconnecting = true;
                        continue;
                    } else {
                        engine.error(&format!("Connection {connection_id} transport config error: {e}"));
                        break;
                    }
                }
            };

            // Create transformer (optional)
            let transformer = if let Some(ref tid) = transformer_id {
                match run_mode.create_transformer(tid, &config).await {
                    Ok(t) => t,
                    Err(e) => {
                        engine.error(&format!("Failed to create transformer: {e}"));
                        break;
                    }
                }
            } else {
                None
            };

            // Connection established — reset backoff
            backoff = Duration::from_secs(1);
            reconnecting = false;

            let mut connection = Connection::new(connection_id, transport, transformer, encoding);

            // Per-connection outbox for messages deferred by codec back-pressure
            // (e.g. half-duplex Modbus RTU returning `CycBoxError::Pending` while a
            // request is awaiting its response).
            let mut pending_outbox: VecDeque<Message> = VecDeque::new();
            let mut outbox_warned = false;
            let mut retry_interval = tokio::time::interval(OUTBOX_RETRY_INTERVAL);
            retry_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

            engine.info(&format!("Connection {connection_id} established"));

            // RX/TX loop
            loop {
                tokio::select! {
                    _ = ctx.cancelled() => break 'outer,
                    result = connection.recv() => {
                        match result {
                            Ok(Some(msg)) => {
                                engine.receive_message(msg).await;
                                // A successful decode typically clears the codec's pending
                                // state, so try to drain the next queued message.
                                if matches!(
                                    drain_outbox(&mut pending_outbox, &mut connection, &engine, connection_id).await,
                                    DrainOutcome::Reconnect
                                ) {
                                    reconnecting = true;
                                    break;
                                }
                            }
                            Ok(None) => {
                                engine.warn(&format!("Connection {connection_id} lost, reconnecting..."));
                                reconnecting = true;
                                break;
                            }
                            Err(e) => {
                                if matches!(e, CycBoxError::Connection(_)) {
                                    engine.warn(&format!("Connection {connection_id} recv error: {e}, reconnecting..."));
                                    break;
                                } else {
                                    engine.error(&format!("Connection {connection_id} recv error: {e}"));
                                }
                            }
                        }
                    }
                    Some(msg) = receiver.recv() => {
                        if pending_outbox.is_empty() {
                            // Fast path: try to send immediately.
                            match connection.send(msg.clone()).await {
                                Ok(tx_msg) => engine.send_confirm(tx_msg).await,
                                Err(CycBoxError::Pending(_)) => {
                                    enqueue_outbox(
                                        msg,
                                        &mut pending_outbox,
                                        &engine,
                                        connection_id,
                                        &mut outbox_warned,
                                    );
                                }
                                Err(e) => {
                                    if matches!(e, CycBoxError::Connection(_)) {
                                        engine.warn(&format!("Connection {connection_id} send error: {e}, reconnecting..."));
                                        reconnecting = true;
                                        break;
                                    } else {
                                        engine.error(&format!("Connection {connection_id} send error: {e}"));
                                    }
                                }
                            }
                        } else {
                            // Preserve FIFO: queue behind existing items, then attempt
                            // to drain the front. Skipping ahead would reorder requests
                            // on protocols where ordering is significant.
                            enqueue_outbox(
                                msg,
                                &mut pending_outbox,
                                &engine,
                                connection_id,
                                &mut outbox_warned,
                            );
                            if matches!(
                                drain_outbox(&mut pending_outbox, &mut connection, &engine, connection_id).await,
                                DrainOutcome::Reconnect
                            ) {
                                reconnecting = true;
                                break;
                            }
                        }
                    }
                    _ = retry_interval.tick() => {
                        // Periodic drain handles the case where the codec clears its
                        // pending state via timeout (no response decoded).
                        if !pending_outbox.is_empty()
                            && matches!(
                                drain_outbox(&mut pending_outbox, &mut connection, &engine, connection_id).await,
                                DrainOutcome::Reconnect
                            )
                        {
                            reconnecting = true;
                            break;
                        }
                    }
                    Some((cmd, resp_sender)) = command_receiver.recv() => {
                        let response = connection.handle_command(&cmd).await;
                        let _ = resp_sender.send(response);
                    }
                }
            }

            // Drop any messages still queued for this (now-dead) connection. Re-sending
            // them after reconnect is unsafe — device request/response state is unknown
            // and stale frames could correlate with the wrong response.
            if !pending_outbox.is_empty() {
                engine.warn(&format!(
                    "Connection {connection_id} dropping {} queued message(s) on reconnect",
                    pending_outbox.len()
                ));
                pending_outbox.clear();
            }
        }
    })
}

fn enqueue_outbox(
    msg: Message,
    outbox: &mut VecDeque<Message>,
    engine: &EngineRef,
    connection_id: u32,
    warned: &mut bool,
) {
    if outbox.len() >= MAX_OUTBOX_LEN {
        outbox.pop_front();
        engine.warn(&format!(
            "Connection {connection_id} outbox full ({MAX_OUTBOX_LEN}), dropping oldest message"
        ));
    }
    outbox.push_back(msg);
    if !*warned && outbox.len() > OUTBOX_WARN_THRESHOLD {
        engine.warn(&format!(
            "Connection {connection_id} outbox depth {} — codec back-pressure",
            outbox.len()
        ));
        *warned = true;
    }
}

async fn drain_outbox(
    outbox: &mut VecDeque<Message>,
    connection: &mut Connection,
    engine: &EngineRef,
    connection_id: u32,
) -> DrainOutcome {
    let Some(msg) = outbox.pop_front() else {
        return DrainOutcome::Idle;
    };
    // Clone before send so transformer/codec mutations on the in-flight copy
    // don't taint the queued original if we have to retry.
    match connection.send(msg.clone()).await {
        Ok(tx_msg) => {
            engine.send_confirm(tx_msg).await;
            DrainOutcome::Idle
        }
        Err(CycBoxError::Pending(_)) => {
            outbox.push_front(msg);
            DrainOutcome::Idle
        }
        Err(e) => {
            if matches!(e, CycBoxError::Connection(_)) {
                outbox.push_front(msg);
                engine.warn(&format!(
                    "Connection {connection_id} retry send error: {e}, reconnecting..."
                ));
                DrainOutcome::Reconnect
            } else {
                engine.error(&format!("Connection {connection_id} retry send error: {e}"));
                DrainOutcome::Idle
            }
        }
    }
}
