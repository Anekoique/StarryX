use axerrno::{LinuxError, LinuxResult};
use axprocess::Pid;

use crate::{
    ctypes::timespec,
    ptr::{UserConstPtr, UserPtr, nullable},
    utils::time::TimeValueLike,
};

pub fn sys_sched_yield() -> LinuxResult<isize> {
    axtask::yield_now();
    Ok(0)
}

pub fn sys_sched_setaffinity(
    _pid: Pid,
    _cpuset_size: usize,
    _mask: UserPtr<usize>,
) -> LinuxResult<isize> {
    warn!("sys_sched_setaffinity not implemented");
    Ok(0)
}

pub fn sys_sched_getaffinity(_pid: Pid, _cpuset_size: usize) -> LinuxResult<isize> {
    warn!("sys_sched_getaffinity not implemented");
    Ok(0)
}

pub fn sys_sched_setscheduler(_pid: Pid, _sched: usize, _param_size: usize) -> LinuxResult<isize> {
    warn!("sys_sched_setscheduler not implemented");
    Ok(0)
}

pub fn sys_sched_getscheduler(_pid: Pid) -> LinuxResult<isize> {
    warn!("sys_sched_getscheduler not implemented");
    Ok(0)
}

pub fn sys_sched_getscheduler_max(
    _pid: Pid,
    _sched: usize,
    _param_size: usize,
) -> LinuxResult<isize> {
    warn!("sys_sched_getscheduler_max not implemented");
    Ok(0)
}

pub fn sys_sched_getscheduler_min(
    _pid: Pid,
    _sched: usize,
    _param_size: usize,
) -> LinuxResult<isize> {
    warn!("sys_sched_getscheduler_min not implemented");
    Ok(0)
}

/// Sleep some nanoseconds
///
/// TODO: should be woken by signals, and set errno
pub fn sys_nanosleep(req: UserConstPtr<timespec>, rem: UserPtr<timespec>) -> LinuxResult<isize> {
    let req = req.get_as_ref()?;

    if req.tv_nsec < 0 || req.tv_nsec > 999_999_999 || req.tv_sec < 0 {
        return Err(LinuxError::EINVAL);
    }

    let dur = timespec::to_time_value(*req);
    debug!("sys_nanosleep <= {:?}", dur);

    let now = axhal::time::monotonic_time();

    axtask::sleep(dur);

    let after = axhal::time::monotonic_time();
    let actual = after - now;

    if let Some(diff) = dur.checked_sub(actual) {
        if let Some(rem) = nullable!(rem.get_as_mut())? {
            *rem = timespec::from_time_value(diff);
        }
        Err(LinuxError::EINTR)
    } else {
        Ok(0)
    }
}
