use alloc::sync::Arc;

use axerrno::{LinuxError, LinuxResult};
use axio::PollState;
use spin::Mutex;

use xcore::fs::{FileLike, Kstat, get_file_like};

use crate::{
    collections::BTreeMap,
    ctypes::{epoll_event, fs::IoEvents},
    time::{TimeValue, wall_time},
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

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PollFd {
    pub fd: i32,
    pub events: IoEvents,
    pub revents: IoEvents,
}

impl PollFd {
    pub fn new(fd: i32, events: IoEvents) -> Self {
        Self {
            fd,
            events,
            revents: IoEvents::empty(),
        }
    }
}

pub fn poll(fds: &mut [PollFd], timeout: Option<TimeValue>) -> LinuxResult<isize> {
    debug!("do_poll fds={:?} timeout={:?}", fds, timeout);

    let deadline = timeout.map(|t| wall_time() + t);

    loop {
        axnet::poll_interfaces();

        let mut res = 0;
        for fd in &mut *fds {
            let mut revents = IoEvents::empty();
            match get_file_like(fd.fd) {
                // FIXME: poll shouldn't return error
                Ok(f) => match f.poll() {
                    Ok(state) => {
                        if fd.events.contains(IoEvents::IN) && state.readable {
                            revents.insert(IoEvents::IN);
                        }
                        if fd.events.contains(IoEvents::OUT) && state.writable {
                            revents.insert(IoEvents::OUT);
                        }
                    }
                    Err(e) => {
                        warn!("poll fd={} error: {:?}", fd.fd, e);
                        revents.insert(IoEvents::ERR);
                    }
                },
                Err(_) => {
                    revents.insert(IoEvents::NVAL);
                }
            }
            fd.revents = revents;
            if !revents.is_empty() {
                res += 1;
            }
        }

        if res > 0 {
            return Ok(res);
        }

        if deadline.is_some_and(|d| wall_time() >= d) {
            return Ok(0);
        }

        axtask::yield_now();
    }
}

pub fn convert_to_events(readable: bool, writable: bool, except: bool) -> IoEvents {
    let mut events = IoEvents::empty();
    if readable {
        events |= IoEvents::IN;
    }
    if writable {
        events |= IoEvents::OUT;
    }
    if except {
        events |= IoEvents::PRI;
    }
    events
}

pub fn convert_to_rwe(events: IoEvents) -> (bool, bool, bool) {
    let readable = events.intersects(IoEvents::IN | IoEvents::HUP | IoEvents::ERR);
    let writable = events.intersects(IoEvents::OUT | IoEvents::ERR);
    let except = events.intersects(IoEvents::PRI);
    (readable, writable, except)
}
