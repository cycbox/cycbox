use crate::SerialControl;
use serialport::{COMPort, ClearBuffer, SerialPort};
use std::io::Read;
use std::os::windows::io::AsRawHandle;
use std::pin::Pin;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use windows_sys::Win32::Devices::Communication::{COMSTAT, ClearCommError};
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile};
use windows_sys::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Threading::CreateEventW;
use windows_sys::Win32::System::Threading::{
    INFINITE, RegisterWaitForSingleObject, UnregisterWait, UnregisterWaitEx, WT_EXECUTEONLYONCE,
};

// Windows error codes
const ERROR_IO_PENDING: u32 = 997;

// State for overlapped read operations
struct ReadState {
    overlapped: Box<OVERLAPPED>,
    event: HANDLE,
    wait_handle: Option<HANDLE>,
    waker: Option<Waker>,
    buffer: Vec<u8>,
    pending: bool,
    completed: bool,
    bytes_transferred: u32,
}

// Safety: Windows HANDLEs are thread-safe kernel objects that can be used from any thread.
unsafe impl Send for ReadState {}

impl ReadState {
    fn new() -> std::io::Result<Self> {
        // Create manual-reset event for overlapped I/O
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if event == (0 as *mut core::ffi::c_void) || event == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }

        let mut overlapped = Box::new(unsafe { std::mem::zeroed::<OVERLAPPED>() });
        overlapped.hEvent = event;

        Ok(ReadState {
            overlapped,
            event,
            wait_handle: None,
            waker: None,
            buffer: Vec::new(),
            pending: false,
            completed: false,
            bytes_transferred: 0,
        })
    }

    fn reset(&mut self) {
        self.pending = false;
        self.completed = false;
        self.bytes_transferred = 0;
        self.waker = None;
        if let Some(wait_handle) = self.wait_handle.take() {
            unsafe {
                UnregisterWait(wait_handle);
            }
        }
    }
}

impl Drop for ReadState {
    fn drop(&mut self) {
        if let Some(wait_handle) = self.wait_handle.take() {
            unsafe {
                // Use UnregisterWaitEx with INVALID_HANDLE_VALUE to wait for callbacks to complete
                UnregisterWaitEx(wait_handle, INVALID_HANDLE_VALUE);
            }
        }
        if self.event != (0 as *mut core::ffi::c_void) && self.event != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.event);
            }
        }
    }
}

// State for overlapped write operations
struct WriteState {
    overlapped: Box<OVERLAPPED>,
    event: HANDLE,
    wait_handle: Option<HANDLE>,
    waker: Option<Waker>,
    buffer: Vec<u8>,
    pending: bool,
    completed: bool,
    bytes_transferred: u32,
}

// Safety: Windows HANDLEs are thread-safe kernel objects that can be used from any thread.
// They are opaque identifiers (not actual pointers to memory) managed by the kernel.
// The HANDLE values themselves are just integers that the kernel uses to lookup objects.
unsafe impl Send for WriteState {}

impl WriteState {
    fn new() -> std::io::Result<Self> {
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if event == (0 as *mut core::ffi::c_void) || event == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }

        let mut overlapped = Box::new(unsafe { std::mem::zeroed::<OVERLAPPED>() });
        overlapped.hEvent = event;

        Ok(WriteState {
            overlapped,
            event,
            wait_handle: None,
            waker: None,
            buffer: Vec::new(),
            pending: false,
            completed: false,
            bytes_transferred: 0,
        })
    }

    fn reset(&mut self) {
        self.pending = false;
        self.completed = false;
        self.bytes_transferred = 0;
        self.waker = None;
        if let Some(wait_handle) = self.wait_handle.take() {
            unsafe {
                UnregisterWait(wait_handle);
            }
        }
    }
}

impl Drop for WriteState {
    fn drop(&mut self) {
        if let Some(wait_handle) = self.wait_handle.take() {
            unsafe {
                UnregisterWaitEx(wait_handle, INVALID_HANDLE_VALUE);
            }
        }
        if self.event != (0 as *mut core::ffi::c_void) && self.event != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.event);
            }
        }
    }
}

pub struct SerialStream {
    inner: COMPort,
    read_state: Arc<Mutex<ReadState>>,
    write_state: Arc<Mutex<WriteState>>,
}

