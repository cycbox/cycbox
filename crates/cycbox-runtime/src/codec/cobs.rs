use async_trait::async_trait;
use bytes::BytesMut;
use cycbox_sdk::prelude::*;

pub const COBS_CODEC_ID: &str = "cobs_codec";

/// CobsCodec implements Consistent Overhead Byte Stuffing (COBS) encoding/decoding.
/// COBS is a framing algorithm that eliminates a specific byte value (sentinel) from
/// the data stream, so that byte can be used as a frame delimiter.
#[derive(Debug, Clone, Default)]
pub struct CobsCodec {
    sentinel: u8,
}

impl CobsCodec {
    /// COBS encode a payload
    fn encode_cobs(&self, data: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(data.len() + (data.len() / 254) + 2);
        let mut code_index = 0;
        let mut code = 1u8;

        encoded.push(0); // Placeholder for first code byte

        for &byte in data {
            if byte == self.sentinel {
                encoded[code_index] = code;
                code_index = encoded.len();
                encoded.push(0); // Placeholder for next code byte
                code = 1;
            } else {
                encoded.push(byte);
                code = code.saturating_add(1);

                if code == 0xFF {
                    encoded[code_index] = code;
                    code_index = encoded.len();
                    encoded.push(0); // Placeholder for next code byte
                    code = 1;
                }
            }
        }

        encoded[code_index] = code;
        encoded.push(self.sentinel); // Frame delimiter

        encoded
    }

    /// COBS decode a payload
    fn decode_cobs(&self, data: &[u8]) -> Result<Vec<u8>, CycBoxError> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let mut decoded = Vec::with_capacity(data.len());
        let mut i = 0;

        while i < data.len() {
            let code = data[i] as usize;

            if code == 0 {
                return Err(CycBoxError::Parse(
                    "Invalid COBS encoding: unexpected zero byte".to_string(),
                ));
            }

            i += 1;

            // Copy the next (code - 1) bytes
            let copy_len = (code - 1).min(data.len() - i);
            if i + copy_len > data.len() {
                return Err(CycBoxError::Parse(
                    "Invalid COBS encoding: code byte exceeds buffer".to_string(),
                ));
            }

            decoded.extend_from_slice(&data[i..i + copy_len]);
            i += copy_len;

            // If code is not 0xFF and we're not at the end, add a sentinel byte
            if code != 0xFF && i < data.len() {
                decoded.push(self.sentinel);
            }
        }

        Ok(decoded)
    }
}

#[async_trait]
impl Codec for CobsCodec {
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        let buffer = src.as_ref();

        // Look for sentinel byte (frame delimiter)
        if let Some(index) = buffer.iter().position(|&b| b == self.sentinel) {
            // Extract frame including the sentinel
            let frame = src.split_to(index + 1);

            // Decode COBS (exclude the sentinel delimiter at the end)
            let encoded_data = &frame[..frame.len() - 1];

            // Skip empty frames (handles leading sentinel bytes for noise flushing)
            if encoded_data.is_empty() {
                return Ok(None);
            }

            let payload = self.decode_cobs(encoded_data)?;
            let message = MessageBuilder::rx(PayloadType::Binary, payload, frame).build();

            return Ok(Some(message));
        }

        Ok(None)
    }

    fn encode(&mut self, item: &mut Message) -> Result<(), CycBoxError> {
        // Encode the payload using COBS
        let encoded = self.encode_cobs(&item.payload);
        item.frame = encoded;
        Ok(())
    }
}

#[async_trait]
impl Manifestable for CobsCodec {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: COBS_CODEC_ID.to_string(),
            name: l10n.get(locale, "cobs-codec"),
            description: l10n.get(locale, "cobs-codec-description"),
            category: PluginCategory::Codec,
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for CobsCodec {
    async fn config(&mut self, _config: &[FormGroup]) -> Result<(), CycBoxError> {
        // COBS implementation only supports sentinel = 0x00
        // (code bytes range 1-255, would conflict with non-zero sentinels)
        self.sentinel = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn test_cobs_encode_decode() {
        type TestCase = (Vec<u8>, Vec<u8>); // (input, expected_encoded)
        let test_cases: Vec<TestCase> = vec![
            (vec![], vec![0x01, 0x00]),
            (vec![0x00], vec![0x01, 0x01, 0x00]),
            (vec![0x00, 0x00], vec![0x01, 0x01, 0x01, 0x00]),
            (vec![0x00, 0x11, 0x00], vec![0x01, 0x02, 0x11, 0x01, 0x00]),
            (
                vec![0x11, 0x22, 0x00, 0x33],
                vec![0x03, 0x11, 0x22, 0x02, 0x33, 0x00],
            ),
            (
                vec![0x11, 0x22, 0x33, 0x44],
                vec![0x05, 0x11, 0x22, 0x33, 0x44, 0x00],
            ),
        ];
        for (input, expected_encoded) in test_cases {
            let codec = CobsCodec::default();
            let encoded = codec.encode_cobs(&input);
            assert_eq!(
                encoded, expected_encoded,
                "Encoding failed for input: {:?}",
                input
            );

            // Strip the frame delimiter (last byte) before decoding, just like the production code does
            let encoded_without_delimiter = &encoded[..encoded.len() - 1];
            let decoded = codec
                .decode_cobs(encoded_without_delimiter)
                .expect("Decoding failed");
            assert_eq!(
                decoded, input,
                "Decoded output does not match original input"
            );
        }
    }

    #[test]
    fn test_cobs_codec_with_leading_sentinel() {
        let mut codec = CobsCodec::default();
        let mut buffer = BytesMut::new();

        // Simulate receiving data with leading sentinel bytes (0x00)
        // Format: [0x00, 0x00, COBS_data, 0x00]
        let payload = vec![0x11, 0x22, 0x33];
        let encoded = codec.encode_cobs(&payload);

        // Add leading sentinels
        buffer.extend_from_slice(&[0x00, 0x00]);
        buffer.extend_from_slice(&encoded);

        // First decode: should skip first empty frame (leading 0x00)
        let result1 = codec.decode(&mut buffer).expect("Decode should succeed");
        assert!(
            result1.is_none(),
            "First leading sentinel should be skipped"
        );

        // Second decode: should skip second empty frame (second leading 0x00)
        let result2 = codec.decode(&mut buffer).expect("Decode should succeed");
        assert!(
            result2.is_none(),
            "Second leading sentinel should be skipped"
        );

        // Third decode: should decode the actual data
        let result3 = codec.decode(&mut buffer).expect("Decode should succeed");
        assert!(result3.is_some(), "Should decode actual data");
        let msg = result3.unwrap();
        assert_eq!(msg.payload, payload, "Payload should match original");

        // Buffer should be empty
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_cobs_codec_without_leading_sentinel() {
        let mut codec = CobsCodec::default();
        let mut buffer = BytesMut::new();

        // Normal case: just COBS_data without leading sentinel
        let payload = vec![0x11, 0x22, 0x33];
        let encoded = codec.encode_cobs(&payload);
        buffer.extend_from_slice(&encoded);

        // Should decode immediately
        let result = codec.decode(&mut buffer).expect("Decode should succeed");
        assert!(result.is_some(), "Should decode data");
        let msg = result.unwrap();
        assert_eq!(msg.payload, payload);
    }
}
