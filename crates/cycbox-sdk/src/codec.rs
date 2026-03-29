use crate::{Configurable, CycBoxError, Manifestable, Message};
use async_trait::async_trait;
use bytes::BytesMut;

/// Unified codec trait combining encoding and decoding functionality
///
/// This trait combines both encoding and decoding functionality into a single interface,
/// along with lifecycle methods and command handling capability.
#[async_trait]
pub trait Codec: Configurable + Manifestable + Send + Sync {
    /// Decode a message from the buffer
    ///
    /// Returns Ok(Some(message)) when a complete message is decoded,
    /// Ok(None) when more data is needed, or Err on parsing errors.
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError>;

    /// Decode a message on timeout event
    fn decode_timeout(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        self.decode(src)
    }

    /// Decode a message on EOF (end of stream)
    ///
    /// Called when the connection is closed. Default behavior is to call decode_timeout.
    fn decode_eof(&mut self, src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        // Default: try decode_timeout on EOF
        self.decode_timeout(src)
    }

    /// Encode a message
    fn encode(&mut self, item: &mut Message) -> Result<(), CycBoxError>;

    /// Reset internal codec state (e.g., on reconnection)
    ///
    /// Called when the connection is re-established. Default implementation does nothing
    /// (suitable for stateless codecs).
    fn reset(&mut self) {
        // Default: no-op for stateless codecs
    }

    /// Handle a command message, return response message if the command is handled
    async fn handle_command(&mut self, _command: &Message) -> Option<Message> {
        None
    }
}
