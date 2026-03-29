use async_trait::async_trait;
use bytes::BytesMut;
use cycbox_sdk::prelude::*;
pub const SLIP_CODEC_ID: &str = "slip_codec";

// SLIP protocol constants
const END: u8 = 0xC0; // Frame delimiter
const ESC: u8 = 0xDB; // Escape character
const ESC_END: u8 = 0xDC; // Escaped END byte
const ESC_ESC: u8 = 0xDD; // Escaped ESC byte

/// SlipCodec implements the Serial Line Internet Protocol (SLIP) framing.
/// SLIP is a simple framing protocol that uses special delimiter bytes to separate
/// packets sent over serial connections.
///
/// Protocol bytes:
/// - END (0xC0): Frame delimiter
/// - ESC (0xDB): Escape character
/// - ESC_END (0xDC): Represents END when escaped
/// - ESC_ESC (0xDD): Represents ESC when escaped
#[derive(Debug, Clone)]
pub struct SlipCodec {
    /// Whether to push a leading END byte before the frame.
    /// This helps flush accumulated noise on noisy connections.
    push_leading_end: bool,
}

impl SlipCodec {
    /// SLIP encode a payload
    ///
    /// Escapes special bytes:
    /// - END (0xC0) → ESC + ESC_END (0xDB 0xDC)
    /// - ESC (0xDB) → ESC + ESC_ESC (0xDB 0xDD)
    ///
    /// Frames the result with END bytes at both ends (optionally).
    fn encode_slip(&self, data: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(data.len() + 10);

        // Leading END byte to flush any accumulated noise (if enabled)
        if self.push_leading_end {
            encoded.push(END);
        }

        for &byte in data {
            match byte {
                END => {
                    encoded.push(ESC);
                    encoded.push(ESC_END);
                }
                ESC => {
                    encoded.push(ESC);
                    encoded.push(ESC_ESC);
                }
                _ => {
                    encoded.push(byte);
                }
            }
        }

        // Trailing END byte to mark frame end
        encoded.push(END);

        encoded
    }

    /// SLIP decode a payload
    ///
    /// Unescapes special byte sequences:
    /// - ESC + ESC_END (0xDB 0xDC) → END (0xC0)
    /// - ESC + ESC_ESC (0xDB 0xDD) → ESC (0xDB)
    fn decode_slip(&self, data: &[u8]) -> Result<Vec<u8>, CycBoxError> {
        let mut decoded = Vec::with_capacity(data.len());
        let mut i = 0;

        while i < data.len() {
            let byte = data[i];

            if byte == ESC {
                // Must have a following byte
                if i + 1 >= data.len() {
                    return Err(CycBoxError::Parse(
                        "Invalid SLIP encoding: ESC at end of frame".to_string(),
                    ));
                }

                let next_byte = data[i + 1];
                match next_byte {
                    ESC_END => {
                        decoded.push(END);
                        i += 2;
                    }
                    ESC_ESC => {
                        decoded.push(ESC);
                        i += 2;
                    }
                    _ => {
                        return Err(CycBoxError::Parse(format!(
                            "Invalid SLIP encoding: ESC followed by unexpected byte 0x{:02X}",
                            next_byte
                        )));
                    }
                }
            } else if byte == END {
                // END bytes should not appear in the frame data (only as delimiters)
                return Err(CycBoxError::Parse(
                    "Invalid SLIP encoding: unescaped END byte in frame data".to_string(),
                ));
            } else {
                decoded.push(byte);
                i += 1;
            }
        }

        Ok(decoded)
    }
}

impl Default for SlipCodec {
    fn default() -> Self {
        Self {
            push_leading_end: true,
        }
    }
}

#[async_trait]
impl Codec for SlipCodec {
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        let buffer = src.as_ref();

        // Look for END byte (frame delimiter)
        if let Some(end_pos) = buffer.iter().position(|&b| b == END) {
            // Extract everything up to and including the END byte
            let frame_with_end = src.split_to(end_pos + 1);

            // Skip the frame if it's empty (just an END byte) or contains only END bytes
            // This handles leading END bytes that flush noise
            let frame_data = &frame_with_end[..frame_with_end.len() - 1];
            if frame_data.is_empty() {
                // Empty frame, continue looking for next frame
                return Ok(None);
            }

            // Decode SLIP (exclude the trailing END delimiter)
            let payload = self.decode_slip(frame_data)?;
            let message = MessageBuilder::rx(PayloadType::Binary, payload, frame_with_end).build();

            return Ok(Some(message));
        }

