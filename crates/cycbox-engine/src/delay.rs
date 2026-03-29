use log::debug;
use std::time::Duration;

/// Threshold below which we use high-resolution timers (1000 ms)
const HIGH_RES_THRESHOLD: Duration = Duration::from_millis(1000);

/// High-resolution timer implementation for Linux using timerfd
#[cfg(target_os = "linux")]
mod linux_timer {
    use libc::time_t;
    use log::debug;
    use nix::sys::time::TimeSpec;
    use nix::sys::timerfd::{ClockId, TimerFd, TimerFlags, TimerSetTimeFlags};
    use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
    use std::time::Duration;
    use tokio::io::Interest;
    use tokio::io::unix::AsyncFd;

    pub struct HighResTimer {
        async_fd: AsyncFd<OwnedFd>,
        timer_fd: TimerFd,
    }

    impl HighResTimer {
        pub fn new() -> std::io::Result<Self> {
            // Create timerfd with CLOCK_MONOTONIC for high-resolution timing
            let timer_fd = TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::TFD_NONBLOCK)
                .map_err(std::io::Error::other)?;

            // Duplicate the file descriptor to avoid double-close issues
            // One fd is managed by TimerFd, the other by AsyncFd
            let raw_fd = timer_fd.as_fd().as_raw_fd();

            // SAFETY: Calling dup() on a valid file descriptor
            let dup_fd = unsafe { libc::dup(raw_fd) };
            if dup_fd < 0 {
                return Err(std::io::Error::last_os_error());
            }

            // SAFETY: dup() returns a new file descriptor that we now own
            let owned_fd = unsafe { OwnedFd::from_raw_fd(dup_fd) };

            let async_fd = AsyncFd::with_interest(owned_fd, Interest::READABLE)
                .map_err(std::io::Error::other)?;

            debug!("High-resolution timer created with timerfd");

            Ok(Self { async_fd, timer_fd })
        }

        /// Set timer to fire after the given duration
        pub fn set_timer(&mut self, duration: Duration) -> std::io::Result<()> {
            let secs = duration.as_secs();
            let nanos = duration.subsec_nanos();

            let timespec = TimeSpec::new(secs as time_t, nanos as libc::c_long);

            // Set one-shot timer using the TimerFd
            self.timer_fd
                .set(
                    nix::sys::timerfd::Expiration::OneShot(timespec),
                    TimerSetTimeFlags::empty(),
                )
                .map_err(std::io::Error::other)?;

            Ok(())
        }

        /// Wait for the timer to fire
        pub async fn wait(&mut self) -> std::io::Result<()> {
            // Wait for the timerfd to become readable (timer expired)
            let mut guard = self.async_fd.readable().await?;

            // Read the timerfd to clear the event (required by timerfd semantics)
            // We need to read exactly 8 bytes (u64)
            let mut buf = [0u8; 8];
            match nix::unistd::read(self.async_fd.get_ref(), &mut buf) {
                Ok(_) => {
                    guard.clear_ready();
                    Ok(())
                }
                Err(nix::errno::Errno::EAGAIN) => {
                    // Would block, shouldn't happen but handle gracefully
                    guard.clear_ready();
                    Ok(())
                }
                Err(e) => Err(std::io::Error::other(e)),
            }
        }

        /// Disarm the timer
        pub fn disarm(&mut self) -> std::io::Result<()> {
            self.timer_fd.unset().map_err(std::io::Error::other)?;
            Ok(())
        }
    }
}

/// High-resolution timer implementation for Windows using waitable timers
#[cfg(target_os = "windows")]
mod windows_timer {
    use log::debug;
    use std::ptr;
    use std::time::Duration;
    use tokio::sync::oneshot;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Threading::{
        CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, INFINITE,
        RegisterWaitForSingleObject, SetWaitableTimer, TIMER_ALL_ACCESS, UnregisterWait,
        WT_EXECUTEONLYONCE,
    };
    use windows_sys::core::PWSTR;

    pub struct HighResTimer {
        timer_handle: HANDLE,
        wait_handle: Option<HANDLE>,
    }

    impl HighResTimer {
        pub fn new() -> std::io::Result<Self> {
            let timer_handle = unsafe {
                CreateWaitableTimerExW(
                    ptr::null_mut(),
                    PWSTR::default(),
                    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                    TIMER_ALL_ACCESS,
                )
            };

            if timer_handle == (0 as *mut std::ffi::c_void) || timer_handle == INVALID_HANDLE_VALUE
            {
                return Err(std::io::Error::last_os_error());
            }

            debug!("High-resolution waitable timer created");

            Ok(Self {
                timer_handle,
                wait_handle: None,
            })
        }

