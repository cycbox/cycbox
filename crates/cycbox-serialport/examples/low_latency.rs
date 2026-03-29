//! Example demonstrating low-latency serial port configuration
//!
//! This example shows how to use the `low_latency()` and `async_io()` methods
//! to configure a serial port for optimal performance.
//!
//! Usage:
//!   cargo run --example low_latency /dev/ttyUSB0

use cycbox_serialport::{
    DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits, default_port,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get port from command line or use default
    let port_name = std::env::args()
        .nth(1)
        .or_else(default_port)
        .expect("No serial port specified and no default port found");

    println!("Opening port: {}", port_name);

    // Configure serial port with low latency settings
    let mut port = cycbox_serialport::new(&port_name, 115200)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .low_latency(true) // Enable low latency mode for sub-1ms performance
        .async_io(true) // Enable async I/O
        .open_native_async()?;

    println!("Port opened successfully with low latency configuration");

    // Send a test message
    let message = b"Hello from low-latency serial port!\n";
    port.write_all(message).await?;
    port.flush().await?;
    println!("Sent: {:?}", String::from_utf8_lossy(message));

    // Read response (with timeout)
    let mut buffer = vec![0u8; 1024];
    match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        port.read(&mut buffer),
    )
    .await
    {
        Ok(Ok(n)) => {
            println!("Received {} bytes: {:?}", n, String::from_utf8_lossy(&buffer[..n]));
        }
        Ok(Err(e)) => {
            eprintln!("Read error: {}", e);
        }
        Err(_) => {
            println!("No response received within timeout");
        }
    }

    Ok(())
}
