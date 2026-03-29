use crate::message::{SYSTEM_CONNECTION_ID, UNKNOW_CONNECTION_ID};
use crate::{MESSAGE_TYPE_RX, Message, PayloadType};

/// Builder for constructing Message instances with a fluent API
pub struct MessageBuilder {
    connection_id: u32,
    timestamp: u64,
    message_type: String,
    payload_type: PayloadType,
    frame: Vec<u8>,
    payload: Vec<u8>,
    contents: Vec<crate::Content>,
    values: Vec<crate::Value>,
    metadata: Vec<crate::Value>,
    highlighted: bool,
    hex_contents: Vec<crate::Content>,
    display_hex: bool,
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageBuilder {
    pub fn new() -> Self {
        Self {
            connection_id: SYSTEM_CONNECTION_ID,
            timestamp: 0,
            message_type: String::new(),
            payload_type: PayloadType::Binary,
            frame: Vec::new(),
            payload: Vec::new(),
            contents: Vec::new(),
            values: Vec::new(),
            metadata: Vec::new(),
            highlighted: false,
            hex_contents: Vec::new(),
            display_hex: false,
        }
    }

    pub fn event(event_id: impl Into<String>) -> Self {
        let mut builder = Self::new();
        builder.message_type = crate::MESSAGE_TYPE_EVENT.to_string();
        builder.frame = event_id.into().into_bytes();
        builder
    }

    pub fn tx(
        connection_id: u32,
        payload_type: PayloadType,
        payload: impl Into<Vec<u8>>,
        frame: impl Into<Vec<u8>>,
    ) -> Self {
        let mut builder = Self::new();
        builder.connection_id = connection_id;
        builder.message_type = crate::MESSAGE_TYPE_TX.to_string();
        builder.payload_type = payload_type;
        builder.payload = payload.into();
        builder.frame = frame.into();
        builder
    }

    pub fn rx(
        payload_type: PayloadType,
        payload: impl Into<Vec<u8>>,
        frame: impl Into<Vec<u8>>,
    ) -> Self {
        let mut builder = Self::new();
        builder.connection_id = UNKNOW_CONNECTION_ID;
        builder.message_type = MESSAGE_TYPE_RX.to_string();
        builder.payload_type = payload_type;
        builder.payload = payload.into();
        builder.frame = frame.into();
        builder
    }

    pub fn request(
        seq_id: u64,
        command: impl Into<String>,
        timestamp: u64,
        _timeout_ms: u32, // not implement yet
    ) -> Self {
        let mut builder = Self::new();
        builder.timestamp = timestamp;
        builder.message_type = crate::MESSAGE_TYPE_REQUEST.to_string();
        builder.payload = seq_id.to_le_bytes().to_vec(); // Encode seq_id as first 8 bytes of payload
        builder.frame = command.into().into_bytes();
        builder
    }

    /// Create a success response from a request message
    pub fn response_success(request: &Message) -> Self {
        let mut builder = Self::new();
        builder.message_type = crate::MESSAGE_TYPE_RESPONSE.to_string();
        builder.frame = request.frame.clone();
        builder.payload = request.payload.clone(); // Copy seq_id
        builder.connection_id = request.connection_id;
        builder
            .metadata
            .push(crate::Value::builder("success").boolean(true));
        builder
    }

    /// Create an error response from a request message with error message
    pub fn response_error(request: &Message, error: impl Into<String>) -> Self {
        let mut builder = Self::new();
        builder.message_type = crate::MESSAGE_TYPE_RESPONSE.to_string();
        builder.frame = request.frame.clone();
        builder.payload = request.payload.clone(); // Copy seq_id
        builder.connection_id = request.connection_id;
        builder
            .metadata
            .push(crate::Value::builder("success").boolean(false));
        builder
            .metadata
            .push(crate::Value::builder("error").string(error));
        builder
    }

    /// Set the connection ID (config index)
    pub fn connection_id(mut self, connection_id: u32) -> Self {
        self.connection_id = connection_id;
        self
    }

    /// Set the timestamp (in microseconds since UNIX_EPOCH)
    /// If not set, the current time will be used when building
    pub fn timestamp(mut self, timestamp: u64) -> Self {
        self.timestamp = timestamp;
        self
    }

    pub fn message_type(mut self, message_type: impl Into<String>) -> Self {
        self.message_type = message_type.into();
        self
    }

    /// Set the payload bytes
    pub fn payload(mut self, payload_type: PayloadType, payload: impl Into<Vec<u8>>) -> Self {
        self.payload_type = payload_type;
        self.payload = payload.into();
        self
    }

    /// Set the frame bytes
    pub fn frame(mut self, frame: impl Into<Vec<u8>>) -> Self {
        self.frame = frame.into();
        self
    }

    /// Set the contents (replaces existing contents)
    pub fn contents(mut self, contents: Vec<crate::Content>) -> Self {
        self.contents = contents;
        self
    }

    /// Add a single content item
    pub fn add_content(mut self, content: crate::Content) -> Self {
        self.contents.push(content);
        self
    }

    /// Set the values (replaces existing values)
    pub fn values(mut self, values: Vec<crate::Value>) -> Self {
        self.values = values;
        self
    }

    /// Add a single value
    pub fn add_value(mut self, value: crate::Value) -> Self {
        self.values.push(value);
        self
    }

    pub fn add_values(mut self, values: Vec<crate::Value>) -> Self {
        self.values.extend(values);
        self
    }

    /// Set the metadata values
    pub fn metadata(mut self, metadata: Vec<crate::Value>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Add a single metadata value
    pub fn add_metadata(mut self, metadata: crate::Value) -> Self {
        self.metadata.push(metadata);
        self
    }

    /// Set whether the message should be highlighted
    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
        self
    }

    /// Set the hex contents (replaces existing hex contents)
    pub fn hex_contents(mut self, hex_contents: Vec<crate::Content>) -> Self {
        self.hex_contents = hex_contents;
        self
    }

    /// Add a single hex content item
    pub fn add_hex_content(mut self, content: crate::Content) -> Self {
        self.hex_contents.push(content);
        self
    }

    /// Set whether the message should display hex dump view
    pub fn display_hex(mut self, display_hex: bool) -> Self {
        self.display_hex = display_hex;
        self
    }

    /// Set the sequence ID (for REQUEST/RESPONSE messages)
    /// The seq_id is encoded as the first 8 bytes of the payload
    pub fn seq_id(mut self, seq_id: u64) -> Self {
        self.payload = seq_id.to_le_bytes().to_vec();
        self
    }

    /// Build the Message instance
    pub fn build(self) -> Message {
        let timestamp = if self.timestamp == 0 {
            Message::current_timestamp()
        } else {
            self.timestamp
        };

        let frame = if self.frame.is_empty() {
            self.payload.clone()
        } else {
            self.frame
        };

        Message {
            connection_id: self.connection_id,
            timestamp,
            message_type: self.message_type,
            payload_type: self.payload_type,
            frame,
            payload: self.payload,
            contents: self.contents,
            values: self.values,
            metadata: self.metadata,
            highlighted: self.highlighted,
            hex_contents: self.hex_contents,
            display_hex: self.display_hex,
        }
    }
}