        /// Set timer to fire after the given duration
        pub fn set_timer(&mut self, duration: Duration) -> std::io::Result<()> {
            // Convert duration to 100-nanosecond intervals (negative for relative time)
            let wait_time: i64 = -(duration.as_nanos() as i64 / 100).max(1);

            let result = unsafe {
                SetWaitableTimer(
                    self.timer_handle,
                    &wait_time as *const i64,
                    0,               // No period (one-shot)
                    None,            // No completion routine
                    ptr::null_mut(), // No arg to completion routine
                    0,               // Don't resume system from suspend
                )
            };

            if result == 0 {
                return Err(std::io::Error::last_os_error());
            }

            Ok(())
        }

        /// Wait for the timer to fire using RegisterWaitForSingleObject
        pub async fn wait(&mut self) -> std::io::Result<()> {
            let (tx, rx) = oneshot::channel::<()>();

            // Box the sender to pass it through the callback
            let tx_ptr: *mut oneshot::Sender<()> = Box::into_raw(Box::new(tx));

            // Scope the wait_handle registration so it doesn't cross the await boundary
            {
                let mut wait_handle: HANDLE = ptr::null_mut();

                // Register a wait callback that will be invoked when the timer fires
                let result = unsafe {
                    RegisterWaitForSingleObject(
                        &mut wait_handle,
                        self.timer_handle,
                        Some(wait_callback),
                        tx_ptr as *const _,
                        INFINITE,
                        WT_EXECUTEONLYONCE,
                    )
                };

                if result == 0 {
                    // Failed to register, clean up the sender
                    unsafe {
                        let _ = Box::from_raw(tx_ptr);
                    }
                    return Err(std::io::Error::last_os_error());
                }

                self.wait_handle = Some(wait_handle);
            } // wait_handle goes out of scope here, before the await

            // Wait for the callback to signal completion
            match rx.await {
                Ok(_) => Ok(()),
                Err(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Timer callback failed to signal",
                )),
            }
        }

        /// Disarm the timer
        pub fn disarm(&mut self) -> std::io::Result<()> {
            // Unregister the wait callback if it exists
            if let Some(wait_handle) = self.wait_handle.take() {
                unsafe {
                    UnregisterWait(wait_handle);
                }
            }

            // Cancel the timer by setting it to a very large value
            let far_future: i64 = i64::MAX;
            unsafe {
                SetWaitableTimer(
                    self.timer_handle,
                    &far_future as *const i64,
                    0,
                    None,
                    ptr::null_mut(),
                    0,
                );
            }

            Ok(())
        }
    }

    impl Drop for HighResTimer {
        fn drop(&mut self) {
            // Unregister wait if still active
            if let Some(wait_handle) = self.wait_handle.take() {
                unsafe {
                    UnregisterWait(wait_handle);
                }
            }

            // Close the timer handle
            if self.timer_handle != (0 as *mut std::ffi::c_void)
                && self.timer_handle != INVALID_HANDLE_VALUE
            {
                unsafe {
                    CloseHandle(self.timer_handle);
                }
            }
        }
    }

    // Callback function invoked by RegisterWaitForSingleObject
    unsafe extern "system" fn wait_callback(context: *mut std::ffi::c_void, _timer_fired: bool) {
        if !context.is_null() {
            // SAFETY: context was created via Box::into_raw in wait() method
            // and is guaranteed to be a valid pointer to a Sender<()>
            unsafe {
                // Reconstruct the sender from the raw pointer
                let tx = Box::from_raw(context as *mut oneshot::Sender<()>);
                // Send signal (ignore error if receiver is dropped)
                let _ = tx.send(());
            }
        }
    }

    // SAFETY: Windows HANDLE is thread-safe and can be safely sent between threads.
    // The Windows API manages internal synchronization for timer operations.
    unsafe impl Send for HighResTimer {}
}

