use alloc::sync::Arc;
use axerrno::{LinuxError, LinuxResult};
use axio::PollState;
use spin::Mutex;

use crate::{
    collections::BTreeMap,
    ctypes::epoll_event,
    fs::{FileLike, Kstat},
};

pub struct EpollInstance {
    // fd -> epoll_event
    pub events: Mutex<BTreeMap<i32, epoll_event>>,
}

impl EpollInstance {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(BTreeMap::new()),
        }
    }
}

impl FileLike for EpollInstance {
    fn read(&self, _buf: &mut [u8]) -> LinuxResult<usize> {
        Err(LinuxError::EINVAL)
    }
    fn write(&self, _buf: &[u8]) -> LinuxResult<usize> {
        Err(LinuxError::EINVAL)
    }
    fn stat(&self) -> LinuxResult<Kstat> {
        Err(LinuxError::EINVAL)
    }
    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }
    fn poll(&self) -> LinuxResult<PollState> {
        Ok(PollState {
            readable: false,
            writable: false,
        })
    }
    fn set_nonblocking(&self, _nonblocking: bool) -> LinuxResult {
        Ok(())
    }
}
