use crate::message_input::converter::SimpleMessageInputConverter;
use crate::message_input::{BatchMessageInput, MessageInputConverter};
use crate::{CycBoxError, Message, Value};
use std::collections::HashMap;

/// Registry of [`MessageInputConverter`]s.
///
/// Built-in converters for `simple` and `frame` are registered automatically.
/// Protocol-specific converters (MQTT, Modbus, UDP) should be registered by
/// the engine during startup via [`RunMode::message_input_registry`].
pub struct MessageInputRegistry {
    converters: HashMap<String, Box<dyn MessageInputConverter>>,
}

impl Default for MessageInputRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageInputRegistry {
    /// Create a new registry with the built-in `simple` and `frame` converters.
    pub fn new() -> Self {
        let mut registry = Self {
            converters: HashMap::new(),
        };
        registry.register(Box::new(SimpleMessageInputConverter));
        registry
    }

    /// Register a converter, replacing any existing one for the same type key.
    pub fn register(&mut self, converter: Box<dyn MessageInputConverter>) {
        self.converters
            .insert(converter.input_type().to_string(), converter);
    }

    /// Convert a raw JSON message input into messages.
    ///
    /// Dispatches on the `"type"` field. The `"batch"` type is handled
    /// inline by iterating items and recursively converting each one.
    pub fn convert(&self, json: &serde_json::Value) -> Result<Vec<Message>, CycBoxError> {
        let input_type = json
            .get("input_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CycBoxError::MissingField("missing 'input_type' field in message input".to_string())
            })?;

        if input_type == "batch" {
            let batch: BatchMessageInput = serde_json::from_value(json.clone())?;
            let mut messages = Vec::new();
            let mut cumulative_delay_us: u64 = 0;
            for item in batch.items {
                let delay_us = (item.delay_ms * 1000.0) as u64;
                cumulative_delay_us += delay_us;
                let mut converted = self.convert(&item.message_input)?;
                for msg in &mut converted {
                    msg.metadata.push(Value::new_u64("delay_us", delay_us));
                    msg.timestamp += cumulative_delay_us;
                }
                messages.extend(converted);
            }
            return Ok(messages);
        }

        let converter = self.converters.get(input_type).ok_or_else(|| {
            CycBoxError::Unsupported(format!(
                "no converter registered for message input type '{}'",
                input_type
            ))
        })?;
        converter.convert(json)
    }
}
