use crate::formatter::{format_hexdump, format_terminal};
use cycbox_sdk::prelude::*;
use log::debug;

/// Unified connection wrapping a `MessageTransport` with transformer, encoding,
/// and highlight state. Handles both codec-based (byte-stream) and native
/// message transports through the common `MessageTransport` trait.
pub(crate) struct Connection {
    connection_id: u32,
    transport: Box<dyn MessageTransport>,
    transformer: Option<Box<dyn Transformer>>,
    encoding: &'static encoding_rs::Encoding,
    highlight_bytes: Option<Vec<u8>>,
}

impl Connection {
    pub fn new(
        connection_id: u32,
        transport: Box<dyn MessageTransport>,
        transformer: Option<Box<dyn Transformer>>,
        encoding: &'static encoding_rs::Encoding,
    ) -> Self {
        Self {
            connection_id,
            transport,
            transformer,
            encoding,
            highlight_bytes: None,
        }
    }

    /// Receive a message from the transport, apply transformer and formatting,
    /// and inject the connection_id.
    ///
    /// Returns `Ok(None)` on EOF (caller should reconnect).
    pub async fn recv(&mut self) -> Result<Option<Message>, CycBoxError> {
        let msg = self.transport.recv().await?;
        let Some(mut message) = msg else {
            return Ok(None);
        };

        // Apply data transformer (if present and values is empty)
        if let Some(ref transformer) = self.transformer
            && message.values.is_empty()
            && let Err(e) = transformer.on_receive(&mut message)
        {
            debug!(
                "Connection {}: transformer on_receive error: {e}",
                self.connection_id
            );
        }

        // Apply formatting (if contents is empty)
        if message.contents.is_empty() {
            format_terminal(&mut message, self.encoding, self.highlight_bytes.as_deref());
        }

        // Format hex contents
        format_hexdump(&mut message, self.highlight_bytes.clone());

        // Inject connection_id
        message.connection_id = self.connection_id;

        Ok(Some(message))
    }

    /// Send a message through the transport, applying transformer on_send first.
    /// Returns the prepared TX confirmation message for the Lua on_send_confirm hook.
    pub async fn send(&mut self, mut message: Message) -> Result<Message, CycBoxError> {
        // Apply data transformer (if present)
        if let Some(ref transformer) = self.transformer
            && let Err(e) = transformer.on_send(&mut message)
        {
            debug!(
                "Connection {}: transformer on_send error: {e}",
                self.connection_id
            );
        }

        // For native message transports, ensure frame has content
        if message.frame.is_empty() {
            message.frame = message.payload.clone();
        }

        // Send through transport (CodecTransport handles encode internally)
        self.transport.send(&mut message).await?;

        // Apply formatting for TX display
        if message.contents.is_empty() {
            format_terminal(&mut message, self.encoding, self.highlight_bytes.as_deref());
        }

        format_hexdump(&mut message, self.highlight_bytes.clone());

        // Build TX confirmation message
        let mut tx_message = MessageBuilder::new()
            .message_type(cycbox_sdk::MESSAGE_TYPE_TX)
            .frame(message.frame.clone())
            .payload(message.payload_type.clone(), message.payload.clone())
            .metadata(message.metadata.clone())
            .contents(message.contents.clone())
            .hex_contents(message.hex_contents.clone())
            .display_hex(message.display_hex)
            .highlighted(message.highlighted)
            .build();

        tx_message.connection_id = self.connection_id;

        Ok(tx_message)
    }

    pub async fn handle_command(&mut self, command: &Message) -> Option<Message> {
        match command.get_command().as_str() {
            COMMAND_ID_SET_HIGHLIGHT => {
                // Extract highlight bytes from values (UInt8Array with id "bytes")
                let bytes = command.param("bytes").and_then(|v| v.as_u8_array());
                if let Some(bytes) = bytes {
                    self.highlight_bytes = Some(bytes);
                } else {
                    return Some(
                        MessageBuilder::response_error(command, "missing 'bytes' parameter")
                            .build(),
                    );
                }
                Some(MessageBuilder::response_success(command).build())
            }
            COMMAND_ID_CLEAR_HIGHLIGHT => {
                self.highlight_bytes = None;
                Some(MessageBuilder::response_success(command).build())
            }
            _ => self.transport.handle_command(command).await,
        }
    }
}
