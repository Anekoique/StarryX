use alloc::sync::Arc;
use core::any::Any;

use axerrno::LinuxResult;
use axio::PollState;

use xutils::ctypes::fs::Kstat;

use crate::fs::file::FileLike;

pub struct DummyFd;

impl FileLike for DummyFd {
    fn read(&self, _buf: &mut [u8]) -> LinuxResult<usize> {
        Ok(0)
    }

    fn write(&self, _buf: &[u8]) -> LinuxResult<usize> {
        Ok(0)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        Ok(Kstat {
            ..Default::default()
        })
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        Ok(PollState {
            readable: true,
            writable: true,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) {}
}
