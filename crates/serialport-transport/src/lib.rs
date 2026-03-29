mod l10n;
mod manifestable;
mod utils;

use async_trait::async_trait;
use cycbox_sdk::prelude::*;
use cycbox_sdk::transport::{MessageTransport, Transport, TransportIO};
use cycbox_serialport::{
    ClearBuffer, DataBits, FlowControl, Parity, SerialPortBuilderExt, SerialStream, StopBits,
};
use log::debug;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use crate::manifestable::serial_manifest;
pub use utils::{SerialPortInfo, get_available_serial_ports};

pub const SERIAL_PORT_TRANSPORT_ID: &str = "serial_port_transport";

/// Serial transport factory that handles configuration and connection creation.
#[derive(Default)]
pub struct SerialTransport;

#[async_trait]
impl Manifestable for SerialTransport {
    async fn manifest(&self, locale: &str) -> Manifest {
        serial_manifest(locale)
    }
}

#[async_trait]
impl Transport for SerialTransport {
    async fn connect(
        &self,
        configs: &[FormGroup],
        codec: Box<dyn Codec>,
        timeout: Duration,
    ) -> Result<Box<dyn MessageTransport>, CycBoxError> {
        // Extract configuration values
        let port_path = FormUtils::get_text_value(
            configs,
            SERIAL_PORT_TRANSPORT_ID,
            &format!("{}_port", SERIAL_PORT_TRANSPORT_ID),
        )
        .ok_or_else(|| CycBoxError::MissingField("Missing serial port path".to_string()))?;
        let baud_rate = FormUtils::get_integer_value(
            configs,
            SERIAL_PORT_TRANSPORT_ID,
            &format!("{}_baud_rate", SERIAL_PORT_TRANSPORT_ID),
        )
        .unwrap_or(115200) as u32;
        let data_bits = FormUtils::get_integer_value(
            configs,
            SERIAL_PORT_TRANSPORT_ID,
            &format!("{}_data_bits", SERIAL_PORT_TRANSPORT_ID),
        )
        .unwrap_or(8) as u8;
        let parity_str = FormUtils::get_text_value(
            configs,
            SERIAL_PORT_TRANSPORT_ID,
            &format!("{}_parity", SERIAL_PORT_TRANSPORT_ID),
        )
        .unwrap_or("none");
        let stop_bits_str = FormUtils::get_text_value(
            configs,
            SERIAL_PORT_TRANSPORT_ID,
            &format!("{}_stop_bits", SERIAL_PORT_TRANSPORT_ID),
        )
        .unwrap_or("1");
        let flow_control_str = FormUtils::get_text_value(
            configs,
            SERIAL_PORT_TRANSPORT_ID,
            &format!("{}_flow_control", SERIAL_PORT_TRANSPORT_ID),
        )
        .unwrap_or("none");

        // Convert config values to tokio_serial types
        let data_bits = match data_bits {
            5 => DataBits::Five,
            6 => DataBits::Six,
            7 => DataBits::Seven,
            8 => DataBits::Eight,
            _ => {
                return Err(CycBoxError::InvalidValue {
                    field: "data_bits".to_string(),
                    reason: format!("Invalid data bits: {data_bits}"),
                });
            }
        };

        let parity = match parity_str {
            "none" => Parity::None,
            "even" => Parity::Even,
            "odd" => Parity::Odd,
            _ => {
                return Err(CycBoxError::InvalidValue {
                    field: "parity".to_string(),
                    reason: format!("Invalid parity: {parity_str}"),
                });
            }
        };

        let stop_bits = match stop_bits_str {
            "1" => StopBits::One,
            "2" => StopBits::Two,
            _ => StopBits::One,
        };

        let flow_control = match flow_control_str {
            "none" => FlowControl::None,
            "software" => FlowControl::Software,
            "hardware" => FlowControl::Hardware,
            _ => FlowControl::None,
        };

        debug!(
            "start serial with port: {port_path}, baud_rate: {baud_rate}, data_bits: {data_bits}, parity: {parity}, stop_bits: {stop_bits}, flow_control: {flow_control}"
        );

        // Create and configure the serial port
        let mut port = cycbox_serialport::new(port_path, baud_rate)
            .data_bits(data_bits)
            .parity(parity)
            .stop_bits(stop_bits)
            .flow_control(flow_control)
            // .timeout(std::time::Duration::from_millis(20))
            .low_latency(true)
            .async_io(true)
            .open_native_async()
            .map_err(|e| CycBoxError::Connection(e.to_string()))?;

        #[cfg(unix)]
        {
            port.set_exclusive(false)
                .map_err(|e| CycBoxError::Connection(e.to_string()))?;
        }

        // Clear input buffer to discard any data that was buffered before opening
        // This prevents receiving old/stale data when the engine starts
        port.clear(ClearBuffer::All)
            .unwrap_or_else(|e| debug!("Failed to clear serial input buffer: {}", e));

        // Warmup: Flush buffered data to clear USB/driver buffers.
        // For continuous streams (data every 1-10ms), we can't wait for "silence",
        // so we flush for a fixed period that's long enough to clear old buffered data
        // but short enough to not delay startup significantly.
        let warmup_start = std::time::Instant::now();
        let overall_timeout = std::time::Duration::from_millis(1500);
        let read_timeout = std::time::Duration::from_millis(3);
        let mut read_buf = vec![0u8; 4096];
        let mut total_discarded = 0usize;
        let mut timeout_count = 0usize;
        let max_timeouts = 2;

        // Flush aggressively until we hit max consecutive timeouts or overall timeout
        while warmup_start.elapsed() < overall_timeout {
            match tokio::time::timeout(read_timeout, port.read(&mut read_buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    // Successfully read data, discard it and reset timeout counter
                    total_discarded += n;
                    timeout_count = 0;
                }
                Ok(Ok(_)) => {
                    // Read 0 bytes (EOF), shouldn't happen for serial port
                    break;
                }
                Ok(Err(e)) => {
                    // Read error
                    debug!("Serial warmup: read error: {}", e);
                    break;
                }
                Err(_) => {
                    // Timeout - no data available within 20ms
                    timeout_count += 1;
                    if timeout_count >= max_timeouts {
                        // No data for 10 consecutive timeouts, buffer is likely empty
                        break;
                    }
                }
            }
        }

        if total_discarded > 10240 {
            debug!(
                "Serial warmup complete: discarded {} bytes in {}ms",
                total_discarded,
                warmup_start.elapsed().as_millis()
            );
        }

        // Wrap the port in SerialTransport, then in CodecTransport for message-based interface
        let transport = Box::new(SerialTransportIO::new(port)) as Box<dyn TransportIO>;
        Ok(Box::new(CodecTransport::new(transport, codec, timeout)))
    }
}

/// Wrapper around a serial port that implements the byte-stream TransportIO trait.
pub struct SerialTransportIO {
    inner: SerialStream,
}

impl SerialTransportIO {
    pub fn new(inner: SerialStream) -> Self {
        Self { inner }
    }
}

impl AsyncRead for SerialTransportIO {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for SerialTransportIO {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[async_trait]
impl TransportIO for SerialTransportIO {}
