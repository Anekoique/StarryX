use axerrno::LinuxResult;
use axio::PollState;
use axsignal::SignalSet;

use crate::{
    ctypes::{timespec, timeval},
    fs::FD_TABLE,
    ptr::{UserConstPtr, UserPtr, nullable},
    time::{TimeValue, timespec_to_timevalue, timeval_to_timevalue, wall_time},
};

fn do_select(
    nfds: u32,
    read_fds: UserPtr<u8>,
    write_fds: UserPtr<u8>,
    except_fds: UserPtr<u8>,
    timeout: Option<TimeValue>,
) -> LinuxResult<isize> {
    let num_bytes = nfds.div_ceil(8) as usize;
    let mut read_fds = nullable!(read_fds.get_as_mut_slice(num_bytes))?;
    let mut write_fds = nullable!(write_fds.get_as_mut_slice(num_bytes))?;
    let mut except_fds = nullable!(except_fds.get_as_mut_slice(num_bytes))?;
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
        let fd_table = FD_TABLE.write();
        let mut num = 0;
        for fd in fd_table.ids() {
            if fd >= nfds as usize {
                break;
            }
            if f(fd_table.get(fd).unwrap().poll()?) {
                let byte_index = fd / 8;
                let bit_index = fd % 8;
                if byte_index < fds.len() {
                    fds[byte_index] |= 1 << bit_index;
                    num += 1;
                } else {
                    warn!("select: fd {} out of bounds for fd_set size {}", fd, fds.len());
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
        let readable_num = fill(nfds, &mut read_fds, |state| state.readable)?;
        let writable_num = fill(nfds, &mut write_fds, |state| state.writable)?;
        let except_num = fill(nfds, &mut except_fds, |_state| false /* TODO */)?;
        let num = readable_num + writable_num + except_num;
        
        debug!("select poll result: readable={}, writable={}, except={}, total={}",
               readable_num, writable_num, except_num, num);
        
        if num > 0 {
            return Ok(num as isize);
        }

        axtask::yield_now();
        if deadline.is_some_and(|d| wall_time() >= d) {
            return Ok(0);
        }
    }
}

pub fn sys_select(
    nfds: u32,
    read_fds: UserPtr<u8>,
    write_fds: UserPtr<u8>,
    except_fds: UserPtr<u8>,
    timeout: UserConstPtr<timeval>,
) -> LinuxResult<isize> {
    do_select(
        nfds,
        read_fds,
        write_fds,
        except_fds,
        nullable!(timeout.get_as_ref())?.map(|it| timeval_to_timevalue(*it)),
    )
}

pub fn sys_pselect6(
    nfds: u32,
    read_fds: UserPtr<u8>,
    write_fds: UserPtr<u8>,
    except_fds: UserPtr<u8>,
    timeout: UserConstPtr<timespec>,
    _sigmask: UserConstPtr<SignalSet>,
) -> LinuxResult<isize> {
    do_select(
        nfds,
        read_fds,
        write_fds,
        except_fds,
        nullable!(timeout.get_as_ref())?.map(|it| timespec_to_timevalue(*it)),
    )
}
