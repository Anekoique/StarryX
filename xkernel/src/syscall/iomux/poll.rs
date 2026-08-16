use xerrno::LinuxResult;

use crate::task::with_uspace;
use xuspace::{UserConstPtr, UserPtr, nullable};
use xutils::{
    ctypes::{sigset_t, timespec},
    time::{TimeValue, TimeValueLike},
};

use crate::syscall::iomux::{PollFd, poll};

/// Wait for events on file descriptors.
///
/// # Arguments
/// * `fds` - Array of file descriptors to monitor
/// * `nfds` - Number of file descriptors in the array
/// * `timeout` - Timeout in milliseconds (-1 for infinite)
pub fn sys_poll(fds: UserPtr<PollFd>, nfds: u32, timeout: i32) -> LinuxResult<isize> {
    let mut values = with_uspace(|uspace| uspace.read_slice(fds, nfds as usize))?;
    let timeout = (timeout >= 0).then_some(TimeValue::from_millis(timeout as u64));
    let result = poll(&mut values, timeout)?;
    with_uspace(|uspace| uspace.write_slice(fds, &values))?;
    Ok(result)
}

/// Wait for events on file descriptors with signal mask.
///
/// # Arguments
/// * `fds` - Array of file descriptors to monitor
/// * `nfds` - Number of file descriptors in the array
/// * `timeout` - Timeout specification (NULL for infinite)
/// * `_sigmask` - Signal mask (currently unused)
pub fn sys_ppoll(
    fds: UserPtr<PollFd>,
    nfds: u32,
    timeout: UserConstPtr<timespec>,
    sigmask: UserConstPtr<sigset_t>,
) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        let mut values = uspace.read_slice(fds, nfds as usize)?;
        let timeout = nullable!(uspace.read(timeout))?
            .map(timespec::to_time_value)
            .transpose()?;
        let _sigmask = nullable!(uspace.read(sigmask))?;
        // TODO: handle signal
        let result = poll(&mut values, timeout)?;
        uspace.write_slice(fds, &values)?;
        Ok(result)
    })
}
