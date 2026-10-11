//! `TransportSink` drop accounting and raw-RX gating, and `RetryLater` carried inside an
//! `io::Error` through `CodecTransport` as `ConnectionBackoff`.

use async_trait::async_trait;
use bytes::BytesMut;
use cycbox_sdk::prelude::*;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[test]
fn full_sink_drops_and_counts() {
    let (sink, mut rx) = TransportSink::new(2, false);
    for i in 0..5 {
        sink.info(format!("line {i}"));
    }
    assert_eq!(sink.take_dropped(), 3);
    assert_eq!(sink.take_dropped(), 0, "the count resets once taken");

    let first = rx.try_recv().unwrap();
    assert_eq!(first.message_type, MESSAGE_TYPE_LOG);
    assert_eq!(first.payload, b"line 0");
    assert_eq!(
        first
            .get_value(cycbox_sdk::transport::LOG_LEVEL_VALUE_ID)
            .and_then(|v| v.as_string())
            .as_deref(),
        Some("info")
    );
}

#[test]
fn detached_sink_drops_silently() {
    let sink = TransportSink::detached();
    for _ in 0..5 {
        sink.event("something", vec![]);
    }
    assert_eq!(sink.take_dropped(), 0, "no receiver is not overflow");
}

#[test]
fn raw_rx_only_when_enabled() {
    let (off, mut off_rx) = TransportSink::new(4, false);
    off.raw_rx(1, b"abc");
    assert!(off_rx.try_recv().is_err());

    let (on, mut on_rx) = TransportSink::new(4, true);
    on.raw_rx(42, b"abc");
    let raw = on_rx.try_recv().unwrap();
    assert_eq!(raw.message_type, MESSAGE_TYPE_RAW_RX);
    assert_eq!(raw.timestamp, 42);
    assert_eq!(raw.frame, b"abc");
}

#[test]
fn event_carries_name_and_values() {
    let (sink, mut rx) = TransportSink::new(4, false);
    sink.event(
        "client_connected",
        vec![Value::new_string("peer", "1.2.3.4:5")],
    );
    let event = rx.try_recv().unwrap();
    assert_eq!(event.message_type, MESSAGE_TYPE_EVENT);
    assert_eq!(event.get_command(), "client_connected");
    assert_eq!(
        event
            .get_value("peer")
            .and_then(|v| v.as_string())
            .as_deref(),
        Some("1.2.3.4:5")
    );
}

/// A byte stream whose reads and writes fail with whatever `fail` builds.
struct Failing {
    fail: fn() -> io::Error,
}

impl AsyncRead for Failing {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Err((self.fail)()))
    }
}

impl AsyncWrite for Failing {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Err((self.fail)()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[async_trait]
impl TransportIO for Failing {}

struct Passthrough;

#[async_trait]
impl Manifestable for Passthrough {
    async fn manifest(&self, _locale: &str) -> Manifest {
        Manifest::default()
    }
}

#[async_trait]
impl Configurable for Passthrough {
    async fn config(&mut self, _configs: &[FormGroup]) -> Result<(), CycBoxError> {
        Ok(())
    }
}

impl Codec for Passthrough {
    fn decode(&mut self, _src: &mut BytesMut) -> Result<Option<Message>, CycBoxError> {
        Ok(None)
    }

    fn encode(&mut self, _item: &mut Message) -> Result<(), CycBoxError> {
        Ok(())
    }
}

fn codec_transport(fail: fn() -> io::Error) -> CodecTransport {
    CodecTransport::new(
        Box::new(Failing { fail }),
        Box::new(Passthrough),
        &TransportContext::detached(Duration::from_millis(25)),
    )
}

fn kicked() -> io::Error {
    RetryLater::new("taken over").into()
}

fn reset() -> io::Error {
    io::Error::new(io::ErrorKind::ConnectionReset, "reset")
}

#[tokio::test]
async fn retry_later_read_error_backs_off() {
    let err = codec_transport(kicked).recv().await.unwrap_err();
    assert!(
        matches!(&err, CycBoxError::ConnectionBackoff(reason) if reason == "taken over"),
        "{err:?}"
    );
}

#[tokio::test]
async fn retry_later_write_error_backs_off() {
    let mut message =
        MessageBuilder::tx(0, PayloadType::Binary, b"x".to_vec(), b"x".to_vec()).build();
    let err = codec_transport(kicked)
        .send(&mut message)
        .await
        .unwrap_err();
    assert!(err.wants_slow_retry(), "{err:?}");
}

#[tokio::test]
async fn plain_io_error_reconnects_quickly() {
    let err = codec_transport(reset).recv().await.unwrap_err();
    assert!(matches!(err, CycBoxError::Connection(_)), "{err:?}");
}
