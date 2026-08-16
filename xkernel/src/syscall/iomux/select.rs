use alloc::vec::Vec;

use xerrno::{LinuxError, LinuxResult};

use crate::task::with_uspace;
use xsignal::SignalSet;
use xuspace::{UserConstPtr, UserPtr, nullable};
use xutils::{
    ctypes::{__FD_SETSIZE, __kernel_fd_set, FD_ISSET, FD_SET, FD_ZERO, timespec, timeval},
    time::{TimeValue, TimeValueLike},
};

use crate::syscall::iomux::{PollFd, convert_to_events, convert_to_rwe, poll};

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

    let read_addr = readfds.address().as_usize();
    let write_addr = writefds.address().as_usize();
    let except_addr = exceptfds.address().as_usize();
    let (mut read_values, mut write_values, mut except_values) =
        with_uspace(|uspace| -> LinuxResult<_> {
            let read = nullable!(uspace.read(UserPtr::<__kernel_fd_set>::from(read_addr)))?;
            let write = nullable!(uspace.read(UserPtr::<__kernel_fd_set>::from(write_addr)))?;
            let except = nullable!(uspace.read(UserPtr::<__kernel_fd_set>::from(except_addr)))?;
            Ok((read, write, except))
        })?;

    let mut poll_fds = {
        let mut poll_fds = Vec::with_capacity(nfds as _);
        for fd in 0..nfds {
            let events = {
                unsafe {
                    let readable = read_values
                        .as_ref()
                        .is_some_and(|fds| FD_ISSET(fd as _, fds));
                    let writable = write_values
                        .as_ref()
                        .is_some_and(|fds| FD_ISSET(fd as _, fds));
                    let except = except_values
                        .as_ref()
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
        if let Some(readfds) = read_values.as_mut() {
            FD_ZERO(readfds);
        }
        if let Some(writefds) = write_values.as_mut() {
            FD_ZERO(writefds);
        }
        if let Some(exceptfds) = except_values.as_mut() {
            FD_ZERO(exceptfds);
        }
    }

    let mut res = 0;
    if poll(&mut poll_fds, timeout)? != 0 {
        for poll_fd in &mut poll_fds {
            let fd = poll_fd.fd;
            let events = poll_fd.revents;
            let (readable, writeable, except) = convert_to_rwe(events);
            if let Some(readfds) = read_values.as_mut()
                && readable
            {
                res += 1;
                unsafe { FD_SET(fd as _, readfds) };
            }
            if let Some(writefds) = write_values.as_mut()
                && writeable
            {
                res += 1;
                unsafe { FD_SET(fd as _, writefds) };
            }
            if let Some(exceptfds) = except_values.as_mut()
                && except
            {
                res += 1;
                unsafe { FD_SET(fd as _, exceptfds) };
            }
        }
    }
    with_uspace(|uspace| {
        if let Some(value) = read_values {
            uspace.write(UserPtr::from(read_addr), value)?;
        }
        if let Some(value) = write_values {
            uspace.write(UserPtr::from(write_addr), value)?;
        }
        if let Some(value) = except_values {
            uspace.write(UserPtr::from(except_addr), value)?;
        }
        Ok::<(), LinuxError>(())
    })?;
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
