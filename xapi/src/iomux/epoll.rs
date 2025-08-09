use alloc::{sync::Arc, vec::Vec};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;

use xcore::{
    fs::fd::{FD_TABLE, add_file_like, get_file_like},
    task::with_uspace,
};
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess};
use xutils::{
    ctypes::{
        EPOLL_CLOEXEC, EPOLL_CTL_ADD, EPOLL_CTL_DEL, EPOLL_CTL_MOD, EPOLLERR, EPOLLHUP, EPOLLIN,
        EPOLLOUT, epoll_event,
    },
    time::{TimeValue, wall_time},
};

use crate::{iomux::EpollInstance, task::check_fatal_signals};

pub fn sys_epoll_create(size: i32) -> LinuxResult<isize> {
    if size <= 0 {
        return Err(LinuxError::EINVAL);
    }
    sys_epoll_create1(size)
}

/// Create an epoll file descriptor.
///
/// # Arguments
/// * `flags` - Flags to control epoll creation (EPOLL_CLOEXEC)
pub fn sys_epoll_create1(flags: i32) -> LinuxResult<isize> {
    if flags != 0 && flags as u32 != EPOLL_CLOEXEC {
        return Err(LinuxError::EINVAL);
    }
    let epoll = Arc::new(EpollInstance::new());
    let fd = add_file_like(
        epoll,
        FileFlags::READ | FileFlags::WRITE,
        flags as u32 & EPOLL_CLOEXEC != 0,
    )?;
    Ok(fd as isize)
}

/// Control interface for an epoll file descriptor.
///
/// # Arguments
/// * `epfd` - Epoll file descriptor
/// * `op` - Operation to perform (EPOLL_CTL_ADD, EPOLL_CTL_DEL, EPOLL_CTL_MOD)
/// * `fd` - File descriptor to operate on
/// * `event` - Event configuration
pub fn sys_epoll_ctl(
    epfd: i32,
    op: i32,
    fd: i32,
    event: UserConstPtr<epoll_event>,
) -> LinuxResult<isize> {
    debug!(
        "epoll_ctl: epfd={}, op={}, fd={}, event={:?}",
        epfd, op, fd, event
    );
    if epfd == fd {
        return Err(LinuxError::EINVAL);
    }
    let epoll = get_file_like(epfd)?
        .into_any()
        .downcast::<EpollInstance>()
        .map_err(|_| LinuxError::EINVAL)?;
    let mut events = epoll.events.lock();
    with_uspace(|uspace| {
        match op as u32 {
            EPOLL_CTL_ADD => {
                if !FD_TABLE.is_assigned(fd as _) {
                    return Err(LinuxError::EBADF);
                }
                if events.contains_key(&fd) {
                    return Err(LinuxError::EEXIST);
                }
                // FIXME: Check for epoll nesting loops and depth limits
                let ev = uspace.read(event)?;
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
                let ev = uspace.read(event)?;
                events.insert(fd, ev);
            }
            _ => return Err(LinuxError::EINVAL),
        }
        Ok(0)
    })
}

/// Wait for events on an epoll file descriptor.
///
/// # Arguments
/// * `epfd` - Epoll file descriptor
/// * `events` - Buffer to store ready events
/// * `maxevents` - Maximum number of events to return
/// * `timeout` - Timeout in milliseconds (-1 for infinite)
pub fn sys_epoll_wait(
    epfd: i32,
    events: UserPtr<epoll_event>,
    maxevents: i32,
    timeout: i32,
) -> LinuxResult<isize> {
    if maxevents <= 0 {
        return Err(LinuxError::EINVAL);
    }
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
        // Poll network interfaces to update network state
        axnet::poll_interfaces();

        ready.clear();
        for (&fd, &ev) in epoll.events.lock().iter() {
            match get_file_like(fd) {
                Ok(f) => match f.poll() {
                    Ok(state) => {
                        let mut revents = 0;
                        if (ev.events & EPOLLIN) != 0 && state.readable {
                            revents |= EPOLLIN;
                        }
                        if (ev.events & EPOLLOUT) != 0 && state.writable {
                            revents |= EPOLLOUT;
                        }
                        // Always report error and hangup conditions
                        if !state.readable && !state.writable {
                            // Check if this might be a hangup condition
                            // This is a heuristic - in a real implementation,
                            // we'd need more detailed state information
                            if (ev.events & EPOLLHUP) != 0 {
                                revents |= EPOLLHUP;
                            }
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
                    Err(_) => {
                        // If poll fails, report error condition
                        if (ev.events & EPOLLERR) != 0 {
                            let mut event = ev;
                            event.events = EPOLLERR;
                            ready.push(event);
                            if ready.len() >= maxevents as usize {
                                break;
                            }
                        }
                    }
                },
                Err(_) => {
                    // File descriptor is invalid, report error
                    if (ev.events & EPOLLERR) != 0 {
                        let mut event = ev;
                        event.events = EPOLLERR;
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

        // Check for fatal signals before yielding
        check_fatal_signals();
        axtask::yield_now();
    }
    let n = ready.len().min(maxevents as usize);
    with_uspace(|uspace| uspace.write_slice(events, &ready[..n]))?;
    Ok(n as isize)
}