        Ok(None)
    }

    fn encode(&mut self, item: &mut Message) -> Result<(), CycBoxError> {
        // Encode the payload using SLIP
        let encoded = self.encode_slip(&item.payload);
        item.frame = encoded;
        Ok(())
    }
}

#[async_trait]
impl Manifestable for SlipCodec {
    async fn manifest(&self, locale: &str) -> Manifest {
        let l10n = crate::l10n::get_l10n();
        Manifest {
            id: SLIP_CODEC_ID.to_string(),
            name: l10n.get(locale, "slip-codec"),
            description: l10n.get(locale, "slip-codec-description"),
            category: PluginCategory::Codec,
            config_schema: vec![FormGroup {
                key: "slip".to_string(),
                label: l10n.get(locale, "slip-codec"),
                description: None,
                condition: None,
                fields: vec![FormField {
                    key: "slip_push_leading_end".to_string(),
                    field_type: FieldType::BooleanInput,
                    label: l10n.get(locale, "slip-push-leading-end-label"),
                    description: Some(l10n.get(locale, "slip-push-leading-end-description")),
                    values: Some(vec![FormValue::Boolean(true)]),
                    options: None,
                    is_required: false,
                    condition: None,
                    span: 12,
                }],
            }],
            ..Default::default()
        }
    }
}

