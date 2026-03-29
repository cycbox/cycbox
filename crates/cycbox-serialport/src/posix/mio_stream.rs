use mio::event::Source;
use mio::unix::SourceFd;
use mio::{Interest, Registry, Token};
use serialport::{ClearBuffer, Result, SerialPort, SerialPortBuilder, TTYPort};
use std::io;
use std::io::ErrorKind::Interrupted;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, RawFd};

pub(crate) struct MioStream {
    inner: TTYPort,
}

#[allow(dead_code)]
impl MioStream {
    /// Open a nonblocking serial port from the provided builder
    pub fn open(builder: &SerialPortBuilder) -> Result<Self> {
        let port = serialport::TTYPort::open(builder)?;
        Ok(MioStream { inner: port })
    }

    /// Sets the exclusivity of the port (Unix only)
    pub fn set_exclusive(&mut self, exclusive: bool) -> Result<()> {
        self.inner.set_exclusive(exclusive)
    }

    /// Returns the exclusivity of the port (Unix only)
    ///
    /// If a port is exclusive, then trying to open the same device path again
    /// will fail.
    pub fn exclusive(&self) -> bool {
        self.inner.exclusive()
    }

    #[inline(always)]
    pub(crate) fn clear(&self, buffer_to_clear: ClearBuffer) -> serialport::Result<()> {
        self.inner.clear(buffer_to_clear)
    }

    /// Sets the state of the RTS (Request To Send) control signal
    pub fn write_request_to_send(&mut self, level: bool) -> Result<()> {
        self.inner.write_request_to_send(level)
    }

    /// Sets the state of the DTR (Data Terminal Ready) control signal
    pub fn write_data_terminal_ready(&mut self, level: bool) -> Result<()> {
        self.inner.write_data_terminal_ready(level)
    }

    /// Reads the state of the CTS (Clear To Send) control signal
    pub fn read_clear_to_send(&mut self) -> Result<bool> {
        self.inner.read_clear_to_send()
    }

    /// Reads the state of the DSR (Data Set Ready) control signal
    pub fn read_data_set_ready(&mut self) -> Result<bool> {
        self.inner.read_data_set_ready()
    }
}

impl Read for MioStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match unsafe {
                libc::read(
                    self.as_raw_fd(),
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len() as libc::size_t,
                )
            } {
                n if n >= 0 => return Ok(n as usize),
                _ => {
                    let err = io::Error::last_os_error();
                    if err.kind() != Interrupted {
                        return Err(err);
                    }
                    // Retry on EINTR
                }
            }
        }
    }
}

#[cfg(unix)]
impl Write for MioStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        loop {
            match unsafe {
                libc::write(
                    self.as_raw_fd(),
                    buf.as_ptr() as *const libc::c_void,
                    buf.len() as libc::size_t,
                )
            } {
                n if n >= 0 => return Ok(n as usize),
                _ => {
                    let err = io::Error::last_os_error();
                    if err.kind() != Interrupted {
                        return Err(err);
                    }
                    // Retry on EINTR
                }
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use nix::sys::termios;
        use std::os::fd::BorrowedFd;

        loop {
            match termios::tcdrain(unsafe { BorrowedFd::borrow_raw(self.inner.as_raw_fd()) }) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    let io_err = io::Error::from(e);
                    if io_err.kind() != Interrupted {
                        return Err(io_err);
                    }
                    // Retry on EINTR
                }
            }
        }
    }
}

#[cfg(unix)]
impl Read for &MioStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match unsafe {
                libc::read(
                    self.as_raw_fd(),
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len() as libc::size_t,
                )
            } {
                n if n >= 0 => return Ok(n as usize),
                _ => {
                    let err = io::Error::last_os_error();
                    if err.kind() != Interrupted {
                        return Err(err);
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
impl Write for &MioStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        loop {
            match unsafe {
                libc::write(
                    self.as_raw_fd(),
                    buf.as_ptr() as *const libc::c_void,
                    buf.len() as libc::size_t,
                )
            } {
                n if n >= 0 => return Ok(n as usize),
                _ => {
                    let err = io::Error::last_os_error();
                    if err.kind() != Interrupted {
                        return Err(err);
                    }
                }
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use nix::sys::termios;
        use std::os::fd::BorrowedFd;

        loop {
            match termios::tcdrain(unsafe { BorrowedFd::borrow_raw(self.inner.as_raw_fd()) }) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    let io_err = io::Error::from(e);
                    if io_err.kind() != Interrupted {
                        return Err(io_err);
                    }
                }
            }
        }
    }
}

impl AsRawFd for MioStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl Source for MioStream {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interests: Interest,
    ) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        SourceFd(&self.as_raw_fd()).deregister(registry)
    }
}
