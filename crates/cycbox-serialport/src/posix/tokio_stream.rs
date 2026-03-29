use crate::posix::mio_stream::MioStream;
use crate::SerialControl;
use serialport::{ClearBuffer, SerialPortBuilder};
use std::io;
use std::io::{Read, Write};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub struct SerialStream {
    inner: AsyncFd<MioStream>,
}

impl SerialStream {
    pub fn open(builder: &SerialPortBuilder) -> serialport::Result<Self> {
        let mio_stream = MioStream::open(builder)?;
        let async_fd = AsyncFd::new(mio_stream)?;
        Ok(SerialStream { inner: async_fd })
    }

    #[inline(always)]
    pub fn clear(&self, buffer_to_clear: ClearBuffer) -> serialport::Result<()> {
        self.inner.get_ref().clear(buffer_to_clear)
    }

    pub fn set_exclusive(&mut self, exclusive: bool) -> serialport::Result<()> {
        self.inner.get_mut().set_exclusive(exclusive)
    }
}

impl AsyncRead for SerialStream {
    /// Attempts to read bytes on the serial port.
    ///
    /// Note that on multiple calls to a `poll_*` method in the read direction, only the
    /// `Waker` from the `Context` passed to the most recent call will be scheduled to
    /// receive a wakeup.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not ready to read
    /// * `Poll::Ready(Ok(()))` reads data `ReadBuf` if the socket is ready
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        use futures::ready;

        loop {
            let mut guard = ready!(self.inner.poll_read_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().read(buf.initialize_unfilled())) {
                Ok(Ok(bytes_read)) => {
                    buf.advance(bytes_read);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(err)) => {
                    return Poll::Ready(Err(err));
                }
                Err(_would_block) => continue,
            }
        }
    }
}

#[cfg(unix)]
impl AsyncWrite for SerialStream {
    /// Attempts to send data on the serial port
    ///
    /// Note that on multiple calls to a `poll_*` method in the send direction,
    /// only the `Waker` from the `Context` passed to the most recent call will
    /// be scheduled to receive a wakeup.
    ///
    /// # Return value
    ///
    /// The function returns:
    ///
    /// * `Poll::Pending` if the socket is not available to write
    /// * `Poll::Ready(Ok(n))` `n` is the number of bytes sent
    /// * `Poll::Ready(Err(e))` if an error is encountered.
    ///
    /// # Errors
    ///
    /// This function may encounter any standard I/O error except `WouldBlock`.
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        use futures::ready;

        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().write(buf)) {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        use futures::ready;

        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;
            match guard.try_io(|inner| inner.get_ref().flush()) {
                Ok(_) => return Poll::Ready(Ok(())),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let _ = self.poll_flush(cx)?;
        Ok(()).into()
    }
}

impl SerialControl for SerialStream {
    fn write_request_to_send(&mut self, level: bool) -> serialport::Result<()> {
        self.inner.get_mut().write_request_to_send(level)
    }

    fn write_data_terminal_ready(&mut self, level: bool) -> serialport::Result<()> {
        self.inner.get_mut().write_data_terminal_ready(level)
    }

    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        self.inner.get_mut().read_clear_to_send()
    }

    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        self.inner.get_mut().read_data_set_ready()
    }
}