#[async_trait]
impl Configurable for SlipCodec {
    async fn config(&mut self, config: &[FormGroup]) -> Result<(), CycBoxError> {
        // Read push_leading_end configuration (default: true)
        self.push_leading_end =
            FormUtils::get_boolean_value(config, "slip", "slip_push_leading_end").unwrap_or(true);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slip_encode_decode() {
        type TestCase = (Vec<u8>, Vec<u8>); // (input, expected_encoded_without_end_bytes)
        let test_cases: Vec<TestCase> = vec![
            // Empty payload
            (vec![], vec![]),
            // Simple data without special bytes
            (vec![0x11, 0x22, 0x33], vec![0x11, 0x22, 0x33]),
            // Data with END byte
            (vec![0x11, END, 0x22], vec![0x11, ESC, ESC_END, 0x22]),
            // Data with ESC byte
            (vec![0x11, ESC, 0x22], vec![0x11, ESC, ESC_ESC, 0x22]),
            // Data with both END and ESC
            (
                vec![END, 0x11, ESC, 0x22, END],
                vec![ESC, ESC_END, 0x11, ESC, ESC_ESC, 0x22, ESC, ESC_END],
            ),
            // Multiple consecutive special bytes
            (
                vec![END, END, ESC, ESC],
                vec![ESC, ESC_END, ESC, ESC_END, ESC, ESC_ESC, ESC, ESC_ESC],
            ),
        ];

        for (input, expected_middle) in test_cases {
            let codec = SlipCodec::default();
            let encoded = codec.encode_slip(&input);

            // Verify frame structure: END + encoded_data + END
            assert_eq!(encoded[0], END, "First byte should be END");
            assert_eq!(encoded[encoded.len() - 1], END, "Last byte should be END");

            // Verify middle part (excluding leading and trailing END)
            let middle = &encoded[1..encoded.len() - 1];
            assert_eq!(
                middle,
                expected_middle.as_slice(),
                "Encoding failed for input: {:?}",
                input
            );

            // Decode and verify
            let decoded = codec.decode_slip(middle).expect("Decoding failed");
            assert_eq!(
                decoded, input,
                "Decoded output does not match original input"
            );
        }
    }

    #[test]
    fn test_slip_decode_errors() {
        let codec = SlipCodec::default();

        // ESC at end of frame
        let result = codec.decode_slip(&[0x11, ESC]);
        assert!(result.is_err());

        // ESC followed by invalid byte
        let result = codec.decode_slip(&[0x11, ESC, 0x99]);
        assert!(result.is_err());

        // Unescaped END in frame data
        let result = codec.decode_slip(&[0x11, END, 0x22]);
        assert!(result.is_err());
    }

    #[test]
    fn test_slip_codec_decode() {
        let mut codec = SlipCodec::default();
        let mut buffer = BytesMut::new();

        // Test with a valid SLIP frame: END + data + END
        buffer.extend_from_slice(&[END, 0x11, 0x22, 0x33, END]);

        // First decode returns None (leading END creates empty frame)
        let result1 = codec.decode(&mut buffer).expect("Decode should succeed");
        assert!(result1.is_none());

        // Second decode returns the actual data
        let result2 = codec.decode(&mut buffer).expect("Decode should succeed");
        assert!(result2.is_some());
        let msg = result2.unwrap();
        assert_eq!(msg.payload, vec![0x11, 0x22, 0x33]);

        // Buffer should be empty after consuming the frame
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_slip_codec_empty_frames() {
        let mut codec = SlipCodec::default();
        let mut buffer = BytesMut::new();

        // Test with leading END bytes (noise flush)
        buffer.extend_from_slice(&[END, END, 0x11, 0x22, END]);

        // First decode returns None (first empty frame from leading END)
        let result1 = codec.decode(&mut buffer).expect("Decode should succeed");
        assert!(result1.is_none());

        // Second decode returns None (second empty frame from second leading END)
        let result2 = codec.decode(&mut buffer).expect("Decode should succeed");
        assert!(result2.is_none());

        // Third decode returns the actual data
        let result3 = codec.decode(&mut buffer).expect("Decode should succeed");
        assert!(result3.is_some());
        let msg = result3.unwrap();
        assert_eq!(msg.payload, vec![0x11, 0x22]);
    }

    #[test]
    fn test_slip_encoder_adds_end_bytes() {
        let codec = SlipCodec::default();
        let payload = vec![0x11, 0x22, 0x33];
        let encoded = codec.encode_slip(&payload);

        // Verify SLIP encoder adds END at both beginning and end
        assert_eq!(encoded[0], END, "First byte should be END");
        assert_eq!(encoded[encoded.len() - 1], END, "Last byte should be END");

        // Verify middle part is the payload (unescaped in this case)
        let middle = &encoded[1..encoded.len() - 1];
        assert_eq!(middle, payload.as_slice(), "Middle should be payload");
    }

    #[test]
    fn test_slip_push_leading_end_config() {
        // Test with push_leading_end = true (default)
        let codec_with_leading = SlipCodec {
            push_leading_end: true,
        };
        let payload = vec![0x11, 0x22, 0x33];
        let encoded_with = codec_with_leading.encode_slip(&payload);

        // Should have leading END
        assert_eq!(encoded_with[0], END, "Should have leading END when enabled");
        assert_eq!(
            encoded_with[encoded_with.len() - 1],
            END,
            "Should have trailing END"
        );
        assert_eq!(
            encoded_with.len(),
            payload.len() + 2,
            "Should have 2 END bytes"
        );

        // Test with push_leading_end = false
        let codec_without_leading = SlipCodec {
            push_leading_end: false,
        };
        let encoded_without = codec_without_leading.encode_slip(&payload);

        // Should NOT have leading END, only trailing
        assert_eq!(
            encoded_without[0], payload[0],
            "First byte should be payload data"
        );
        assert_eq!(
            encoded_without[encoded_without.len() - 1],
            END,
            "Should have trailing END"
        );
        assert_eq!(
            encoded_without.len(),
            payload.len() + 1,
            "Should have only 1 END byte"
        );

        // Verify middle matches payload
        assert_eq!(
            &encoded_without[..encoded_without.len() - 1],
            payload.as_slice()
        );
    }

    #[test]
    fn test_slip_codec_with_user_added_end_bytes() {
        let mut codec = SlipCodec::default();
        let mut buffer = BytesMut::new();

        // Simulate user manually adding END bytes before and after SLIP frame
        // SLIP encoder already adds: [END, data, END]
        // User adds more: [END, [END, data, END], END]
        let payload = vec![0x11, 0x22, 0x33];
        let encoded = codec.encode_slip(&payload); // Already [END, data, END]

        // User adds extra END bytes
        buffer.extend_from_slice(&[END]); // Extra leading END
        buffer.extend_from_slice(&encoded); // [END, data, END]
        buffer.extend_from_slice(&[END]); // Extra trailing END

        // Buffer now: [END, END, data, END, END]

        // Decode sequence:
        // 1. First END creates empty frame → None
        let r1 = codec.decode(&mut buffer).unwrap();
        assert!(r1.is_none(), "Extra leading END creates empty frame");

        // 2. Second END (from encoder) creates empty frame → None
        let r2 = codec.decode(&mut buffer).unwrap();
        assert!(r2.is_none(), "Encoder's leading END creates empty frame");

        // 3. Third END (from encoder) ends the data frame → Some(message)
        let r3 = codec.decode(&mut buffer).unwrap();
        assert!(r3.is_some(), "Should decode actual data");
        assert_eq!(r3.unwrap().payload, payload);

        // 4. Fourth END (extra trailing) creates empty frame → None
        let r4 = codec.decode(&mut buffer).unwrap();
        assert!(r4.is_none(), "Extra trailing END creates empty frame");

        // Buffer should be empty
        assert_eq!(buffer.len(), 0);
    }
}
