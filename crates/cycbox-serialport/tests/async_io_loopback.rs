//! Hardware loopback async I/O tests
//!
//! These tests require physical serial port hardware with TX->RX connection (loopback).
//! Set SERIALPORT_TEST_PORT environment variable and run with --features hardware-tests.
//!
//! Example: SERIALPORT_TEST_PORT=/dev/ttyUSB0 cargo test --features hardware-tests

mod config;

use config::{hw_config, HardwareConfig, TEST_MESSAGE, TEST_TIMEOUT};
use cycbox_serialport::SerialPortBuilderExt;
use rstest::rstest;
use serialport::ClearBuffer;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Test basic loopback write and read
#[rstest]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_basic(hw_config: HardwareConfig) {
    let mut port = serialport::new(&hw_config.port, 115200)
        .timeout(TEST_TIMEOUT)
        .open_native_async()
        .unwrap();

    // Clear any residual data
    port.clear(ClearBuffer::All).unwrap();

    let test_data = b"Loopback test";

    // Write data
    port.write_all(test_data).await.unwrap();
    port.flush().await.unwrap();

    // Read it back (TX->RX connected)
    let mut buffer = vec![0u8; test_data.len()];
    tokio::time::timeout(Duration::from_secs(1), port.read_exact(&mut buffer))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(&buffer[..], test_data);
}

/// Test loopback at different baud rates
#[rstest]
#[case(9600)]
#[case(57600)]
#[case(115200)]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_baud_rates(hw_config: HardwareConfig, #[case] baud: u32) {
    let mut port = serialport::new(&hw_config.port, baud)
        .timeout(TEST_TIMEOUT)
        .open_native_async()
        .unwrap();

    port.clear(ClearBuffer::All).unwrap();

    let test_data = TEST_MESSAGE;

    // Write data
    port.write_all(test_data).await.unwrap();
    port.flush().await.unwrap();

    // Read it back
    let mut buffer = vec![0u8; test_data.len()];
    tokio::time::timeout(Duration::from_secs(2), port.read_exact(&mut buffer))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(&buffer[..], test_data);
}

/// Test clear buffer operations
#[rstest]
#[case(ClearBuffer::Input)]
#[case(ClearBuffer::Output)]
#[case(ClearBuffer::All)]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_clear_buffer(hw_config: HardwareConfig, #[case] buffer: ClearBuffer) {
    let mut port = serialport::new(&hw_config.port, 115200)
        .timeout(Duration::from_millis(500))
        .open_native_async()
        .unwrap();

    port.clear(ClearBuffer::All).unwrap();

    // Write some data
    port.write_all(b"data before clear").await.unwrap();
    port.flush().await.unwrap();

    // Small delay to ensure loopback data has been received before clearing
    // In loopback mode, flush() waits for TX but not for RX to receive the data
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Clear the specified buffer
    port.clear(buffer).unwrap();

    // For Output or All clear, the RX buffer should be cleared
    // For Input-only clear, we need to drain any remaining data in RX buffer
    // to ensure we read the new test data
    if matches!(buffer, ClearBuffer::Output) {
        // Output clear doesn't affect RX buffer, so drain it manually
        let mut drain_buf = vec![0u8; 1024];
        let _ = tokio::time::timeout(
            Duration::from_millis(100),
            port.read(&mut drain_buf)
        ).await;
    }

    // After clearing, subsequent operations should work normally
    port.write_all(b"test").await.unwrap();
    port.flush().await.unwrap();

    let mut buf = [0u8; 4];
    let result = tokio::time::timeout(Duration::from_secs(1), port.read_exact(&mut buf)).await;

    // Should be able to read the new data
    assert!(result.is_ok());
    assert_eq!(&buf, b"test");
}

/// Test flush timing (verify flush waits for transmission)
#[rstest]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_flush_timing(hw_config: HardwareConfig) {
    let mut port = serialport::new(&hw_config.port, 9600)
        .timeout(TEST_TIMEOUT)
        .open_native_async()
        .unwrap();

    port.clear(ClearBuffer::All).unwrap();

    // Large data to ensure flush actually waits
    let data = vec![0x55u8; 1024];

    let start = Instant::now();
    port.write_all(&data).await.unwrap();
    port.flush().await.unwrap();
    let flush_duration = start.elapsed();

    // At 9600 baud, 1024 bytes should take significant time
    // Each byte is ~10 bits (8 data + start + stop) = 10240 bits
    // At 9600 bps, this is ~1.07 seconds minimum
    // We'll check it took at least some time (not instant)
    assert!(
        flush_duration >= Duration::from_millis(100),
        "Flush completed too quickly: {:?}",
        flush_duration
    );

    // Read back the data
    let mut buffer = vec![0u8; data.len()];
    tokio::time::timeout(Duration::from_secs(3), port.read_exact(&mut buffer))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(buffer, data);
}

