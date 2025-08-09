use alloc::sync::Arc;
use core::any::Any;

use axerrno::{LinuxError, LinuxResult};
use axio::PollState;
use axsync::Mutex;

use xutils::ctypes::{EFD_SEMAPHORE, S_IFIFO, fs::Kstat};

use crate::{fs::file::FileLike, task::have_signals};

/// EventFd implementation for inter-process communication
pub struct EventFd {
    /// Counter value - the main state of the eventfd
    counter: Arc<Mutex<u64>>,
    /// Whether this is in semaphore mode (EFD_SEMAPHORE)
    semaphore: bool,
    /// Whether this fd is non-blocking
    nonblocking: bool,
}

impl EventFd {
    /// Create a new EventFd with initial value and flags
    pub fn new(initval: u64, flags: i32) -> Self {
        Self {
            counter: Arc::new(Mutex::new(initval)),
            semaphore: (flags as u32 & EFD_SEMAPHORE) != 0,
            nonblocking: (flags as u32 & xutils::ctypes::O_NONBLOCK) != 0,
        }
    }
}

impl FileLike for EventFd {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        if buf.len() < 8 {
            return Err(LinuxError::EINVAL);
        }

        loop {
            let mut counter = self.counter.lock();

            if *counter == 0 {
                if self.nonblocking {
                    return Err(LinuxError::EAGAIN);
                }
                // Drop the lock and wait
                drop(counter);
                if have_signals() {
                    return Err(LinuxError::EINTR);
                }
                axtask::yield_now();
                continue;
            }

            let value = if self.semaphore {
                // In semaphore mode, return 1 and decrement counter by 1
                *counter -= 1;
                1u64
            } else {
                // In counter mode, return current value and reset to 0
                let val = *counter;
                *counter = 0;
                val
            };

            // Write the value as little-endian u64
            buf[..8].copy_from_slice(&value.to_le_bytes());
            return Ok(8);
        }
    }

    fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        if buf.len() < 8 {
            return Err(LinuxError::EINVAL);
        }

        // Read the value as little-endian u64
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&buf[..8]);
        let value = u64::from_le_bytes(value_bytes);

        if value == 0 || value == 0xfffffffffffffffe {
            return Err(LinuxError::EINVAL);
        }

        loop {
            let mut counter = self.counter.lock();

            // Check for overflow
            if *counter > u64::MAX - value {
                if self.nonblocking {
                    return Err(LinuxError::EAGAIN);
                }
                // Drop the lock and wait
                drop(counter);
                if have_signals() {
                    return Err(LinuxError::EINTR);
                }
                axtask::yield_now();
                continue;
            }

            *counter += value;
            return Ok(8);
        }
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        Ok(Kstat {
            mode: S_IFIFO | 0o600u32,
            ..Default::default()
        })
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        let counter = self.counter.lock();

        Ok(PollState {
            // Readable when counter > 0
            readable: *counter > 0,
            // Writable when counter < u64::MAX - 1 (to avoid overflow)
            writable: *counter < u64::MAX - 1,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) {
        // EventFd nonblocking behavior is set at creation time
        // This is a no-op for compatibility
    }

    fn is_nonblocking(&self) -> bool {
        self.nonblocking
    }
}
