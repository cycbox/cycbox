use async_trait::async_trait;
use bytes::BytesMut;
use cycbox_sdk::prelude::*;

pub const TIMEOUT_CODEC_ID: &str = "timeout_codec";

pub struct TimeoutCodec {
    timeout_ms: u32,
}

impl TimeoutCodec {
    #[allow(dead_code)]
    pub fn new(timeout_ms: u32) -> Self {
        Self { timeout_ms }
    }
}

impl Default for TimeoutCodec {
    fn default() -> Self {
        Self { timeout_ms: 400 }
    }
}

#[async_trait]
impl Codec for TimeoutCodec {
    fn decode(&mut self, _src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        // TimeoutCodec doesn't decode on regular data arrival
        // It only decodes when a timeout occurs (via decode_timeout)
        Ok(None)
    }

    fn decode_timeout(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        if src.is_empty() {
            return Ok(None);
        }
        // On timeout, treat all buffered data as a complete frame
        let data = src.split_to(src.len());
        let builder = MessageBuilder::rx(PayloadType::Binary, data.to_vec(), data.to_vec());
        Ok(Some(builder.build()))
    }

    fn encode(&mut self, item: &mut Message) -> Result<(), CycBoxError> {
        item.frame = item.payload.clone();
        Ok(())
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
                    values: Some(vec![FormValue::Integer(100)]),
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
                .unwrap_or(400);
        self.timeout_ms = timeout_ms as u32;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_codec_default() {
        let codec = TimeoutCodec::default();
        assert_eq!(codec.timeout_ms, 400);
    }

    #[test]
    fn test_timeout_codec_new() {
        let codec = TimeoutCodec::new(1000);
        assert_eq!(codec.timeout_ms, 1000);
    }

    #[test]
    fn test_decode_timeout_splits_frame() {
        let mut codec = TimeoutCodec::default();
        let mut src = BytesMut::from(&b"test data"[..]);
        let result = codec.decode_timeout(&mut src).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();
        assert_eq!(msg.payload, b"test data");
        // Buffer should be empty after timeout decode
        assert_eq!(src.len(), 0);
    }

    #[test]
    fn test_decode_timeout_empty() {
        let mut codec = TimeoutCodec::default();
        let mut src = BytesMut::new();
        let result = codec.decode_timeout(&mut src).unwrap();
        assert!(result.is_none());
    }
}
