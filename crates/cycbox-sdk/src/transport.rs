use crate::{
    COMMAND_ID_SEND_RAW, Codec, CycBoxError, FormGroup, LogLevel, MESSAGE_TYPE_LOG,
    MESSAGE_TYPE_RAW_RX, Manifestable, Message, MessageBuilder, PayloadType, Value,
};
use async_trait::async_trait;
use bytes::BytesMut;
use log::{debug, info, warn};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

/// Id of the value that carries a sink `LOG` message's level ([`LogLevel::as_str`]).
pub const LOG_LEVEL_VALUE_ID: &str = "level";

/// Per-connection sink a transport pushes out-of-band messages on: log lines, events,
/// and raw RX bytes. None of these are protocol data — they never pass through the
/// codec, the transformer, or the Lua `on_receive` hook.
///
/// The engine creates one sink per connection, before the first connect, and keeps it
/// across reconnects, so a background task a transport spawns inside
/// [`Transport::connect`] can report from its first moment.
///
/// Sending never blocks and never fails the caller: a full channel drops the message and
/// counts it ([`Self::take_dropped`]), so every method is safe inside `poll_read` /
/// `poll_write` as well as in async code. A sink whose receiver is gone (see
/// [`Self::detached`]) drops silently.
#[derive(Clone)]
pub struct TransportSink {
    tx: mpsc::Sender<Message>,
    dropped: Arc<AtomicU32>,
    raw_rx: bool,
}

impl TransportSink {
    /// A sink and the receiver its messages arrive on. `raw_rx` enables [`Self::raw_rx`];
    /// with it off, raw bytes are not even copied.
    pub fn new(capacity: usize, raw_rx: bool) -> (Self, mpsc::Receiver<Message>) {
        let (tx, rx) = mpsc::channel(capacity);
        let sink = Self {
            tx,
            dropped: Arc::new(AtomicU32::new(0)),
            raw_rx,
        };
        (sink, rx)
    }

    /// A sink that goes nowhere, for callers with no engine behind them.
    pub fn detached() -> Self {
        Self::new(1, false).0
    }

    /// Push a message as is. The receiver stamps the connection id.
    pub fn send(&self, message: Message) {
        if let Err(TrySendError::Full(_)) = self.tx.try_send(message) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// A `MESSAGE_TYPE_LOG` message: `text` as a text payload, `level` as a value.
    pub fn log(&self, level: LogLevel, text: impl Into<String>) {
        let text: String = text.into();
        let message = MessageBuilder::new()
            .message_type(MESSAGE_TYPE_LOG)
            .payload(PayloadType::Text, text.into_bytes())
            .add_value(Value::new_string(LOG_LEVEL_VALUE_ID, level.as_str()))
            .build();
        self.send(message);
    }

    pub fn info(&self, text: impl Into<String>) {
        self.log(LogLevel::Info, text);
    }

    pub fn warn(&self, text: impl Into<String>) {
        self.log(LogLevel::Warning, text);
    }

    pub fn error(&self, text: impl Into<String>) {
        self.log(LogLevel::Error, text);
    }

    /// A `MESSAGE_TYPE_EVENT` message named `name` (Lua `on_event`, then the UI).
    pub fn event(&self, name: &str, values: Vec<Value>) {
        self.send(MessageBuilder::event(name).add_values(values).build());
    }

    /// A `MESSAGE_TYPE_RAW_RX` message, if this sink has raw RX enabled.
    pub fn raw_rx(&self, timestamp: u64, bytes: &[u8]) {
        if !self.raw_rx {
            return;
        }
        let message = MessageBuilder::new()
            .message_type(MESSAGE_TYPE_RAW_RX)
            .timestamp(timestamp)
            .frame(bytes.to_vec())
            .build();
        self.send(message);
    }

    /// Whether [`Self::raw_rx`] does anything.
    pub fn raw_rx_enabled(&self) -> bool {
        self.raw_rx
    }

    /// Number of messages dropped on a full channel since the last call.
    pub fn take_dropped(&self) -> u32 {
        self.dropped.swap(0, Ordering::Relaxed)
    }
}

/// What the engine hands a transport at connect time.
#[derive(Clone)]
pub struct TransportContext {
    /// Where the transport reports logs, events and raw RX. Lives as long as the
    /// connection, across reconnects.
    pub sink: TransportSink,
    /// Receive timeout `CodecTransport` uses to drive `Codec::decode_timeout`.
    pub timeout: Duration,
}

impl TransportContext {
    pub fn new(sink: TransportSink, timeout: Duration) -> Self {
        Self { sink, timeout }
    }

