use crate::parser::{CBRT_SYNC, ParseOutcome, SessionState, parse_at};
use async_trait::async_trait;
use cycbox_sdk::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;

pub const CBRT_TRANSFORMER_ID: &str = "cbrt_transformer";

/// Sentinel metadata key the codec writes on every successful decode. If present
/// the transformer skips parsing to avoid double-populating values/metadata when
/// both the codec and the transformer are configured on the same connection.
const CODEC_SENTINEL: &str = "datatype";

#[derive(Debug, Default)]
pub struct CbrtTransformer {
    /// Per-connection session state. Each message-based connection (UDP peer, MQTT
    /// client) gets its own slot so wrap counters, sequence drops, and the ts
    /// anchor track that source independently.
    sessions: Mutex<HashMap<u32, SessionState>>,
}

impl CbrtTransformer {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Transformer for CbrtTransformer {
    fn on_receive(&self, message: &mut Message) -> Result<(), CycBoxError> {
        // Skip if the codec already parsed this frame on the same pipeline.
        if message.metadata_value(CODEC_SENTINEL).is_some() {
            return Ok(());
        }

        if message.payload.len() < CBRT_SYNC.len()
            || message.payload[..CBRT_SYNC.len()] != CBRT_SYNC
        {
            return Err(CycBoxError::InvalidFormat(
                "cbrt: payload does not start with CBRT sync word".to_string(),
            ));
        }

        let arrival_us = if message.timestamp != 0 {
            message.timestamp
        } else {
            Message::current_timestamp()
        };

        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| CycBoxError::InvalidFormat("cbrt: session lock poisoned".to_string()))?;
        let state = sessions.entry(message.connection_id).or_default();

        match parse_at(state, &message.payload, 0, arrival_us) {
            ParseOutcome::Complete {
                frame_end,
                message: parsed,
            } => {
                if frame_end != message.payload.len() {
                    log::debug!(
                        "cbrt transformer: {} trailing byte(s) after frame on conn {}",
                        message.payload.len() - frame_end,
                        message.connection_id
                    );
                }
                message.values.extend(parsed.values);
                message.metadata.extend(parsed.metadata);
                if message.contents.is_empty() {
                    message.contents = parsed.contents;
                } else {
                    message.contents.extend(parsed.contents);
                }
                Ok(())
            }
            ParseOutcome::NeedMore => Err(CycBoxError::InvalidFormat(
                "cbrt: payload truncated mid-frame".to_string(),
            )),
            ParseOutcome::Reject => {
                // Reset on reject so a corrupted message doesn't poison the
                // session — the next valid frame establishes a fresh session.
                state.reset();
                Err(CycBoxError::InvalidFormat(
                    "cbrt: frame validation failed".to_string(),
                ))
            }
        }
    }
}

#[async_trait]
impl Manifestable for CbrtTransformer {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: CBRT_TRANSFORMER_ID.to_string(),
            name: l10n.get(locale, "cbrt-transformer"),
            description: l10n.get(locale, "cbrt-transformer-description"),
            category: PluginCategory::Transformer,
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for CbrtTransformer {}
