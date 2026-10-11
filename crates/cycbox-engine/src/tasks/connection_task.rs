use crate::connection::{Connection, Sent};
use crate::engine::EngineRef;
use crate::formatter::get_encoding_from_name;
use cycbox_sdk::manifest::FormUtils;
use cycbox_sdk::prelude::*;
use cycbox_sdk::transport::LOG_LEVEL_VALUE_ID;
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
/// Bound for the per-connection transport sink. It carries raw RX chunks (connection 0
/// only) as well as logs and events, so it is sized for a read burst; overflow is
/// dropped, counted, and reported as one warning line.
const SINK_BUF: usize = 1024;
/// Wait after a failure the transport told us not to retry soon
/// (`CycBoxError::ConnectionBackoff`), plus a random share of `SLOW_BACKOFF_JITTER`.
///
/// The case these are sized for is two app instances pointed at one p2p node that serves
/// a single client: each one's arrival kicks the other, so with the ordinary 1 s backoff
/// they take the node from each other twice a second forever and neither is usable. A
/// wait long enough for the other instance to get real work done is the only thing that
/// breaks it.
const SLOW_BACKOFF: Duration = Duration::from_secs(30);
const SLOW_BACKOFF_JITTER: Duration = Duration::from_secs(15);

enum DrainOutcome {
    Idle,
    Reconnect,
    /// Reconnect, but wait out [`slow_backoff`] first — the transport reported a failure
    /// that will repeat if it is retried in a second.
    ReconnectSlowly,
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

        // One sink for the life of this connection, across reconnects, so a transport's
        // background tasks can report from inside `connect`. Raw RX is only wanted for
        // connection 0, which the UI's raw view shows.
        let (sink, mut sink_receiver) = TransportSink::new(SINK_BUF, connection_id == 0);

        // Reconnection loop with exponential backoff
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(10);
        let mut reconnecting = false;

        'outer: loop {
            if ctx.is_cancelled() {
                break;
            }

