use crate::{
    COMMAND_ID_SEND_RAW, Codec, CycBoxError, FormGroup, Manifestable, Message, MessageBuilder,
};
use async_trait::async_trait;
use bytes::BytesMut;
use log::{debug, info, warn};
use std::collections::VecDeque;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

/// A raw-byte observation captured from the underlying byte-stream transport.
#[derive(Debug, Clone)]
pub struct RawBytes {
    /// Microseconds since UNIX_EPOCH at the moment the bytes were observed.
    pub timestamp: u64,
    pub bytes: Vec<u8>,
}

/// Bounded sink that a stream-based transport notifies on every read.
#[derive(Clone)]
pub struct RawByteObserver {
    pub rx: mpsc::Sender<RawBytes>,
}

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

    /// Returns `true` (and clears its internal flag) when the underlying
    /// transport has just transitioned to a new logical session — for example,
    /// a server-style transport that switched from one accepted client to the
    /// next without surfacing EOF to the upper layer.
    ///
    /// When this returns `true`, `CodecTransport` flushes any in-flight decode
    /// state (calls `Codec::decode_eof` to drain a possible final frame, then
    /// `Codec::reset` and clears the decode buffer) before processing any
    /// further bytes. This guarantees stateful codecs (Modbus framing, AT
    /// parsing, …) do not carry one session's parser state into the next.
    ///
    /// Default returns `false` for transports without session boundaries
    fn take_session_boundary(&mut self) -> bool {
        false
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

    /// Install observers that receive raw bytes flowing through the transport
    /// (pre-decode for RX, post-encode for TX). Default no-op; only stream-based
    /// transports backed by `CodecTransport` honour this. Pass `None` to detach.
    fn set_raw_observer(&mut self, _observer: Option<RawByteObserver>) {}
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
    raw_observer: Option<RawByteObserver>,
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
            raw_observer: None,
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
                    // Surface raw bytes before any decode work runs — this guarantees
                    // the UI sees them even if the codec mishandles the buffer.
                    if let Some(ref observer) = self.raw_observer {
                        let raw = RawBytes {
                            timestamp: Message::current_timestamp(),
                            bytes: self.read_buf[..n].to_vec(),
                        };
                        let _ = observer.rx.try_send(raw);
                    }

                    // If the underlying transport rotated to a new logical
                    // session, drain any final frame from the previous
                    // session and reset codec state before consuming new bytes.
                    if self.transport.take_session_boundary() {
                        debug!(
                            "CodecTransport: session boundary observed (prev buffer: {} bytes)",
                            self.buffer.len()
                        );
                        if !self.buffer.is_empty()
                            && let Ok(Some(msg)) =
                                Codec::decode_eof(&mut *self.codec, &mut self.buffer)
                        {
                            self.pending_messages.push_back(msg);
                        }
                        self.codec.reset();
                        self.buffer.clear();
                    }

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
                    // Honour a session boundary observed during the idle
                    // window: drain final frame, then reset codec/buffer.
                    if self.transport.take_session_boundary() {
                        info!(
                            "CodecTransport: session boundary on timeout (prev buffer: {} bytes)",
                            self.buffer.len()
                        );
                        if !self.buffer.is_empty()
                            && let Ok(Some(msg)) =
                                Codec::decode_eof(&mut *self.codec, &mut self.buffer)
                        {
                            self.codec.reset();
                            self.buffer.clear();
                            return Ok(Some(msg));
                        }
                        self.codec.reset();
                        self.buffer.clear();
                    }
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
        if !message.frame.is_empty() {
            self.transport.write_all(&message.frame).await.map_err(|e| {
                // `NotConnected` is the agreed signal from server-style
                // transports (e.g. p2p server with no active client) that the
                // bytes should be discarded but the transport stays alive. All
                // other IO errors still mean "real connection failure" and
                // trigger the connection task's reconnect path.
                match e.kind() {
                    std::io::ErrorKind::NotConnected => CycBoxError::Discarded(e.to_string()),
                    _ => CycBoxError::Connection(e.to_string()),
                }
            })?;
        }
        Ok(())
    }

    async fn handle_command(&mut self, command: &Message) -> Option<Message> {
        if command.get_command() == COMMAND_ID_SEND_RAW {
            let bytes = command.param("bytes").and_then(|v| v.as_u8_array());
            if bytes.is_none() {
                return Some(
                    MessageBuilder::response_error(command, "missing 'bytes' parameter").build(),
                );
            }
            let bytes = bytes.unwrap();
            if !bytes.is_empty()
                && let Err(e) = self.transport.write_all(&bytes).await
            {
                return Some(MessageBuilder::response_error(command, e.to_string()).build());
            }
            return Some(MessageBuilder::response_success(command).build());
        }
        if let Some(response) = self.codec.handle_command(command).await {
            return Some(response);
        }
        self.transport.handle_command(command).await
    }

    fn set_raw_observer(&mut self, observer: Option<RawByteObserver>) {
        self.raw_observer = observer;
    }
}
