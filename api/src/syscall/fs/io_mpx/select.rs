use axerrno::LinuxResult;
use axio::PollState;
use axsignal::SignalSet;
use axtask::current;
use axuspace::{UserConstPtr, UserPtr, UserSpace, nullable};
use starry_core::task::TaskExt;

use crate::{
    ctypes::{timespec, timeval},
    fs::FD_TABLE,
    time::{TimeValue, TimeValueLike, wall_time},
};

fn do_select(
    nfds: u32,
    read_fds: UserPtr<u8>,
    write_fds: UserPtr<u8>,
    except_fds: UserPtr<u8>,
    timeout: Option<TimeValue>,
) -> LinuxResult<isize> {
    let uspace = UserSpace::new(TaskExt::from_task(&current()).process_data());
    let num_words = nfds.div_ceil(8) as usize;
    let mut read_fds = nullable!(uspace.raw_slice(read_fds, num_words))?;
    let mut write_fds = nullable!(uspace.raw_slice(write_fds, num_words))?;
    let mut except_fds = nullable!(uspace.raw_slice(except_fds, num_words))?;
    if let Some(fds) = read_fds.as_mut() {
        fds.fill(0);
    }
    if let Some(fds) = write_fds.as_mut() {
        fds.fill(0);
    }
    if let Some(fds) = except_fds.as_mut() {
        fds.fill(0);
    }

    fn fill(
        nfds: u32,
        fds: &mut Option<&'static mut [u8]>,
        f: impl Fn(PollState) -> bool,
    ) -> LinuxResult<usize> {
        let Some(fds) = fds else { return Ok(0) };
        let mut num = 0;
        for fd in FD_TABLE.ids() {
            if fd >= nfds as usize {
                break;
            }
            if let Some(file) = FD_TABLE.get(fd) {
                if f(file.poll()?) {
                    debug!("select: fd: {} is ready, nfds: {}", fd, nfds);
                    fds[fd / 8] |= 1 << (fd % 8);
                    num += 1;
                }
            }
        }
        Ok(num)
    }
    let deadline = timeout.map(|t| wall_time() + t);

    debug!(
        "select timeout: {:?} {} {} {} {}",
        timeout,
        nfds,
        read_fds.is_some(),
        write_fds.is_some(),
        except_fds.is_some()
    );

    loop {
        axtask::yield_now();
        let num = fill(nfds, &mut read_fds, |state| state.readable)?
            + fill(nfds, &mut write_fds, |state| state.writable)?
            + fill(nfds, &mut except_fds, |_state| false /* TODO */)?;
        if num > 0 {
            return Ok(num as isize);
        }

        if deadline.is_some_and(|d| wall_time() >= d) {
            return Ok(0);
        }
    }
}

/// Monitor multiple file descriptors for I/O events.
///
/// # Arguments
/// * `nfds` - Number of file descriptors to monitor
/// * `read_fds` - Bit mask of file descriptors to check for readability
/// * `write_fds` - Bit mask of file descriptors to check for writability
/// * `except_fds` - Bit mask of file descriptors to check for exceptions
/// * `timeout` - Timeout value (NULL for infinite)
pub fn sys_select(
    nfds: u32,
    read_fds: UserPtr<u8>,
    write_fds: UserPtr<u8>,
    except_fds: UserPtr<u8>,
    timeout: UserConstPtr<timeval>,
) -> LinuxResult<isize> {
    let uspace = UserSpace::new(TaskExt::from_task(&current()).process_data());
    do_select(
        nfds,
        read_fds,
        write_fds,
        except_fds,
        nullable!(uspace.read(timeout))?.map(timeval::to_time_value),
    )
}

/// Monitor multiple file descriptors for I/O events with signal mask.
///
/// # Arguments
/// * `nfds` - Number of file descriptors to monitor
/// * `read_fds` - Bit mask of file descriptors to check for readability
/// * `write_fds` - Bit mask of file descriptors to check for writability
/// * `except_fds` - Bit mask of file descriptors to check for exceptions
/// * `timeout` - Timeout specification (NULL for infinite)
/// * `_sigmask` - Signal mask (currently unused)
pub fn sys_pselect6(
    nfds: u32,
    read_fds: UserPtr<u8>,
    write_fds: UserPtr<u8>,
    except_fds: UserPtr<u8>,
    timeout: UserConstPtr<timespec>,
    _sigmask: UserConstPtr<SignalSet>,
) -> LinuxResult<isize> {
    let uspace = UserSpace::new(TaskExt::from_task(&current()).process_data());
    do_select(
        nfds,
        read_fds,
        write_fds,
        except_fds,
        nullable!(uspace.read(timeout))?.map(timespec::to_time_value),
    )
}
