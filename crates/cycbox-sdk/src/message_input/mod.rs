pub mod converter;
mod registry;

pub use converter::MessageInputConverter;
pub use converter::{parse_hex_string, text_to_bytes};
pub use registry::MessageInputRegistry;
use serde::{Deserialize, Serialize};

/// A message input configuration, discriminated by the `input_type` JSON field.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MessageInput(serde_json::Value);

impl MessageInput {
    pub fn input_type(&self) -> &str {
        self.0
            .get("input_type")
            .and_then(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn id(&self) -> Option<&str> {
        self.0.get("id").and_then(|s| s.as_str())
    }

    pub fn name(&self) -> Option<&str> {
        self.0.get("name").and_then(|s| s.as_str())
    }

    pub fn connection_id(&self) -> Option<u32> {
        self.0
            .get("connection_id")
            .and_then(|v| v.as_u64().map(|i| i as u32))
    }

    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }
}

/// A group of message input configs, used for organizing the UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageInputGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub inputs: Vec<MessageInput>,
}

/// A single item inside a batch, paired with a pre-send delay.
///
/// `delay_ms` is relative to the previous item's send time (not batch start).
/// Fractional milliseconds are supported for sub-millisecond precision.
///
/// `message_input` is stored as raw JSON so the engine registry can convert
/// protocol-specific types (MQTT, Modbus, etc.) without the SDK knowing about them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchMessageItem {
    pub message_input: serde_json::Value,
    pub delay_ms: f64,
}

/// Batch message: an ordered list of items sent sequentially with per-item delays.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchMessageInput {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub items: Vec<BatchMessageItem>,
    #[serde(default)]
    pub repeat: bool,
}

/// Simple text or hex message input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimpleMessageInput {
    pub id: String,
    pub name: String,
    pub connection_id: u32,
    pub raw_value: String,
    pub is_hex: bool,
}
