use crate::{
    ctypes::{EPOLL_CTL_ADD, EPOLL_CTL_DEL, EPOLL_CTL_MOD, EPOLLIN, EPOLLOUT, epoll_event},
    fs::{FileLike, add_file_like, get_file_like},
    ptr::{UserConstPtr, UserPtr},
    time::{TimeValue, wall_time},
};
use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use axerrno::{LinuxError, LinuxResult};
use spin::Mutex;

struct EpollInstance {
    // fd -> epoll_event
    events: Mutex<BTreeMap<i32, epoll_event>>,
}

impl EpollInstance {
    fn new() -> Self {
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
    fn stat(&self) -> LinuxResult<crate::fs::Kstat> {
        Err(LinuxError::EINVAL)
    }
    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }
    fn poll(&self) -> LinuxResult<axio::PollState> {
        Ok(axio::PollState {
            readable: false,
            writable: false,
        })
    }
    fn set_nonblocking(&self, _nonblocking: bool) -> LinuxResult {
        Ok(())
    }
}

pub fn sys_epoll_create1(_flags: u32) -> LinuxResult<isize> {
    let epoll = Arc::new(EpollInstance::new());
    let fd = add_file_like(epoll)?;
    Ok(fd as isize)
}

pub fn sys_epoll_ctl(
    epfd: i32,
    op: i32,
    fd: i32,
    event: UserConstPtr<epoll_event>,
) -> LinuxResult<isize> {
    let epoll = get_file_like(epfd)?;
    let epoll = epoll
        .into_any()
        .downcast::<EpollInstance>()
        .map_err(|_| LinuxError::EINVAL)?;
    let mut events = epoll.events.lock();
    match op as u32 {
        EPOLL_CTL_ADD => {
            if events.contains_key(&fd) {
                return Err(LinuxError::EEXIST);
            }
            let ev = *event.get_as_ref()?;
            events.insert(fd, ev);
        }
        EPOLL_CTL_DEL => {
            if events.remove(&fd).is_none() {
                return Err(LinuxError::ENOENT);
            }
        }
        EPOLL_CTL_MOD => {
            if !events.contains_key(&fd) {
                return Err(LinuxError::ENOENT);
            }
            let ev = *event.get_as_ref()?;
            events.insert(fd, ev);
        }
        _ => return Err(LinuxError::EINVAL),
    }
    Ok(0)
}

pub fn sys_epoll_wait(
    epfd: i32,
    events: UserPtr<epoll_event>,
    maxevents: i32,
    timeout: i32,
) -> LinuxResult<isize> {
    let epoll = get_file_like(epfd)?;
    let epoll = epoll
        .into_any()
        .downcast::<EpollInstance>()
        .map_err(|_| LinuxError::EINVAL)?;
    let mut ready = Vec::new();
    let deadline = if timeout < 0 {
        None
    } else {
        Some(wall_time() + TimeValue::from_millis(timeout as u64))
    };

    loop {
        ready.clear();
        for (&fd, &ev) in epoll.events.lock().iter() {
            if let Ok(f) = get_file_like(fd) {
                if let Ok(state) = f.poll() {
                    let mut revents = 0;
                    if (ev.events & EPOLLIN) != 0 && state.readable {
                        revents |= EPOLLIN;
                    }
                    if (ev.events & EPOLLOUT) != 0 && state.writable {
                        revents |= EPOLLOUT;
                    }
                    if revents != 0 {
                        let mut event = ev;
                        event.events = revents;
                        ready.push(event);
                        if ready.len() >= maxevents as usize {
                            break;
                        }
                    }
                }
            }
        }
        if !ready.is_empty() || timeout == 0 {
            break;
        }
        if let Some(d) = deadline {
            if wall_time() >= d {
                break;
            }
        }
        axtask::yield_now();
    }
    let n = ready.len().min(maxevents as usize);
    events.get_as_mut_slice(n)?.copy_from_slice(&ready[..n]);
    Ok(n as isize)
}
