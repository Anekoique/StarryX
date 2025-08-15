use alloc::sync::Arc;

use axerrno::{LinuxError, LinuxResult};
use axio::PollState;
use spin::Mutex;

use xutils::{
    collections::btreemap::BTreeMap,
    ctypes::{epoll_event, fs::Kstat},
};

use crate::fs::file::FileLike;

#[derive(Clone)]
pub struct EpollEventInfo {
    pub event: epoll_event,
    pub last_state: Option<PollState>,
}

pub struct EpollInstance {
    // fd -> epoll event info
    pub events: Mutex<BTreeMap<i32, EpollEventInfo>>,
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
    fn set_nonblocking(&self, _nonblocking: bool) {}
}