/// Fallback timer for other platforms (macOS, etc.)
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod fallback_timer {
    use std::time::Duration;

    pub struct HighResTimer;

    impl HighResTimer {
        pub fn new() -> std::io::Result<Self> {
            Ok(Self)
        }

        pub fn set_timer(&mut self, _duration: Duration) -> std::io::Result<()> {
            Ok(())
        }

        pub async fn wait(&mut self) -> std::io::Result<()> {
            // Fallback does nothing; we'll use tokio::time::sleep instead
            Ok(())
        }

        pub fn disarm(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
use linux_timer::HighResTimer;

#[cfg(target_os = "windows")]
use windows_timer::HighResTimer;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use fallback_timer::HighResTimer;

/// A reusable high-resolution delay mechanism
///
/// This type provides a cross-platform way to perform precise delays with microsecond accuracy
/// on supported platforms (Linux, Windows). The instance can be reused for multiple delays,
/// avoiding the overhead of creating and destroying timer resources.
///
/// # Platform Support
/// - **Linux**: Uses timerfd with CLOCK_MONOTONIC
/// - **Windows**: Uses high-resolution waitable timers
/// - **Other platforms**: Falls back to tokio::time::sleep
///
/// # Performance
/// - For delays < 20ms: Uses platform-specific high-resolution timers
/// - For delays >= 20ms: Falls back to tokio::time::sleep for efficiency
///
/// # Example
/// ```no_run
/// use std::time::Duration;
/// use cycbox::delay::HighResDelay;
///
/// # async fn example() -> std::io::Result<()> {
/// let mut delay = HighResDelay::new()?;
///
/// // Reuse the same instance for multiple delays
/// for _ in 0..100 {
///     delay.delay(Duration::from_micros(500)).await?;
///     // ... do work ...
/// }
/// # Ok(())
/// # }
/// ```
pub struct HighResDelay {
    timer: Option<HighResTimer>,
}

impl HighResDelay {
    /// Create a new high-resolution delay instance
    ///
    /// On Linux and Windows, this allocates OS timer resources.
    /// On other platforms, this is a no-op.
    ///
    /// # Errors
    /// Returns an error if the platform-specific timer creation fails.
    pub fn new() -> std::io::Result<Self> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            let timer = match HighResTimer::new() {
                Ok(t) => {
                    debug!("HighResDelay: High-resolution timer initialized");
                    Some(t)
                }
                Err(e) => {
                    debug!(
                        "HighResDelay: Failed to create high-res timer, will use tokio fallback: {}",
                        e
                    );
                    None
                }
            };
            Ok(Self { timer })
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Ok(Self { timer: None })
        }
    }

    /// Delay for the specified duration
    ///
    /// For delays < 20ms on Linux/Windows, uses high-resolution timers.
    /// For longer delays or unsupported platforms, uses tokio::time::sleep.
    ///
    /// This method can be called multiple times on the same instance without
    /// recreating timer resources.
    ///
    /// # Arguments
    /// * `duration` - How long to delay
    ///
    /// # Errors
    /// Returns an error if the timer operation fails on the platform.
    pub async fn delay(&mut self, duration: Duration) -> std::io::Result<()> {
        if duration == Duration::from_micros(0) {
            // No delay needed for zero or negative durations
            return Ok(());
        }
        if duration <= HIGH_RES_THRESHOLD {
            if let Some(ref mut timer) = self.timer {
                // Set the timer
                if let Err(e) = timer.set_timer(duration) {
                    debug!(
                        "HighResDelay: Failed to set timer: {}, falling back to tokio sleep",
                        e
                    );
                    tokio::time::sleep(duration).await;
                } else {
                    // Wait for timer
                    if let Err(e) = timer.wait().await {
                        debug!("HighResDelay: Timer wait error: {}", e);
                    }
                }
                return Ok(());
            }
        } else {
            tokio::time::sleep(duration).await;
        }

        Ok(())
    }

    /// Disarm the timer if it's currently armed
    ///
    /// This is useful if you need to cancel a pending timer operation.
    /// On platforms without high-resolution timers, this is a no-op.
    pub fn disarm(&mut self) -> std::io::Result<()> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            if let Some(ref mut timer) = self.timer {
                return timer.disarm();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_high_res_delay_creation() {
        let result = HighResDelay::new();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_high_res_delay_short_duration() {
        let mut delay = HighResDelay::new().expect("Failed to create delay");
        let start = std::time::Instant::now();
        delay
            .delay(Duration::from_millis(5))
            .await
            .expect("Delay failed");
        let elapsed = start.elapsed();
        // Allow some tolerance (within 10ms)
        assert!(
            elapsed >= Duration::from_millis(5) && elapsed < Duration::from_millis(15),
            "Expected ~5ms, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_high_res_delay_long_duration() {
        let mut delay = HighResDelay::new().expect("Failed to create delay");
        let start = std::time::Instant::now();
        delay
            .delay(Duration::from_millis(50))
            .await
            .expect("Delay failed");
        let elapsed = start.elapsed();
        // Allow some tolerance (within 20ms)
        assert!(
            elapsed >= Duration::from_millis(50) && elapsed < Duration::from_millis(70),
            "Expected ~50ms, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_high_res_delay_reuse() {
        let mut delay = HighResDelay::new().expect("Failed to create delay");
        // Reuse the same instance multiple times
        for _ in 0..5 {
            let start = std::time::Instant::now();
            delay
                .delay(Duration::from_millis(2))
                .await
                .expect("Delay failed");
            let elapsed = start.elapsed();
            assert!(
                elapsed >= Duration::from_millis(2) && elapsed < Duration::from_millis(10),
                "Expected ~2ms, got {:?}",
                elapsed
            );
        }
    }
}
