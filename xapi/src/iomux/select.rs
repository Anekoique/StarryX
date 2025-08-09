use alloc::vec::Vec;

use axerrno::{LinuxError, LinuxResult};

use xcore::task::with_uspace;
use xsignal::SignalSet;
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::{
    ctypes::{__FD_SETSIZE, __kernel_fd_set, FD_ISSET, FD_SET, FD_ZERO, timespec, timeval},
    time::{TimeValue, TimeValueLike},
};

use crate::iomux::{PollFd, convert_to_events, convert_to_rwe, poll};

fn do_select(
    nfds: u32,
    readfds: UserPtr<__kernel_fd_set>,
    writefds: UserPtr<__kernel_fd_set>,
    exceptfds: UserPtr<__kernel_fd_set>,
    timeout: Option<TimeValue>,
) -> LinuxResult<isize> {
    if nfds > __FD_SETSIZE {
        return Err(LinuxError::EINVAL);
    }

    let (mut readfds, mut writefds, mut exceptfds) = with_uspace(|uspace| -> LinuxResult<_> {
        let readfds = nullable!(uspace.raw_ptr(readfds))?;
        let writefds = nullable!(uspace.raw_ptr(writefds))?;
        let exceptfds = nullable!(uspace.raw_ptr(exceptfds))?;
        Ok((readfds, writefds, exceptfds))
    })?;

    let mut poll_fds = {
        let mut poll_fds = Vec::with_capacity(nfds as _);
        for fd in 0..nfds {
            let events = {
                unsafe {
                    let readable = readfds.as_deref().is_some_and(|fds| FD_ISSET(fd as _, fds));
                    let writable = writefds
                        .as_deref()
                        .is_some_and(|fds| FD_ISSET(fd as _, fds));
                    let except = exceptfds
                        .as_deref()
                        .is_some_and(|fds| FD_ISSET(fd as _, fds));
                    convert_to_events(readable, writable, except)
                }
            };

            if events.is_empty() {
                continue;
            }
            poll_fds.push(PollFd::new(fd as _, events));
        }
        poll_fds
    };

    unsafe {
        if let Some(readfds) = readfds.as_deref_mut() {
            FD_ZERO(readfds);
        }
        if let Some(writefds) = writefds.as_deref_mut() {
            FD_ZERO(writefds);
        }
        if let Some(exceptfds) = exceptfds.as_deref_mut() {
            FD_ZERO(exceptfds);
        }
    }

    if poll(&mut poll_fds, timeout)? == 0 {
        return Ok(0);
    }

    let mut res = 0;
    for poll_fd in &mut poll_fds {
        let fd = poll_fd.fd;
        let events = poll_fd.revents;
        let (readable, writeable, except) = convert_to_rwe(events);
        if let Some(readfds) = readfds.as_deref_mut()
            && readable
        {
            res += 1;
            unsafe { FD_SET(fd as _, readfds) };
        }
        if let Some(writefds) = writefds.as_deref_mut()
            && writeable
        {
            res += 1;
            unsafe { FD_SET(fd as _, writefds) };
        }
        if let Some(exceptfds) = exceptfds.as_deref_mut()
            && except
        {
            res += 1;
            unsafe { FD_SET(fd as _, exceptfds) };
        }
    }
    Ok(res)
}

pub fn sys_select(
    nfds: u32,
    readfds: UserPtr<__kernel_fd_set>,
    writefds: UserPtr<__kernel_fd_set>,
    exceptfds: UserPtr<__kernel_fd_set>,
    timeout: UserPtr<timeval>,
) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        do_select(
            nfds,
            readfds,
            writefds,
            exceptfds,
            nullable!(uspace.read(timeout))?
                .map(timeval::to_time_value)
                .transpose()?,
        )
    })
}

pub fn sys_pselect6(
    nfds: u32,
    readfds: UserPtr<__kernel_fd_set>,
    writefds: UserPtr<__kernel_fd_set>,
    exceptfds: UserPtr<__kernel_fd_set>,
    timeout: UserConstPtr<timespec>,
    _sigmask: UserConstPtr<SignalSet>,
) -> LinuxResult<isize> {
    // FIXME: process sigmask
    with_uspace(|uspace| {
        do_select(
            nfds,
            readfds,
            writefds,
            exceptfds,
            nullable!(uspace.read(timeout))?
                .map(timespec::to_time_value)
                .transpose()?,
        )
    })
}
