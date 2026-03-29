#[cfg(unix)]
mod posix;
#[cfg(unix)]
pub use posix::SerialStream;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::SerialStream;

pub use serialport::{
    ClearBuffer, DataBits, FlowControl, Parity, SerialPort, SerialPortBuilder, SerialPortType,
    StopBits, available_ports, new,
};

/// Serial port control signal operations
///
/// This trait provides methods to read and write control signals (RTS, DTR, CTS, DSR)
/// for serial port communication.
pub trait SerialControl {
    /// Sets the state of the RTS (Request To Send) control signal
    ///
    /// Setting a value of `true` asserts the RTS control signal. `false` clears the signal.
    fn write_request_to_send(&mut self, level: bool) -> serialport::Result<()>;

    /// Sets the state of the DTR (Data Terminal Ready) control signal
    ///
    /// Setting a value of `true` asserts the DTR control signal. `false` clears the signal.
    fn write_data_terminal_ready(&mut self, level: bool) -> serialport::Result<()>;

    /// Reads the state of the CTS (Clear To Send) control signal
    ///
    /// Returns `true` if the CTS control signal is asserted.
    fn read_clear_to_send(&mut self) -> serialport::Result<bool>;

    /// Reads the state of the DSR (Data Set Ready) control signal
    ///
    /// Returns `true` if the DSR control signal is asserted.
    fn read_data_set_ready(&mut self) -> serialport::Result<bool>;
}

/// Extension trait for [`SerialPortBuilder`] to open async serial ports
///
/// This trait extends [`SerialPortBuilder`] with the `open_native_async()` method,
/// which creates a [`SerialStream`] configured for asynchronous I/O with tokio.
///
/// # Additional Builder Methods
///
/// The [`SerialPortBuilder`] also supports additional configuration methods
/// from the underlying serialport library:
///
/// - `low_latency(enable: bool)` - Enable/disable low latency mode for sub-millisecond performance
/// - `async_io(enable: bool)` - Enable/disable asynchronous I/O mode
///
/// These methods are available directly on the builder returned by [`new()`].
///
/// # Example
/// ```no_run
/// use cycbox_serialport::SerialPortBuilderExt;
///
/// # async fn example() -> serialport::Result<()> {
/// let port = cycbox_serialport::new("/dev/ttyUSB0", 115200)
///     .low_latency(true)   // Available from serialport builder
///     .async_io(true)      // Available from serialport builder
///     .open_native_async()?;
/// # Ok(())
/// # }
/// ```
pub trait SerialPortBuilderExt {
    /// Open a platform-specific interface to the port configured for async I/O
    ///
    /// This method creates a [`SerialStream`] that can be used with tokio for
    /// async serial port communication.
    fn open_native_async(self) -> serialport::Result<SerialStream>;
}

impl SerialPortBuilderExt for SerialPortBuilder {
    fn open_native_async(self) -> serialport::Result<SerialStream> {
        SerialStream::open(&self)
    }
}

pub fn default_port() -> Option<String> {
    let ports = available_ports().ok()?;

    if ports.is_empty() {
        return None;
    }

    #[cfg(target_os = "linux")]
    {
        // Priority order for Linux: ttyUSB*, ttyACM*, then others

        // First try to find ttyUSB* ports
        if let Some(port) = ports.iter().find(|p| p.port_name.contains("ttyUSB")) {
            return Some(port.port_name.clone());
        }

        // Then try ttyACM* ports
        if let Some(port) = ports.iter().find(|p| p.port_name.contains("ttyACM")) {
            return Some(port.port_name.clone());
        }

        // Fall back to the first available port
        ports.first().map(|p| p.port_name.clone())
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, prioritize lower-numbered COM ports
        let mut com_ports: Vec<_> = ports
            .iter()
            .filter_map(|p| {
                // Extract COM port number (e.g., "COM3" -> 3)
                if p.port_name.starts_with("COM") {
                    p.port_name[3..]
                        .parse::<u32>()
                        .ok()
                        .map(|num| (num, &p.port_name))
                } else {
                    None
                }
            })
            .collect();

        if !com_ports.is_empty() {
            // Sort by COM port number (lowest first)
            com_ports.sort_by_key(|(num, _)| *num);
            return Some(com_ports[0].1.clone());
        }

        // Fall back to the first available port if no COM ports found
        ports.first().map(|p| p.port_name.clone())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        // For other platforms, just return the first port
        ports.first().map(|p| p.port_name.clone())
    }
}
