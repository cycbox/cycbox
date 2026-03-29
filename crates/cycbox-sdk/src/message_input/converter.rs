use crate::message_input::SimpleMessageInput;
use crate::{CycBoxError, Message, MessageBuilder, PayloadType};

/// Trait for converting a raw JSON message input value into one or more [`Message`]s.
///
/// Implementations are registered in [`MessageInputRegistry`] so the engine
/// can convert protocol-specific JSON inputs without the SDK knowing about them.
pub trait MessageInputConverter: Send + Sync {
    /// The `input_type` discriminator value this converter handles (e.g. `"simple"`, `"mqtt"`).
    fn input_type(&self) -> &str;

    /// Convert `json` (the full message-input JSON object) into messages to send.
    fn convert(&self, json: &serde_json::Value) -> Result<Vec<Message>, CycBoxError>;
}

pub struct SimpleMessageInputConverter;

impl MessageInputConverter for SimpleMessageInputConverter {
    fn input_type(&self) -> &str {
        "simple"
    }

    fn convert(&self, json: &serde_json::Value) -> Result<Vec<Message>, CycBoxError> {
        let input: SimpleMessageInput = serde_json::from_value(json.clone())?;

        let input_json = serde_json::to_string_pretty(&input)?;
        log::debug!("Converting SimpleMessageInput: {input_json}");

        let payload = text_to_bytes(&input.raw_value, input.is_hex)?;
        let msg = MessageBuilder::tx(
            input.connection_id,
            PayloadType::Binary,
            payload.clone(),
            vec![],
        )
        .build();

        let message_json = serde_json::to_string_pretty(&msg)?;
        log::debug!("Converted Message: {message_json}");

        Ok(vec![msg])
    }
}

/// Parse a hex string like "AA BB CC" or "AABBCC" into bytes.
pub fn parse_hex_string(s: &str) -> Result<Vec<u8>, CycBoxError> {
    let s: String = s.split_whitespace().collect();
    if s.is_empty() {
        return Ok(Vec::new());
    }
    if !s.len().is_multiple_of(2) {
        return Err(CycBoxError::InvalidFormat(format!(
            "hex string must have an even number of characters: '{}'",
            s
        )));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| {
                CycBoxError::InvalidFormat(format!("invalid hex byte '{}': {}", &s[i..i + 2], e))
            })
        })
        .collect()
}

/// Convert a text string to bytes. If `is_hex` is true the string is parsed as
/// a hex byte string (e.g. `"AA BB CC"`), otherwise it is encoded as UTF-8.
pub fn text_to_bytes(text: &str, is_hex: bool) -> Result<Vec<u8>, CycBoxError> {
    if is_hex {
        parse_hex_string(text)
    } else {
        Ok(text.as_bytes().to_vec())
    }
}
