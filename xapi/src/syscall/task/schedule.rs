use axerrno::{LinuxError, LinuxResult};
use axprocess::Pid;
use axtask::{AxCpuMask, current, set_affinity, with_task};
use axuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xcore::task::{XTaskExt, with_uspace};

use crate::{
    ctypes::{CLOCK_MONOTONIC, CLOCK_REALTIME, SCHED_FIFO, timespec},
    utils::time::TimeValueLike,
};

/// Yield the processor to other threads.
///
/// # Arguments
/// None
pub fn sys_sched_yield() -> LinuxResult<isize> {
    axtask::yield_now();
    Ok(0)
}

/// Set CPU affinity mask for a thread.
///
/// # Arguments
/// * `pid` - Thread ID (0 for calling thread)
/// * `cpuset_size` - Size of the CPU mask
/// * `mask` - CPU affinity mask
pub fn sys_sched_setaffinity(
    pid: Pid,
    cpuset_size: usize,
    mask: UserPtr<u8>,
) -> LinuxResult<isize> {
    with_task(pid.into(), |task| {
        let len = cpuset_size.min(axconfig::SMP.div_ceil(8));
        let mask_slice = with_uspace(|uspace| uspace.raw_slice(mask, len))?;
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

/// Get CPU affinity mask for a thread.
///
/// # Arguments
/// * `pid` - Thread ID (0 for calling thread)
/// * `cpuset_size` - Size of the CPU mask buffer
/// * `mask` - Buffer to store CPU affinity mask
pub fn sys_sched_getaffinity(
    pid: Pid,
    cpuset_size: usize,
    mask: UserPtr<u8>,
) -> LinuxResult<isize> {
    with_task(pid.into(), |task| {
        let len = cpuset_size.min(axconfig::SMP.div_ceil(8));
        let mask_slice = with_uspace(|uspace| uspace.raw_slice(mask, len))?;
        let cpumask = task.cpumask();
        let cpumask_bytes = cpumask.as_bytes();

        for i in 0..len {
            if i < cpumask_bytes.len() {
                mask_slice[i] = cpumask_bytes[i];
            } else {
                mask_slice[i] = 0;
            }
        }
        Ok(0)
    })
    .ok_or(LinuxError::ESRCH)?
}

/// Get scheduling parameters for a thread.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
/// * `_param` - Buffer to store scheduling parameters (currently unused)
pub fn sys_sched_getparam(_pid: Pid, _param: UserPtr<u8>) -> LinuxResult<isize> {
    warn!("sys_sched_getparam not implemented");
    Ok(0)
}

/// Set scheduling parameters for a thread.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
/// * `_param` - New scheduling parameters (currently unused)
pub fn sys_sched_setparam(_pid: Pid, _param: UserPtr<u8>) -> LinuxResult<isize> {
    warn!("sys_sched_setparam not implemented");
    Ok(0)
}

/// Set scheduling algorithm and parameters for a thread.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
/// * `policy` - Scheduling policy
/// * `_param` - Scheduling parameters (currently unused)
pub fn sys_sched_setscheduler(_pid: Pid, policy: usize, _param: UserPtr<u8>) -> LinuxResult<isize> {
    if policy as u32 != SCHED_FIFO {
        error!("Not supported policy: {}", policy);
        return Err(LinuxError::EINVAL);
    }
    Ok(0)
}

/// Get scheduling algorithm for a thread.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
pub fn sys_sched_getscheduler(_pid: Pid) -> LinuxResult<isize> {
    Ok(SCHED_FIFO as isize)
}

/// Get maximum priority value for a scheduling algorithm.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
/// * `_sched` - Scheduling algorithm (currently unused)
/// * `_param_size` - Parameter size (currently unused)
pub fn sys_sched_getscheduler_max(
    _pid: Pid,
    _sched: usize,
    _param_size: usize,
) -> LinuxResult<isize> {
    warn!("sys_sched_getscheduler_max not implemented");
    Ok(0)
}

/// Get minimum priority value for a scheduling algorithm.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
/// * `_sched` - Scheduling algorithm (currently unused)
/// * `_param_size` - Parameter size (currently unused)
pub fn sys_sched_getscheduler_min(
    _pid: Pid,
    _sched: usize,
    _param_size: usize,
) -> LinuxResult<isize> {
    warn!("sys_sched_getscheduler_min not implemented");
    Ok(0)
}

/// Sleep for a specified time.
///
/// # Arguments
/// * `req` - Time to sleep
/// * `rem` - Remaining time if interrupted (NULL if not needed)
pub fn sys_nanosleep(req: UserConstPtr<timespec>, rem: UserPtr<timespec>) -> LinuxResult<isize> {
    let uspace = XTaskExt::from_task(&current()).xprocess_ref().uspace();
    let req = uspace.read(req)?;

    if req.tv_nsec < 0 || req.tv_nsec > 999_999_999 || req.tv_sec < 0 {
        return Err(LinuxError::EINVAL);
    }

    let dur = timespec::to_time_value(req);
    debug!("sys_nanosleep <= {:?}", dur);

    let now = axhal::time::monotonic_time();

    axtask::sleep(dur);

    let after = axhal::time::monotonic_time();
    let actual = after - now;

    if let Some(diff) = dur.checked_sub(actual) {
        nullable!(uspace.write(rem, timespec::from_time_value(diff)))?;
        Err(LinuxError::EINTR)
    } else {
        Ok(0)
    }
}

/// Sleep for a specified time using a specific clock.
///
/// # Arguments
/// * `clock_id` - Clock identifier (CLOCK_REALTIME, CLOCK_MONOTONIC)
/// * `flags` - Sleep flags (currently unused)
/// * `req` - Time to sleep
/// * `rem` - Remaining time if interrupted (NULL if not needed)
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
