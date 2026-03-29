#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod builder;
pub use builder::MessageBuilder;

mod content;
pub use content::{Color, Content, ContentType, Decoration};

pub mod lua_user_data;

pub mod lua_functions;
mod value;

pub use value::{Value, ValueBuilder, ValueType};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PayloadType {
    Binary,
    Text,
    WebsocketBinary,
    WebsocketText,
    Mqtt,
    ModbusRequest,
    ModbusResponse,
    HttpRequest,
    HttpResponse,
}

pub const SYSTEM_CONNECTION_ID: u32 = 9999; // Reserved connection ID for system events and messages
pub const UNKNOW_CONNECTION_ID: u32 = 9998; // Message created inside Codec, Transport or Transformer without a specific connection context

pub const MESSAGE_TYPE_RX: &str = "rx";
pub const MESSAGE_TYPE_TX: &str = "tx";
pub const MESSAGE_TYPE_LOG: &str = "log";
pub const MESSAGE_TYPE_EVENT: &str = "event";
pub const MESSAGE_TYPE_REQUEST: &str = "request";
pub const MESSAGE_TYPE_RESPONSE: &str = "response";

/// Command IDs for MESSAGE_TYPE_REQUEST and MESSAGE_TYPE_RESPONSE
pub const COMMAND_ID_SET_HIGHLIGHT: &str = "set_highlight";
pub const COMMAND_ID_CLEAR_HIGHLIGHT: &str = "clear_highlight";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub connection_id: u32, // Identifies the source/target connection (config index)
    pub timestamp: u64,     // timestamp in microseconds since epoch
    pub message_type: String,
    pub payload_type: PayloadType,

    // MESSAGE_TYPE_RX|MESSAGE_TYPE_TX: frame = [ frame header ] + [ payload ] + [ frame trailer ]
    // MESSAGE_TYPE_EVENT: event_name
    // MESSAGE_TYPE_REQUEST|MESSAGE_TYPE_RESPONSE: command_name
    pub frame: Vec<u8>,

    // MESSAGE_TYPE_RX|MESSAGE_TYPE_TX: message payload;
    // MESSAGE_TYPE_REQUEST|MESSAGE_TYPE_RESPONSE: request seq_id
    pub payload: Vec<u8>,

    // MESSAGE_TYPE_RX|MESSAGE_TYPE_TX: message text content to display;
    pub contents: Vec<Content>,

    // MESSAGE_TYPE_RX|MESSAGE_TYPE_TX: message values
    // MESSAGE_TYPE_EVENT: event key value pairs;
    // MESSAGE_TYPE_REQUEST: Request parameters
    // MESSAGE_TYPE_RESPONSE: response values
    pub values: Vec<Value>,

    // metadata for message
    // MESSAGE_TYPE_RX|MESSAGE_TYPE_TX: message metadata (e.g., MQTT topic, QoS)
    // MESSAGE_TYPE_REQUEST:
    // `timeout_ms`: Timeout override (UInt32, optional)
    // MESSAGE_TYPE_RESPONSE:
    // - `success`: Success flag (Boolean)
    // - `error`: Error message (String)
    pub metadata: Vec<Value>,

    pub highlighted: bool, // indicates if the message should be highlighted in UI

    pub hex_contents: Vec<Content>, // Hex dump formatted content
    pub display_hex: bool,          // indicates if the message should display hex dump view in UI
}

impl Message {
    pub fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }

    pub fn get_command(&self) -> String {
        String::from_utf8_lossy(&self.frame).to_string()
    }

    pub fn set_command(&mut self, command: &str) {
        self.frame = command.as_bytes().to_vec();
    }

    pub fn get_seq_id(&self) -> u64 {
        if self.payload.len() >= 8 {
            let bytes: [u8; 8] = self.payload[0..8].try_into().unwrap();
            u64::from_le_bytes(bytes)
        } else {
            0
        }
    }

    pub fn set_seq_id(&mut self, seq_id: u64) {
        let bytes = seq_id.to_le_bytes();
        self.payload = bytes.to_vec();
    }

    pub fn timeout(&self) -> Option<Duration> {
        for val in &self.metadata {
            if val.id == "timeout_ms"
                && let Some(timeout_ms) = val.as_u32()
            {
                return Some(Duration::from_millis(timeout_ms as u64));
            }
        }
        None
    }

    pub fn param(&self, name: &str) -> Option<Value> {
        self.get_value(name)
    }

    /// Get a value from `values` by id
    pub fn get_value(&self, id: &str) -> Option<Value> {
        self.values.iter().find(|v| v.id == id).cloned()
    }

    /// Get metadata value by name
    pub fn metadata_value(&self, name: &str) -> Option<Value> {
        for val in &self.metadata {
            if val.id == name {
                return Some(val.clone());
            }
        }
        None
    }

    /// Check if this is a successful response (for MESSAGE_TYPE_RESPONSE)
    pub fn is_success(&self) -> bool {
        self.metadata_value("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Get error message from response (for MESSAGE_TYPE_RESPONSE)
    pub fn error_message(&self) -> Option<String> {
        self.metadata_value("error").and_then(|v| v.as_string())
    }

    /// Refresh the timestamp to the current time
    pub fn refresh_timestamp(&mut self) {
        self.timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
    }
}