impl SerialStream {
    pub fn open(builder: &serialport::SerialPortBuilder) -> serialport::Result<Self> {
        let com_port = COMPort::open(builder)?;

        let read_state = Arc::new(Mutex::new(ReadState::new().map_err(|e| {
            serialport::Error {
                kind: serialport::ErrorKind::Io(e.kind()),
                description: e.to_string(),
            }
        })?));

        let write_state = Arc::new(Mutex::new(WriteState::new().map_err(|e| {
            serialport::Error {
                kind: serialport::ErrorKind::Io(e.kind()),
                description: e.to_string(),
            }
        })?));

        Ok(SerialStream {
            inner: com_port,
            read_state,
            write_state,
        })
    }

    #[inline(always)]
    pub fn clear(&self, buffer_to_clear: ClearBuffer) -> serialport::Result<()> {
        self.inner.clear(buffer_to_clear)
    }
}

// Callback for RegisterWaitForSingleObject
unsafe extern "system" fn read_completion_callback(
    context: *mut std::ffi::c_void,
    _timer_fired: bool,
) {
    let state = context as *const Mutex<ReadState>;
    if let Some(state) = state.as_ref() {
        if let Ok(mut state) = state.lock() {
            state.completed = true;
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        }
    }
}

unsafe extern "system" fn write_completion_callback(
    context: *mut std::ffi::c_void,
    _timer_fired: bool,
) {
    let state = context as *const Mutex<WriteState>;
    if let Some(state) = state.as_ref() {
        if let Ok(mut state) = state.lock() {
            state.completed = true;
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        }
    }
}

impl AsyncRead for SerialStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let handle = this.inner.as_raw_handle() as HANDLE;

        let mut state = match this.read_state.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to lock read state",
                )));
            }
        };

        // If there's a completed overlapped operation, retrieve the results
        if state.completed {
            let mut bytes_transferred = 0u32;
            let result = unsafe {
                GetOverlappedResult(
                    handle,
                    state.overlapped.as_ref() as *const _ as *mut _,
                    &mut bytes_transferred,
                    0, // Don't wait
                )
            };

            if result == 0 {
                let err = std::io::Error::last_os_error();
                state.reset();
                return Poll::Ready(Err(err));
            }

            // Copy data from internal buffer to output buffer
            let bytes_to_copy = std::cmp::min(bytes_transferred as usize, buf.remaining());
            if bytes_to_copy > 0 {
                unsafe {
                    ptr::copy_nonoverlapping(
                        state.buffer.as_ptr(),
                        buf.unfilled_mut().as_mut_ptr() as *mut u8,
                        bytes_to_copy,
                    );
                    buf.assume_init(bytes_to_copy);
                }
                buf.advance(bytes_to_copy);
            }

            state.reset();
            return Poll::Ready(Ok(()));
        }

        // If already pending, just update waker and return
        if state.pending {
            state.waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        // Check for available data using ClearCommError (optimization)
        let mut errors: u32 = 0;
        let mut stat: COMSTAT = unsafe { std::mem::zeroed() };
        let result = unsafe { ClearCommError(handle, &mut errors, &mut stat) };

        if result == 0 {
            return Poll::Ready(Err(std::io::Error::last_os_error()));
        }

        // Prepare buffer for overlapped read
        let bytes_to_read = if stat.cbInQue > 0 {
            std::cmp::min(stat.cbInQue as usize, buf.remaining())
        } else {
            // No data immediately available, but start a read anyway
            std::cmp::min(4096, buf.remaining())
        };

        if bytes_to_read == 0 {
            // Output buffer is full
            return Poll::Ready(Ok(()));
        }

        state.buffer.resize(bytes_to_read, 0);

        // Reset overlapped structure
        unsafe {
            ptr::write_bytes(state.overlapped.as_mut() as *mut OVERLAPPED, 0, 1);
            state.overlapped.hEvent = state.event;
        }

        // Start overlapped read
        let mut bytes_read = 0u32;
        let result = unsafe {
            ReadFile(
                handle,
                state.buffer.as_mut_ptr() as *mut _,
                bytes_to_read as u32,
                &mut bytes_read,
                state.overlapped.as_mut() as *mut _,
            )
        };

        if result != 0 {
            // Read completed immediately
            let bytes_to_copy = std::cmp::min(bytes_read as usize, buf.remaining());
            if bytes_to_copy > 0 {
                unsafe {
                    ptr::copy_nonoverlapping(
                        state.buffer.as_ptr(),
                        buf.unfilled_mut().as_mut_ptr() as *mut u8,
                        bytes_to_copy,
                    );
                    buf.assume_init(bytes_to_copy);
                }
                buf.advance(bytes_to_copy);
            }
            return Poll::Ready(Ok(()));
        }

        // Check if operation is pending
        let err = unsafe { GetLastError() };
        if err != ERROR_IO_PENDING {
            return Poll::Ready(Err(std::io::Error::from_raw_os_error(err as i32)));
        }

        // Operation is pending - register wait callback
        let mut wait_handle: HANDLE = ptr::null_mut();
        let state_ptr = Arc::as_ptr(&this.read_state) as *mut std::ffi::c_void;

        let result = unsafe {
            RegisterWaitForSingleObject(
                &mut wait_handle,
                state.event,
                Some(read_completion_callback),
                state_ptr,
                INFINITE,
                WT_EXECUTEONLYONCE,
            )
        };

        if result == 0 {
            return Poll::Ready(Err(std::io::Error::last_os_error()));
        }

        state.wait_handle = Some(wait_handle);
        state.pending = true;
        state.waker = Some(cx.waker().clone());

        Poll::Pending
    }
}