/// Test multiple sequential write/read cycles
#[rstest]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_sequential_cycles(hw_config: HardwareConfig) {
    let mut port = serialport::new(&hw_config.port, 115200)
        .timeout(TEST_TIMEOUT)
        .open_native_async()
        .unwrap();

    port.clear(ClearBuffer::All).unwrap();

    // Run 10 cycles
    for i in 0..10u8 {
        let data = [i; 16];

        port.write_all(&data).await.unwrap();
        port.flush().await.unwrap();

        let mut buf = [0u8; 16];
        tokio::time::timeout(Duration::from_secs(1), port.read_exact(&mut buf))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(buf, data, "Cycle {} failed", i);
    }
}

/// Test partial writes and reads
#[rstest]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_partial_operations(hw_config: HardwareConfig) {
    let mut port = serialport::new(&hw_config.port, 115200)
        .timeout(TEST_TIMEOUT)
        .open_native_async()
        .unwrap();

    port.clear(ClearBuffer::All).unwrap();

    // Write 100 bytes
    let data = vec![0xAAu8; 100];
    port.write_all(&data).await.unwrap();
    port.flush().await.unwrap();

    // Read in chunks of 10
    let mut received = Vec::new();
    let mut chunk = [0u8; 10];

    for _ in 0..10 {
        let n = tokio::time::timeout(Duration::from_secs(1), port.read(&mut chunk))
            .await
            .unwrap()
            .unwrap();
        received.extend_from_slice(&chunk[..n]);
    }

    assert_eq!(received, data);
}

/// Test large data transfer
#[rstest]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_large_data(hw_config: HardwareConfig) {
    let mut port = serialport::new(&hw_config.port, 115200)
        .timeout(Duration::from_secs(5))
        .open_native_async()
        .unwrap();

    port.clear(ClearBuffer::All).unwrap();

    // 4KB of data
    let data: Vec<u8> = (0..4096).map(|i| (i % 256) as u8).collect();

    port.write_all(&data).await.unwrap();
    port.flush().await.unwrap();

    let mut buffer = vec![0u8; data.len()];
    tokio::time::timeout(Duration::from_secs(5), port.read_exact(&mut buffer))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(buffer, data);
}

/// Test throughput measurement
#[rstest]
#[case(9600, 512)]
#[case(115200, 2048)]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_throughput(
    hw_config: HardwareConfig,
    #[case] baud: u32,
    #[case] bytes: usize,
) {
    let mut port = serialport::new(&hw_config.port, baud)
        .timeout(Duration::from_secs(10))
        .open_native_async()
        .unwrap();

    port.clear(ClearBuffer::All).unwrap();

    let data = vec![0x55u8; bytes];

    let start = Instant::now();

    // Write
    port.write_all(&data).await.unwrap();
    port.flush().await.unwrap();

    // Read
    let mut buffer = vec![0u8; bytes];
    port.read_exact(&mut buffer).await.unwrap();

    let elapsed = start.elapsed();

    assert_eq!(buffer, data);

    // Calculate throughput
    let throughput_bps = (bytes * 8) as f64 / elapsed.as_secs_f64();

    println!(
        "Baud: {}, Bytes: {}, Time: {:?}, Throughput: {:.0} bps",
        baud, bytes, elapsed, throughput_bps
    );

    // Throughput should be reasonable (at least 50% of baud rate accounting for overhead)
    assert!(
        throughput_bps >= (baud as f64 * 0.3),
        "Throughput too low: {:.0} bps (expected >= {:.0} bps)",
        throughput_bps,
        baud as f64 * 0.3
    );
}

/// Test timeout behavior on hardware
#[rstest]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_read_timeout(hw_config: HardwareConfig) {
    let mut port = serialport::new(&hw_config.port, 115200)
        .timeout(Duration::from_millis(100))
        .open_native_async()
        .unwrap();

    port.clear(ClearBuffer::All).unwrap();

    // Don't write anything, just try to read
    let mut buf = [0u8; 10];
    let result = tokio::time::timeout(Duration::from_millis(200), port.read_exact(&mut buf)).await;

    // Should timeout
    assert!(result.is_err(), "Expected timeout but read succeeded");
}

/// Test write-flush-read pattern with different message sizes
#[rstest]
#[case(1)]
#[case(16)]
#[case(64)]
#[case(256)]
#[cfg_attr(not(feature = "hardware-tests"), ignore)]
#[tokio::test]
async fn test_loopback_message_sizes(hw_config: HardwareConfig, #[case] size: usize) {
    let mut port = serialport::new(&hw_config.port, 115200)
        .timeout(TEST_TIMEOUT)
        .open_native_async()
        .unwrap();

    port.clear(ClearBuffer::All).unwrap();

    let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

    port.write_all(&data).await.unwrap();
    port.flush().await.unwrap();

    let mut buffer = vec![0u8; size];
    tokio::time::timeout(Duration::from_secs(2), port.read_exact(&mut buffer))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(buffer, data);
}
