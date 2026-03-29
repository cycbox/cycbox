use crate::{Codec, CycBoxError, FormGroup, Manifestable, Message};
use async_trait::async_trait;
use bytes::BytesMut;
use log::{debug, warn};
use std::collections::VecDeque;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[async_trait]
pub trait Transport: Manifestable + Send + Sync {
    /// Connect to the transport with the given configurations and codec.
    ///
    /// Returns a boxed MessageTransport on success, or an error on failure.
    async fn connect(
        &self,
        configs: &[FormGroup],
        codec: Box<dyn Codec>,
        timeout: Duration,
    ) -> Result<Box<dyn MessageTransport>, CycBoxError>;
}

/// Byte-stream transport trait combining AsyncRead and AsyncWrite with command handling.
/// Used internally by stream-based transports before wrapping with CodecTransport.
#[async_trait]
pub trait TransportIO: AsyncRead + AsyncWrite + Send + Unpin {
    /// Handle a command request and optionally return a response
    async fn handle_command(&mut self, _command: &Message) -> Option<Message> {
        None
    }
}

/// Message-based transport trait for all transports.
/// Stream-based transports are wrapped with CodecTransport to implement this trait.
#[async_trait]
pub trait MessageTransport: Send + Unpin {
    /// Receive a complete message from the transport
    ///
    /// Returns:
    /// - Ok(Some(message)) when a message is successfully received
    /// - Ok(None) when the connection is closed gracefully
    /// - Err on connection errors
    async fn recv(&mut self) -> Result<Option<Message>, CycBoxError>;

    /// Send a complete message to the transport
    ///
    /// Returns:
    /// - Ok(()) when the message is successfully sent
    /// - Err on connection errors
    async fn send(&mut self, message: &mut Message) -> Result<(), CycBoxError>;

    /// Handle a command and optionally return a response
    async fn handle_command(&mut self, _command: &Message) -> Option<Message> {
        None
    }
}

const MAX_BUFFER_SIZE: usize = 1024 * 1024 * 1024;

/// Wraps a byte-stream `TransportIO` + `Codec` into a `MessageTransport`.
///
/// This adapter handles:
/// - Reading bytes from transport with timeout
/// - Decoding bytes into messages via codec
/// - Encoding outgoing messages via codec
/// - Buffering multiple decoded messages from a single read
pub struct CodecTransport {
    transport: Box<dyn TransportIO>,
    codec: Box<dyn Codec>,
    buffer: BytesMut,
    read_buf: Vec<u8>,
    timeout_duration: Duration,
    pending_messages: VecDeque<Message>,
}

impl CodecTransport {
    pub fn new(
        transport: Box<dyn TransportIO>,
        codec: Box<dyn Codec>,
        timeout_duration: Duration,
    ) -> Self {
        Self {
            transport,
            codec,
            buffer: BytesMut::new(),
            read_buf: vec![0u8; 102400],
            timeout_duration,
            pending_messages: VecDeque::new(),
        }
    }
}

#[async_trait]
impl MessageTransport for CodecTransport {
    async fn recv(&mut self) -> Result<Option<Message>, CycBoxError> {
        loop {
            // Return buffered message if available
            if let Some(msg) = self.pending_messages.pop_front() {
                return Ok(Some(msg));
            }

            // Read from transport with timeout
            match tokio::time::timeout(
                self.timeout_duration,
                self.transport.read(&mut self.read_buf),
            )
            .await
            {
                Ok(Ok(0)) => {
                    debug!("CodecTransport: EOF received from transport");
                    // EOF - try to decode remaining data
                    if !self.buffer.is_empty() {
                        debug!(
                            "CodecTransport: EOF received, trying decode_eof on buffer with {} bytes",
                            self.buffer.len()
                        );
                        if let Ok(Some(msg)) = Codec::decode_eof(&mut *self.codec, &mut self.buffer)
                        {
                            self.pending_messages.push_back(msg);
                        }
                    }
                    self.codec.reset();
                    self.buffer.clear();

                    // Return any decoded message, or None for EOF
                    return Ok(self.pending_messages.pop_front());
                }
                Ok(Ok(n)) => {
                    // Data received
                    if self.buffer.len() + n > MAX_BUFFER_SIZE {
                        warn!(
                            "CodecTransport: buffer overflow, clearing buffer (size: {} bytes)",
                            self.buffer.len()
                        );
                        self.buffer.clear();
                    }
                    self.buffer.extend_from_slice(&self.read_buf[..n]);

                    // Decode all available messages
                    loop {
                        match self.codec.decode(&mut self.buffer) {
                            Ok(Some(msg)) => {
                                self.pending_messages.push_back(msg);
                            }
                            Ok(None) => break,
                            Err(e) => {
                                warn!("CodecTransport: decode error: {e}");
                                self.buffer.clear();
                                break;
                            }
                        }
                    }

                    // If we got a complete message, return it; otherwise loop
                    // to read more data (instead of recursive call which overflows stack)
                    if let Some(msg) = self.pending_messages.pop_front() {
                        return Ok(Some(msg));
                    }
                    // continue loop to read more
                }
                Ok(Err(e)) => return Err(CycBoxError::Connection(e.to_string())),
                Err(_) => {
                    // Timeout - try decode_timeout if buffer is not empty
                    if !self.buffer.is_empty()
                        && let Ok(Some(msg)) = self.codec.decode_timeout(&mut self.buffer)
                    {
                        return Ok(Some(msg));
                    }
                    // No message from timeout - loop to try again
                    // (instead of recursive call which overflows stack)
                }
            }
        }
    }

    async fn send(&mut self, message: &mut Message) -> Result<(), CycBoxError> {
        self.codec.encode(message)?;
        if message.frame.is_empty() {
            message.frame = message.payload.clone();
        }
        self.transport
            .write_all(&message.frame)
            .await
            .map_err(|e| CycBoxError::Connection(e.to_string()))?;
        Ok(())
    }
}
