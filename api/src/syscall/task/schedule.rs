use axerrno::{LinuxError, LinuxResult};
use axprocess::Pid;
use axtask::{AxCpuMask, set_affinity, with_task};

use crate::{
    ctypes::{SCHED_FIFO, CLOCK_MONOTONIC, CLOCK_REALTIME, timespec},
    ptr::{UserConstPtr, UserPtr, nullable},
    utils::time::TimeValueLike,
};

pub fn sys_sched_yield() -> LinuxResult<isize> {
    axtask::yield_now();
    Ok(0)
}

pub fn sys_sched_setaffinity(
    pid: Pid,
    cpuset_size: usize,
    mask: UserPtr<u8>,
) -> LinuxResult<isize> {
    with_task(pid.into(), |task| {
        let len = cpuset_size.min(axconfig::SMP.div_ceil(8));
        let mask_slice = mask.get_as_mut_slice(len)?;
        let mut cpu_mask = AxCpuMask::new();

        for i in 0..(len * 8).min(axconfig::SMP) {
            if mask_slice[i / 8] & (1 << (i % 8)) != 0 {
                cpu_mask.set(i, true);
            }
        }
        if set_affinity(task, cpu_mask) {
            Ok(0)
        } else {
            Err(LinuxError::EINVAL)
        }
    })
    .ok_or(LinuxError::ESRCH)?
}

pub fn sys_sched_getaffinity(
    pid: Pid,
    cpuset_size: usize,
    mask: UserPtr<u8>,
) -> LinuxResult<isize> {
    with_task(pid.into(), |task| {
        let len = cpuset_size.min(axconfig::SMP.div_ceil(8));
        let mask_slice = mask.get_as_mut_slice(len)?;
        let cpumask = task.cpumask();
        let cpumask_bytes = cpumask.as_bytes();

        for i in 0..len {
            if i < cpumask_bytes.len() {
                mask_slice[i] = cpumask_bytes[i];
            } else {
                mask_slice[i] = 0;
            }
        }
        Ok(len as isize)
    })
    .ok_or(LinuxError::ESRCH)?
}

pub fn sys_sched_getparam(_pid: Pid, _param: UserPtr<u8>) -> LinuxResult<isize> {
    warn!("sys_sched_getparam not implemented");
    Ok(0)
}

pub fn sys_sched_setparam(_pid: Pid, _param: UserPtr<u8>) -> LinuxResult<isize> {
    warn!("sys_sched_setparam not implemented");
    Ok(0)
}

pub fn sys_sched_setscheduler(
    _pid: Pid,
    policy: usize,
    _param: UserPtr<u8>,
) -> LinuxResult<isize> {
    if policy as u32 != SCHED_FIFO {
        error!("Not supported policy: {}", policy);
        return Err(LinuxError::EINVAL);
    }
    Ok(0)
}

pub fn sys_sched_getscheduler(_pid: Pid) -> LinuxResult<isize> {
    Ok(SCHED_FIFO as isize)
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

pub fn sys_clock_nanosleep(
    clock_id: usize,
    flags: usize,
    req: UserConstPtr<timespec>,
    rem: UserPtr<timespec>,
) -> LinuxResult<isize> {
    if clock_id as u32 != CLOCK_MONOTONIC && clock_id as u32 != CLOCK_REALTIME {
        warn!("sys_clock_nanosleep: invalid clock_id {}", clock_id);
        return Err(LinuxError::EINVAL);
    }

    if flags != 0 {
        warn!("sys_clock_nanosleep: invalid flags {}", flags);
    }

    sys_nanosleep(req, rem)
}