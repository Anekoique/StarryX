mod epoll;
mod poll;
mod select;

pub use self::{epoll::*, poll::*, select::*};

use axerrno::LinuxResult;

use xcore::fs::fd::get_file_like;
use xutils::{
    ctypes::fs::IoEvents,
    time::{TimeValue, wall_time},
};

use crate::task::check_fatal_signals;

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

    let deadline = timeout.map(|t| wall_time().saturating_add(t));

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

        check_fatal_signals();
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