impl AsyncWrite for SerialStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        let handle = this.inner.as_raw_handle() as HANDLE;

        let mut state = match this.write_state.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to lock write state",
                )));
            }
        };

        // If there's a completed overlapped operation, retrieve the results
        if state.completed {
            let mut bytes_transferred = 0u32;
            let result = unsafe {
                GetOverlappedResult(
                    handle,
                    state.overlapped.as_ref() as *const _ as *mut _,
                    &mut bytes_transferred,
                    0, // Don't wait
                )
            };

            if result == 0 {
                let err = std::io::Error::last_os_error();
                state.reset();
                return Poll::Ready(Err(err));
            }

            let bytes_written = bytes_transferred as usize;
            state.reset();
            return Poll::Ready(Ok(bytes_written));
        }

        // If already pending, just update waker and return
        if state.pending {
            state.waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        // Nothing to write
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        // Copy data to internal buffer (needed because overlapped I/O requires buffer to remain valid)
        state.buffer.clear();
        state.buffer.extend_from_slice(buf);

        // Reset overlapped structure
        unsafe {
            ptr::write_bytes(state.overlapped.as_mut() as *mut OVERLAPPED, 0, 1);
            state.overlapped.hEvent = state.event;
        }

        // Start overlapped write
        let mut bytes_written = 0u32;
        let result = unsafe {
            WriteFile(
                handle,
                state.buffer.as_ptr() as *const _,
                state.buffer.len() as u32,
                &mut bytes_written,
                state.overlapped.as_mut() as *mut _,
            )
        };

        if result != 0 {
            // Write completed immediately
            return Poll::Ready(Ok(bytes_written as usize));
        }

        // Check if operation is pending
        let err = unsafe { GetLastError() };
        if err != ERROR_IO_PENDING {
            return Poll::Ready(Err(std::io::Error::from_raw_os_error(err as i32)));
        }

        // Operation is pending - register wait callback
        let mut wait_handle: HANDLE = ptr::null_mut();
        let state_ptr = Arc::as_ptr(&this.write_state) as *mut std::ffi::c_void;

        let result = unsafe {
            RegisterWaitForSingleObject(
                &mut wait_handle,
                state.event,
                Some(write_completion_callback),
                state_ptr,
                INFINITE,
                WT_EXECUTEONLYONCE,
            )
        };

        if result == 0 {
            return Poll::Ready(Err(std::io::Error::last_os_error()));
        }

        state.wait_handle = Some(wait_handle);
        state.pending = true;
        state.waker = Some(cx.waker().clone());

        Poll::Pending
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let handle = this.inner.as_raw_handle() as HANDLE;

        let result = unsafe { FlushFileBuffers(handle) };

        if result == 0 {
            return Poll::Ready(Err(std::io::Error::last_os_error()));
        }

        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        // For serial ports, shutdown is the same as flush
        self.poll_flush(cx)
    }
}

impl Drop for SerialStream {
    fn drop(&mut self) {}
}

impl SerialControl for SerialStream {
    fn write_request_to_send(&mut self, level: bool) -> serialport::Result<()> {
        self.inner.write_request_to_send(level)
    }

    fn write_data_terminal_ready(&mut self, level: bool) -> serialport::Result<()> {
        self.inner.write_data_terminal_ready(level)
    }

    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        self.inner.read_clear_to_send()
    }

    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        self.inner.read_data_set_ready()
    }
}