    /// A context with a [`TransportSink::detached`] sink.
    pub fn detached(timeout: Duration) -> Self {
        Self::new(TransportSink::detached(), timeout)
    }
}

/// A byte-stream transport's report that the failure it just produced will repeat if it
/// is retried straight away.
///
/// A transport fails a read or a write with `io::Error::other(RetryLater::new(..))` (or
/// `RetryLater::new(..).into()`); `CodecTransport` finds it inside the `io::Error` and
/// returns [`CycBoxError::ConnectionBackoff`] instead of [`CycBoxError::Connection`]. The
/// connection task still reconnects; it just waits much longer first.
#[derive(Debug, Clone)]
pub struct RetryLater {
    pub reason: String,
}

impl RetryLater {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    /// The `RetryLater` inside `error`, if there is one.
    pub fn find(error: &std::io::Error) -> Option<&RetryLater> {
        error.get_ref()?.downcast_ref::<RetryLater>()
    }
}

impl std::fmt::Display for RetryLater {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for RetryLater {}

impl From<RetryLater> for std::io::Error {
    fn from(hint: RetryLater) -> Self {
        std::io::Error::other(hint)
    }
}

#[async_trait]
pub trait Transport: Manifestable + Send + Sync {
    /// Connect to the transport with the given configurations and codec.
    ///
    /// `ctx` carries the connection's [`TransportSink`] and receive timeout. Transports
    /// keep a clone of the sink for as long as they have something to report.
    ///
    /// Returns a boxed MessageTransport on success, or an error on failure.
    async fn connect(
        &self,
        configs: &[FormGroup],
        codec: Box<dyn Codec>,
        ctx: TransportContext,
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

    /// Cooperatively tear the transport down. Implementations should release
    /// any OS handles, terminate background bridge tasks, and `await` until
    /// cleanup is complete. After `close` returns, further `read`/`write`
    /// calls may fail; the value should only be dropped afterwards.
    ///
    /// Default is a no-op for transports whose `Drop` impl already does
    /// everything synchronously (e.g. owning a single OS handle).
    async fn close(&mut self) {}
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

    /// Cooperatively tear the transport down, awaiting any in-flight cleanup
    /// (background bridge tasks, remote disconnect handshakes, etc.). Called
    /// by the engine before dropping the transport on cooperative shutdown
    /// — abort-based drop is still the fallback if `close` takes too long.
    ///
    /// Default is a no-op; transports that own only synchronous OS handles
    /// (TCP, serial) don't need to override it.
    async fn close(&mut self) {}
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
    sink: TransportSink,
}

impl CodecTransport {
    /// Wraps `transport` and `codec`, taking the receive timeout and the raw-RX sink from
    /// `ctx`.
    pub fn new(
        transport: Box<dyn TransportIO>,
        codec: Box<dyn Codec>,
        ctx: &TransportContext,
    ) -> Self {
        Self {
            transport,
            codec,
            buffer: BytesMut::new(),
            read_buf: vec![0u8; 102400],
            timeout_duration: ctx.timeout,
            pending_messages: VecDeque::new(),
            sink: ctx.sink.clone(),
        }
    }

    /// Wraps a transport failure as the error the connection task should act on: a
    /// [`RetryLater`] inside the `io::Error` asks for the slow backoff.
    fn connection_error(error: std::io::Error) -> CycBoxError {
        match RetryLater::find(&error) {
            Some(hint) => CycBoxError::ConnectionBackoff(hint.reason.clone()),
            None => CycBoxError::Connection(error.to_string()),
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

                    // Return any decoded message first.
                    if let Some(msg) = self.pending_messages.pop_front() {
                        return Ok(Some(msg));
                    }
                    return Ok(None);
                }
                Ok(Ok(n)) => {
                    // Surface raw bytes before any decode work runs — this guarantees
                    // the UI sees them even if the codec mishandles the buffer.
                    if self.sink.raw_rx_enabled() {
                        self.sink
                            .raw_rx(Message::current_timestamp(), &self.read_buf[..n]);
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
                Ok(Err(e)) => return Err(Self::connection_error(e)),
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
            if let Err(e) = self.transport.write_all(&message.frame).await {
                // `NotConnected` is the agreed signal from server-style
                // transports (e.g. p2p server with no active client) that the
                // bytes should be discarded but the transport stays alive. All
                // other IO errors still mean "real connection failure" and
                // trigger the connection task's reconnect path.
                return Err(match e.kind() {
                    std::io::ErrorKind::NotConnected => CycBoxError::Discarded(e.to_string()),
                    _ => Self::connection_error(e),
                });
            }
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

    async fn close(&mut self) {
        self.transport.close().await;
    }
}
