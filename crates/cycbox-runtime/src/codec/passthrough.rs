use async_trait::async_trait;
use bytes::BytesMut;
use cycbox_sdk::prelude::*;

pub const PASSTHROUGH_CODEC_ID: &str = "passthrough_codec";

/// PassthroughCodec passes all buffered data as messages immediately.
/// It has no configuration options and acts as the default codec.
#[derive(Debug, Clone, Default)]
pub struct PassthroughCodec;

impl PassthroughCodec {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Codec for PassthroughCodec {
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        if src.is_empty() {
            return Ok(None);
        }
        // Pass through all buffered data as a message
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
impl Manifestable for PassthroughCodec {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: PASSTHROUGH_CODEC_ID.to_string(),
            name: l10n.get(locale, "passthrough-codec"),
            description: l10n.get(locale, "passthrough-codec-description"),
            category: PluginCategory::Codec,
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for PassthroughCodec {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_codec_new() {
        let _codec = PassthroughCodec::new();
    }

    #[test]
    fn test_passthrough_codec_default() {
        let _codec = PassthroughCodec::default();
    }

    #[test]
    fn test_decode_timeout_same_as_decode() {
        let mut codec = PassthroughCodec::default();
        let mut src = BytesMut::from(&b"timeout data"[..]);
        let result = codec.decode_timeout(&mut src).unwrap();
        assert!(result.is_some());
        let msg = result.unwrap();
        assert_eq!(msg.payload, b"timeout data");
        assert_eq!(src.len(), 0);
    }
}
