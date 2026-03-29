use crate::connection::Connection;
use crate::engine::EngineRef;
use crate::formatter::get_encoding_from_name;
use cycbox_sdk::manifest::FormUtils;
use cycbox_sdk::prelude::*;
use log::warn;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

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

            engine.info(&format!("Connection {connection_id} established"));

            // RX/TX loop
            loop {
                tokio::select! {
                    _ = ctx.cancelled() => break 'outer,
                    result = connection.recv() => {
                        match result {
                            Ok(Some(msg)) => engine.receive_message(msg).await,
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
                        match connection.send(msg).await {
                            Ok(tx_msg) => engine.send_confirm(tx_msg).await,
                            Err(e) => {
                                if matches!(e, CycBoxError::Connection(_)) {
                                    engine.warn(&format!("Connection {connection_id} send error: {e}, reconnecting..."));
                                    break;
                                } else {
                                    engine.error(&format!("Connection {connection_id} send error: {e}"));
                                }
                            }
                        }
                    }
                    Some((cmd, resp_sender)) = command_receiver.recv() => {
                        let response = connection.handle_command(&cmd).await;
                        let _ = resp_sender.send(response);
                    }
                }
            }
        }
    })
}
