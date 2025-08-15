use alloc::{sync::Arc, sync::Weak};
use core::{any::Any, ffi::c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axio::PollState;
use xprocess::{Pid, Thread};
use xutils::ctypes::fs::Kstat;

use crate::fs::{fd::get_file_like, file::FileLike};

/// Process file descriptor wrapper.
///
/// A pidfd represents a process and allows certain operations to be performed on it:
/// - Waiting for process termination
/// - Sending signals to the process
/// - Getting file descriptors from the process
pub struct PidFd {
    pid: Pid,
    thread: Weak<Thread>,
    nonblocking: bool,
}

impl PidFd {
    /// Create a new PidFd for the given process ID.
    pub fn new(pid: Pid, thread: Weak<Thread>, nonblocking: bool) -> Self {
        Self {
            pid,
            thread,
            nonblocking,
        }
    }

    /// Check if the process is still alive.
    pub fn is_alive(&self) -> bool {
        self.thread.upgrade().is_some()
    }

    /// Get the process ID this pidfd refers to.
    pub fn pid(&self) -> Pid {
        self.pid
    }

    pub fn get_thread(&self) -> LinuxResult<Arc<Thread>> {
        self.thread.upgrade().ok_or(LinuxError::ESRCH)
    }
}

impl FileLike for PidFd {
    fn read(&self, _buf: &mut [u8]) -> LinuxResult<usize> {
        Err(LinuxError::EINVAL)
    }

    fn write(&self, _buf: &[u8]) -> LinuxResult<usize> {
        Err(LinuxError::EINVAL)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        // pidfd is treated as a character device
        Ok(Kstat {
            mode: 0o020000 | 0o600, // S_IFCHR | rw-------
            ..Default::default()
        })
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        // pidfd is readable when the process has exited
        let readable = !self.is_alive();
        Ok(PollState {
            readable,
            writable: false,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) {}

    fn from_fd(fd: c_int, required: FileFlags, forbidden: FileFlags) -> LinuxResult<Arc<Self>> {
        get_file_like(fd)?
            .validate(required, forbidden)?
            .clone()
            .into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::EINVAL)
    }

    fn is_nonblocking(&self) -> bool {
        self.nonblocking
    }
}
