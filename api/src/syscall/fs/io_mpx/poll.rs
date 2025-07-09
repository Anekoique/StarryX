use axerrno::LinuxResult;
use axtask::current;
use axuspace::{UserConstPtr, UserPtr, UserSpace, nullable};
use starry_core::task::TaskExt;

use crate::{
    ctypes::{POLLERR, POLLIN, POLLNVAL, POLLOUT, pollfd, sigset_t, timespec},
    fs::get_file_like,
    time::{TimeValue, TimeValueLike, wall_time},
};

fn do_poll(fds: &mut [pollfd], timeout: Option<TimeValue>) -> LinuxResult<isize> {
    debug!("do_poll fds={:?} timeout={:?}", fds, timeout);

    let deadline = timeout.map(|t| wall_time() + t);

    loop {
        axnet::poll_interfaces();

        let mut res = 0;
        for fd in &mut *fds {
            let mut revents = 0;
            match get_file_like(fd.fd) {
                Ok(f) => match f.poll() {
                    Ok(state) => {
                        if (fd.events & POLLIN as i16) != 0 && state.readable {
                            revents |= POLLIN;
                        }
                        if (fd.events & POLLOUT as i16) != 0 && state.writable {
                            revents |= POLLOUT;
                        }
                    }
                    Err(e) => {
                        warn!("poll fd={} error: {:?}", fd.fd, e);
                        revents = POLLERR;
                    }
                },
                Err(_) => {
                    revents = POLLNVAL;
                }
            }
            fd.revents = revents as _;
            if revents != 0 {
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

/// Wait for events on file descriptors.
///
/// # Arguments
/// * `fds` - Array of file descriptors to monitor
/// * `nfds` - Number of file descriptors in the array
/// * `timeout` - Timeout in milliseconds (-1 for infinite)
pub fn sys_poll(fds: UserPtr<pollfd>, nfds: u32, timeout: i32) -> LinuxResult<isize> {
    let uspace = UserSpace::new(TaskExt::from_task(&current()).process_data());
    let fds = uspace.raw_slice(fds, nfds as usize)?;
    let timeout = if timeout < 0 {
        None
    } else {
        Some(TimeValue::from_millis(timeout as u64))
    };
    do_poll(fds, timeout)
}

/// Wait for events on file descriptors with signal mask.
///
/// # Arguments
/// * `fds` - Array of file descriptors to monitor
/// * `nfds` - Number of file descriptors in the array
/// * `timeout` - Timeout specification (NULL for infinite)
/// * `_sigmask` - Signal mask (currently unused)
pub fn sys_ppoll(
    fds: UserPtr<pollfd>,
    nfds: u32,
    timeout: UserConstPtr<timespec>,
    _sigmask: UserConstPtr<sigset_t>,
) -> LinuxResult<isize> {
    let uspace = UserSpace::new(TaskExt::from_task(&current()).process_data());
    let fds = uspace.raw_slice(fds, nfds as usize)?;
    let timeout = nullable!(uspace.read(timeout))?.map(timespec::to_time_value);
    // TODO: handle signal
    do_poll(fds, timeout)
}
