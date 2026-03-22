use alloc::{sync::Arc, vec::Vec};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;

use xcore::{
    fs::fd::{EpollEventInfo, EpollInstance, FD_TABLE, add_file_like, get_file_like},
    task::with_uspace,
};
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::{
    ctypes::{
        EPOLL_CLOEXEC, EPOLL_CTL_ADD, EPOLL_CTL_DEL, EPOLL_CTL_MOD, EPOLLERR, EPOLLET, EPOLLHUP,
        EPOLLIN, EPOLLONESHOT, EPOLLOUT, epoll_event, fs::IoEvents, sigset_t, timespec,
    },
    time::{TimeValue, TimeValueLike},
};

use crate::iomux::{PollFd, poll};

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

/// Check if adding an epoll fd would create a loop
fn check_epoll_loop(epfd: i32, target_fd: i32, depth: usize) -> LinuxResult<()> {
    const MAX_EPOLL_DEPTH: usize = 5;

    if depth > MAX_EPOLL_DEPTH {
        return Err(LinuxError::EINVAL);
    }

    if epfd == target_fd {
        return Err(LinuxError::EINVAL);
    }

    // Check if target_fd is an epoll instance
    if let Ok(target_file) = get_file_like(target_fd)
        && let Ok(target_epoll) = target_file.into_any().downcast::<EpollInstance>()
    {
        let target_events = target_epoll.events.lock();
        // Check if any of the target's monitored fds would create a loop
        for &monitored_fd in target_events.keys() {
            check_epoll_loop(epfd, monitored_fd, depth + 1)?;
        }
    }

    Ok(())
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
                // Check for epoll nesting loops
                check_epoll_loop(epfd, fd, 0)?;

                let ev = uspace.read(event)?;
                let info = EpollEventInfo {
                    event: ev,
                    last_state: None,
                };
                events.insert(fd, info);
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
                let info = EpollEventInfo {
                    event: ev,
                    last_state: events.get(&fd).and_then(|info| info.last_state),
                };
                events.insert(fd, info);
            }
            _ => return Err(LinuxError::EINVAL),
        }
        Ok(0)
    })
}

/// Convert epoll events to IoEvents for poll
fn epoll_to_ioevents(epoll_events: u32) -> IoEvents {
    let mut events = IoEvents::empty();
    if (epoll_events & EPOLLIN) != 0 {
        events |= IoEvents::IN;
    }
    if (epoll_events & EPOLLOUT) != 0 {
        events |= IoEvents::OUT;
    }
    if (epoll_events & EPOLLERR) != 0 {
        events |= IoEvents::ERR;
    }
    if (epoll_events & EPOLLHUP) != 0 {
        events |= IoEvents::HUP;
    }
    events
}

/// Convert IoEvents back to epoll events
fn ioevents_to_epoll(io_events: IoEvents) -> u32 {
    let mut events = 0;
    if io_events.contains(IoEvents::IN) {
        events |= EPOLLIN;
    }
    if io_events.contains(IoEvents::OUT) {
        events |= EPOLLOUT;
    }
    if io_events.contains(IoEvents::ERR) {
        events |= EPOLLERR;
    }
    if io_events.contains(IoEvents::HUP) {
        events |= EPOLLHUP;
    }
    events
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

    let epoll = get_file_like(epfd)?
        .into_any()
        .downcast::<EpollInstance>()
        .map_err(|_| LinuxError::EINVAL)?;

    let timeout_val = if timeout < 0 {
        None
    } else {
        Some(TimeValue::from_millis(timeout as u64))
    };

    // Convert epoll events to PollFd array
    let mut poll_fds = Vec::new();
    let mut fd_to_info = Vec::new();
    {
        let epoll_events = epoll.events.lock();
        for (&fd, info) in epoll_events.iter() {
            let io_events = epoll_to_ioevents(info.event.events);
            poll_fds.push(PollFd::new(fd, io_events));
            fd_to_info.push((fd, info.clone()));
        }
    }

    // Use the common poll implementation
    let ready_count = poll(&mut poll_fds, timeout_val)?;

    if ready_count == 0 {
        return Ok(0);
    }

    // Convert results back to epoll format
    let mut ready_events = Vec::new();
    let mut epoll_events = epoll.events.lock();

    for (poll_fd, (fd, info)) in poll_fds.iter().zip(fd_to_info.iter()) {
        if poll_fd.revents.is_empty() {
            continue;
        }

        let current_revents = ioevents_to_epoll(poll_fd.revents);
        let is_edge_triggered = (info.event.events & EPOLLET) != 0;
        let is_oneshot = (info.event.events & EPOLLONESHOT) != 0;

        // For edge-triggered mode, only report events if state changed
        let should_report = if is_edge_triggered {
            // Compare with last known state
            let state_changed = match &info.last_state {
                Some(last) => {
                    let last_readable = last.readable;
                    let last_writable = last.writable;
                    let current_readable = poll_fd.revents.contains(IoEvents::IN);
                    let current_writable = poll_fd.revents.contains(IoEvents::OUT);

                    (current_readable && !last_readable)
                        || (current_writable && !last_writable)
                        || poll_fd.revents.contains(IoEvents::ERR | IoEvents::HUP)
                }
                None => true, // First time, always report
            };

            if state_changed {
                // Update last state for edge-triggered
                let mut updated_info = info.clone();
                updated_info.last_state = Some(axio::PollState {
                    readable: poll_fd.revents.contains(IoEvents::IN),
                    writable: poll_fd.revents.contains(IoEvents::OUT),
                });
                epoll_events.insert(*fd, updated_info);
            }

            state_changed
        } else {
            // Level-triggered: always report
            true
        };

        if should_report {
            let mut event = info.event;
            event.events = current_revents;
            ready_events.push(event);

            // Remove fd for EPOLLONESHOT
            if is_oneshot {
                epoll_events.remove(fd);
            }

            if ready_events.len() >= maxevents as usize {
                break;
            }
        }
    }

    let n = ready_events.len();
    with_uspace(|uspace| uspace.write_slice(events, &ready_events[..n]))?;
    Ok(n as isize)
}

pub fn sys_epoll_pwait2(
    epfd: i32,
    events: UserPtr<epoll_event>,
    maxevents: i32,
    timeout: UserConstPtr<timespec>,
    sigmask: UserConstPtr<sigset_t>,
) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        let timeout = nullable!(uspace.read(timeout))?
            .map(timespec::to_time_value)
            .transpose()?;
        let _sigmask = nullable!(uspace.read(sigmask))?;
        sys_epoll_wait(
            epfd,
            events,
            maxevents,
            timeout.unwrap_or(TimeValue::from_millis(0)).as_millis() as i32,
        )
    })
}
