# cycbox-serialport

A cross-platform Rust library for asynchronous serial port communication using tokio.

## Overview

`cycbox-serialport` is a high-performance async serial port library specifically designed for [CycBox](https://github.com/cycbox/cycbox). Built on top of [serialport-rs](https://github.com/serialport/serialport-rs) with native tokio integration, it provides an async interface for serial port I/O operations with platform-specific optimizations for both POSIX (Unix/Linux) and Windows systems.

**Performance Goal**: Achieve serial port read/write latency of **less than 1ms** to meet the real-time requirements of CycBox.

[High-Precision Serial Port Testing: 1ms Timer & Low-Latency with CycBox](https://youtu.be/iwMMNEYILbk)

## Features

- **Low Latency**: Optimized for sub-1ms read/write latency with platform-specific async I/O
- **Async I/O**: Full tokio `AsyncRead` and `AsyncWrite` trait implementations
- **Cross-platform**: Native support for Windows and POSIX systems (Linux, macOS, BSD)
- **Serial Control Signals**: Read and write RTS, DTR, CTS, and DSR control signals

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
cycbox-serialport = { version = "0.3.0", git = "https://github.com/cycbox/cycbox-serialport" }
tokio = { version = "1", features = ["rt", "io-util"] }
```

## Usage

### Basic Example

```rust
use cycbox_serialport::{SerialPortBuilderExt, available_ports};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open a serial port with async I/O
    let mut port = cycbox_serialport::new("/dev/ttyUSB0", 115200)
        .open_native_async()?;

    // Write data
    port.write_all(b"Hello, serial port!").await?;
    port.flush().await?;

    // Read data
    let mut buffer = vec![0u8; 128];
    let n = port.read(&mut buffer).await?;
    println!("Received: {:?}", &buffer[..n]);

    Ok(())
}
```

### Low-Latency Configuration

For applications requiring sub-millisecond latency, use the `low_latency()` and `async_io()` configuration options:

```rust
use cycbox_serialport::{SerialPortBuilderExt, DataBits, Parity, StopBits, FlowControl};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut port = cycbox_serialport::new("/dev/ttyUSB0", 115200)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .low_latency(true)      // Enable low latency mode
        .async_io(true)         // Enable async I/O
        .open_native_async()?;

    // Your low-latency serial communication here
    port.write_all(b"Fast!").await?;
    port.flush().await?;

    Ok(())
}
```

## Platform-Specific Features

### POSIX (Unix/Linux/macOS)
- Uses `mio` and tokio's `AsyncFd` for efficient async I/O

### Windows
- Native Windows async I/O using IOCP

## Testing

The library includes comprehensive hardware tests that require a physical serial port with TX-RX loopback connection.

```bash

SERIALPORT_TEST_PORT=/dev/ttyUSB0 cargo test --features hardware-tests -- --test-threads=1

```


## Dependencies

- [serialport](https://github.com/cycbox/serialport-rs) - Core serial port functionality (cycbox fork)
- [tokio](https://tokio.rs) - Async runtime
- [mio](https://github.com/tokio-rs/mio) - Low-level I/O (POSIX)
- [nix](https://github.com/nix-rust/nix) - Unix system calls (POSIX)
- [windows-sys](https://github.com/microsoft/windows-rs) - Windows APIs

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the Mozilla Public License, version 2.0. See [LICENSE](LICENSE) for details.

## Acknowledgments

Built on top of the excellent [serialport-rs](https://github.com/serialport/serialport-rs) library with enhancements for async I/O.
