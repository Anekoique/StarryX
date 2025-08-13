use axerrno::LinuxResult;

use xcore::task::with_uspace;
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::{
    ctypes::{sigset_t, timespec},
    time::{TimeValue, TimeValueLike},
};

use crate::iomux::{PollFd, poll};

/// Wait for events on file descriptors.
///
/// # Arguments
/// * `fds` - Array of file descriptors to monitor
/// * `nfds` - Number of file descriptors in the array
/// * `timeout` - Timeout in milliseconds (-1 for infinite)
pub fn sys_poll(fds: UserPtr<PollFd>, nfds: u32, timeout: i32) -> LinuxResult<isize> {
    let fds = with_uspace(|uspace| uspace.raw_slice(fds, nfds as usize))?;
    let timeout = (timeout >= 0).then_some(TimeValue::from_millis(timeout as u64));
    poll(fds, timeout)
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
        let fds = uspace.raw_slice(fds, nfds as usize)?;
        let timeout = nullable!(uspace.read(timeout))?
            .map(timespec::to_time_value)
            .transpose()?;
        let _sigmask = nullable!(uspace.read(sigmask))?;
        // TODO: handle signal
        poll(fds, timeout)
    })
}
