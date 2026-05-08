use async_trait::async_trait;
use bytes::BytesMut;
use cycbox_sdk::prelude::*;
use log::info;
use std::time::Instant;

pub const TIMEOUT_CODEC_ID: &str = "timeout_codec";

pub struct TimeoutCodec {
    timeout_ms: u32,
    /// Instant when the current frame started buffering (first byte after last emit).
    /// None means no data has arrived yet for the current frame.
    frame_start: Option<Instant>,
}

impl TimeoutCodec {
    #[allow(dead_code)]
    pub fn new(timeout_ms: u32) -> Self {
        Self {
            timeout_ms,
            frame_start: None,
        }
    }

    fn emit(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        self.frame_start = None;
        let data = src.split_to(src.len());
        let frame = data.to_vec();
        let payload = frame.clone();
        Ok(Some(
            MessageBuilder::rx(PayloadType::Binary, payload, frame).build(),
        ))
    }
}

impl Default for TimeoutCodec {
    fn default() -> Self {
        Self {
            timeout_ms: 50,
            frame_start: None,
        }
    }
}

#[async_trait]
impl Codec for TimeoutCodec {
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        if src.is_empty() {
            return Ok(None);
        }
        let now = Instant::now();
        // Record the start of a new frame on the first byte after the last emit.
        let start = self.frame_start.get_or_insert(now);
        // Emit once timeout_ms has elapsed since frame start, even if data is still arriving.
        // This handles the case where the transport never goes quiet and decode_timeout is never called.
        if now.duration_since(*start).as_millis() >= self.timeout_ms as u128 {
            return self.emit(src);
        }
        Ok(None)
    }

    fn decode_timeout(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        if src.is_empty() {
            return Ok(None);
        }
        // Transport went quiet before timeout_ms elapsed in decode(); emit whatever is buffered.
        self.emit(src)
    }

    fn encode(&mut self, item: &mut Message) -> Result<(), CycBoxError> {
        item.frame = item.payload.clone();
        Ok(())
    }

    fn reset(&mut self) {
        self.frame_start = None;
    }
}

#[async_trait]
impl Manifestable for TimeoutCodec {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: TIMEOUT_CODEC_ID.to_string(),
            name: l10n.get(locale, "timeout-codec"),
            description: l10n.get(locale, "timeout-codec-description"),
            category: PluginCategory::Codec,
            config_schema: vec![FormGroup {
                key: TIMEOUT_CODEC_ID.to_string(),
                label: l10n.get(locale, "timeout-codec"),
                description: Some(l10n.get(locale, "timeout-codec-timeout-description")),
                fields: vec![FormField {
                    key: "with_receive_timeout".to_string(),
                    field_type: FieldType::IntegerInput,
                    label: l10n.get(locale, "timeout-codec-timeout-label"),
                    description: None,
                    values: Some(vec![FormValue::Integer(50)]),
                    options: None,
                    is_required: true,
                    condition: None,
                    span: 6,
                }],
                condition: None,
            }],
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for TimeoutCodec {
    async fn config(&mut self, config: &[FormGroup]) -> Result<(), CycBoxError> {
        let timeout_ms =
            FormUtils::get_integer_value(config, TIMEOUT_CODEC_ID, "with_receive_timeout")
                .unwrap_or(50);
        info!("Configuring TimeoutCodec with timeout_ms: {}", timeout_ms);
        self.timeout_ms = timeout_ms as u32;
        Ok(())
    }
}