            // Apply backoff delay (skip on first attempt). The sink keeps being read
            // meanwhile: a slow backoff lasts minutes, and whatever the last session said
            // on its way down belongs before the next "established" line, not after it.
            if reconnecting {
                let wake = tokio::time::sleep(backoff);
                tokio::pin!(wake);
                loop {
                    tokio::select! {
                        _ = ctx.cancelled() => break 'outer,
                        _ = &mut wake => break,
                        Some(msg) = sink_receiver.recv() => {
                            handle_sink_message(&engine, connection_id, &sink, msg).await;
                        }
                    }
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
                .create_transport(
                    &transport_id,
                    &config,
                    codec,
                    TransportContext::new(sink.clone(), timeout),
                )
                .await
            {
                Ok(t) => t,
                Err(e) => {
                    // What the transport reported while failing explains the failure, so
                    // it goes out before the line that announces it.
                    drain_sink(&engine, connection_id, &sink, &mut sink_receiver).await;
                    // Only reconnect for IO/connection failures; config errors are fatal
                    if e.is_reconnectable() {
                        if e.wants_slow_retry() {
                            backoff = slow_backoff();
                        }
                        warn!("Connection {connection_id} transport connection error: {e}, reconnecting...");
                        engine.warn(&format!("Connection {connection_id} transport error: {e}, retrying in {:?}...", backoff));
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

            // Connection established — reset backoff.
            backoff = Duration::from_secs(1);

            let mut connection = Connection::new(connection_id, transport, transformer, encoding);

            // Per-connection outbox for messages deferred by codec back-pressure
            // (e.g. half-duplex Modbus RTU returning `CycBoxError::Pending` while a
            // request is awaiting its response).
            let mut pending_outbox: VecDeque<Message> = VecDeque::new();
            let mut outbox_warned = false;
            // Set when the failure that ends this session is one the transport said not
            // to retry soon. Read once, below the RX/TX loop, because every path out of
            // that loop is a `break` and the backoff is applied at the top of `'outer`.
            let mut slow_retry = false;
            let mut retry_interval = tokio::time::interval(OUTBOX_RETRY_INTERVAL);
            retry_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

            engine.info(&format!("Connection {connection_id} established"));

            // RX/TX loop
            loop {
                tokio::select! {
                    _ = ctx.cancelled() => {
                        // Cooperative shutdown — give the transport a chance to
                        // tear itself down.
                        connection.close().await;
                        break 'outer;
                    }
                    result = connection.recv() => {
                        match result {
                            Ok(Some(msg)) => {
                                engine.receive_message(msg).await;
                                // A successful decode typically clears the codec's pending
                                // state, so try to drain the next queued message.
                                match drain_outbox(&mut pending_outbox, &mut connection, &engine, connection_id).await {
                                    DrainOutcome::Idle => {}
                                    outcome => {
                                        slow_retry = matches!(outcome, DrainOutcome::ReconnectSlowly);
                                        reconnecting = true;
                                        break;
                                    }
                                }
                            }
                            Ok(None) => {
                                engine.warn(&format!("Connection {connection_id} lost, reconnecting..."));
                                reconnecting = true;
                                break;
                            }
                            Err(e) => {
                                if e.is_reconnectable() {
                                    if e.wants_slow_retry() {
                                        slow_retry = true;
                                    }
                                    engine.warn(&format!("Connection {connection_id} recv error: {e}, reconnecting..."));
                                    reconnecting = true;
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
                                Ok(sent) => report_sent(&engine, connection_id, sent).await,
                                Err(CycBoxError::Pending(_)) => {
                                    enqueue_outbox(
                                        msg,
                                        &mut pending_outbox,
                                        &engine,
                                        connection_id,
                                        &mut outbox_warned,
                                    );
                                }
                                Err(CycBoxError::Discarded(reason)) => {
                                    // Transport accepted-but-dropped (e.g. server with
                                    // no client). Stay connected, just log it.
                                    engine.warn(&format!("Connection {connection_id} send discarded: {reason}"));
                                }
                                Err(e) => {
                                    if e.is_reconnectable() {
                                        if e.wants_slow_retry() {
                                            slow_retry = true;
                                        }
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
                            match drain_outbox(&mut pending_outbox, &mut connection, &engine, connection_id).await {
                                DrainOutcome::Idle => {}
                                outcome => {
                                    slow_retry = matches!(outcome, DrainOutcome::ReconnectSlowly);
                                    reconnecting = true;
                                    break;
                                }
                            }
                        }
                    }
                    _ = retry_interval.tick() => {
                        // Periodic drain handles the case where the codec clears its
                        // pending state via timeout (no response decoded).
                        if !pending_outbox.is_empty() {
                            match drain_outbox(&mut pending_outbox, &mut connection, &engine, connection_id).await {
                                DrainOutcome::Idle => {}
                                outcome => {
                                    slow_retry = matches!(outcome, DrainOutcome::ReconnectSlowly);
                                    reconnecting = true;
                                    break;
                                }
                            }
                        }
                    }
                    Some((cmd, resp_sender)) = command_receiver.recv() => {
                        let response = connection.handle_command(&cmd).await;
                        let _ = resp_sender.send(response);
                    }
                    Some(msg) = sink_receiver.recv() => {
                        handle_sink_message(&engine, connection_id, &sink, msg).await;
                    }
                }
            }

            // Drop the transport first, so whatever it reports while going away (a server's
            // last `client_disconnected`) is in the sink, then hand all of it on now rather
            // than after the next session is up.
            drop(connection);
            drain_sink(&engine, connection_id, &sink, &mut sink_receiver).await;

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

            // The session that just ended failed in a way the transport says will repeat
            // — a p2p node handing itself to another client, typically. Reconnecting in a
            // second would take it straight back off them and start the fight again, so
            // wait long enough for the other side to be worth something, and say so: a
            // minutes-long silence with no explanation looks exactly like a broken app.
            if slow_retry {
                backoff = slow_backoff();
                engine.warn(&format!(
                    "Connection {connection_id} will not retry for {:?} — the last failure \
                     would repeat immediately",
                    backoff
                ));
            }
        }

        // Stop, or a fatal error: the transport is gone, and what it said last still goes
        // out (Stop swaps in an empty Lua script first, so a late event is only broadcast).
        drain_sink(&engine, connection_id, &sink, &mut sink_receiver).await;
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
        Ok(sent) => {
            report_sent(engine, connection_id, sent).await;
            DrainOutcome::Idle
        }
        Err(CycBoxError::Pending(_)) => {
            outbox.push_front(msg);
            DrainOutcome::Idle
        }
        Err(CycBoxError::Discarded(reason)) => {
            // Drop the message — re-queueing it would just spin until a peer
            // appears, and by then the request/response correlation is stale.
            engine.warn(&format!(
                "Connection {connection_id} retry send discarded: {reason}"
            ));
            DrainOutcome::Idle
        }
        Err(e) => {
            if e.is_reconnectable() {
                outbox.push_front(msg);
                engine.warn(&format!(
                    "Connection {connection_id} retry send error: {e}, reconnecting..."
                ));
                if e.wants_slow_retry() {
                    DrainOutcome::ReconnectSlowly
                } else {
                    DrainOutcome::Reconnect
                }
            } else {
                engine.error(&format!("Connection {connection_id} retry send error: {e}"));
                DrainOutcome::Idle
            }
        }
    }
}

/// Route one message a transport pushed on its sink, stamped with the connection id:
/// `LOG` → a log line, `EVENT` → the Lua `on_event` hook then broadcast, anything else
/// (`RAW_RX`) → broadcast. Then report anything the sink had to drop.
async fn handle_sink_message(
    engine: &EngineRef,
    connection_id: u32,
    sink: &TransportSink,
    mut msg: Message,
) {
    msg.connection_id = connection_id;
    match msg.message_type.as_str() {
        MESSAGE_TYPE_LOG => {
            let level = msg
                .get_value(LOG_LEVEL_VALUE_ID)
                .and_then(|v| v.as_string())
                .and_then(|l| LogLevel::parse(&l))
                .unwrap_or(LogLevel::Info);
            engine.log_for(connection_id, level, &String::from_utf8_lossy(&msg.payload));
        }
        MESSAGE_TYPE_EVENT => engine.transport_event(msg).await,
        _ => engine.broadcast(msg),
    }
    let dropped = sink.take_dropped();
    if dropped > 0 {
        engine.warn(&format!(
            "Connection {connection_id} dropped {dropped} transport message(s): sink full"
        ));
    }
}

/// Hand on everything already in the sink, without waiting for more. For the moments
/// the RX/TX loop is not running to read it: after a failed connect and after a session
/// ends.
async fn drain_sink(
    engine: &EngineRef,
    connection_id: u32,
    sink: &TransportSink,
    receiver: &mut Receiver<Message>,
) {
    while let Ok(msg) = receiver.try_recv() {
        handle_sink_message(engine, connection_id, sink, msg).await;
    }
}

/// Act on what [`Connection::send`] did: confirm TX data, or log the outcome of a
/// `REQUEST` command (D8). Whatever else a command wants Lua and the UI to see, the
/// transport reports as an `EVENT` on its sink.
async fn report_sent(engine: &EngineRef, connection_id: u32, sent: Sent) {
    match sent {
        Sent::Tx(tx_msg) => engine.send_confirm(tx_msg).await,
        Sent::Command {
            command,
            response: Some(response),
        } => {
            let success = response
                .metadata_value("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if success {
                engine.log_for(
                    connection_id,
                    LogLevel::Info,
                    &format!("Connection {connection_id} command '{command}' succeeded"),
                );
            } else {
                let error = response
                    .metadata_value("error")
                    .and_then(|v| v.as_string())
                    .unwrap_or_else(|| "unknown error".to_string());
                engine.log_for(
                    connection_id,
                    LogLevel::Warning,
                    &format!("Connection {connection_id} command '{command}' failed: {error}"),
                );
            }
        }
        Sent::Command {
            command,
            response: None,
        } => engine.log_for(
            connection_id,
            LogLevel::Warning,
            &format!("Connection {connection_id} has no handler for command '{command}'"),
        ),
    }
}

/// The wait to use after a failure the transport said would repeat.
///
/// **Jitter is the load-bearing part, not the length.** Two app instances kicking each
/// other off one node fail at the same moment and would otherwise wake at the same moment
/// too, forever — a fixed delay only makes the fight slower, it does not end it. A random
/// spread means one of them gets a clear run.
///
/// **Flat rather than escalating, deliberately.** The outcome this produces is the pair
/// alternating: one works for half a minute, the other takes over, and so on, with the log
/// saying why each pause happened. An escalating backoff would instead starve whichever
/// instance lost first, which is worse for the case that actually happens — somebody
/// leaving an app open on another machine.
///
/// The randomness comes from the clock rather than from a `rand` dependency: this is a
/// tie-breaker between two processes, not a security primitive, and the two do not fail in
/// the same nanosecond.
fn slow_backoff() -> Duration {
    let spread = SLOW_BACKOFF_JITTER.as_millis() as u64;
    let jitter = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::from(d.subsec_nanos()) % spread)
        .unwrap_or(0);
    SLOW_BACKOFF + Duration::from_millis(jitter)
}
