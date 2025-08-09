use axerrno::{LinuxError, LinuxResult};
use axtask::{AxCpuMask, set_affinity, with_task};

use xprocess::Pid;
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::{
    ctypes::{
        CLOCK_MONOTONIC, CLOCK_REALTIME, PRIO_PGRP, PRIO_PROCESS, PRIO_USER, TIMER_ABSTIME,
        timespec,
    },
    time::{TimeValue, TimeValueLike},
};

use xcore::task::{XThread, get_process, get_process_group, get_thread, have_signals, with_uspace};

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
    if cpuset_size == 0 {
        return Err(LinuxError::EINVAL);
    }

    with_task(pid.into(), |task| {
        let len = cpuset_size.min(axconfig::SMP.div_ceil(8));
        let mask_slice = with_uspace(|uspace| uspace.raw_slice(mask, len))?;
        let cpumask = task.cpumask();

        for item in mask_slice.iter_mut().take(len) {
            *item = 0;
        }

        for cpu_id in 0..axconfig::SMP.min(len * 8) {
            if cpumask.get(cpu_id) {
                mask_slice[cpu_id / 8] |= 1 << (cpu_id % 8);
            }
        }

        Ok(axconfig::SMP.div_ceil(8).min(cpuset_size) as isize)
    })
    .ok_or(LinuxError::ESRCH)?
}

/// Get scheduling parameters for a thread.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
/// * `_param` - Buffer to store scheduling parameters (currently unused)
pub fn sys_sched_getparam(pid: i32, param: UserPtr<usize>) -> LinuxResult<isize> {
    if pid < 0 {
        return Err(LinuxError::EINVAL);
    }
    let thread = get_thread(pid as _)?;
    with_uspace(|uspace| uspace.write(param, XThread::from_thread(&thread).get_priority() as _))
        .map_err(|_| LinuxError::EINVAL)?;
    Ok(0)
}

/// Set scheduling parameters for a thread.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
/// * `_param` - New scheduling parameters (currently unused)
pub fn sys_sched_setparam(pid: i32, param: UserPtr<usize>) -> LinuxResult<isize> {
    if pid < 0 {
        return Err(LinuxError::EINVAL);
    }
    let thread = get_thread(pid as _)?;
    with_uspace(|uspace| -> LinuxResult<()> {
        let priority = uspace.read(param)?;
        XThread::from_thread(&thread).set_priority(priority as _);
        Ok(())
    })
    .map_err(|_| LinuxError::EINVAL)?;
    Ok(0)
}

/// Set scheduling algorithm and parameters for a thread.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
/// * `policy` - Scheduling policy
/// * `_param` - Scheduling parameters (currently unused)
pub fn sys_sched_setscheduler(
    pid: i32,
    policy: usize,
    param: UserPtr<usize>,
) -> LinuxResult<isize> {
    if pid < 0 || policy > 6 {
        return Err(LinuxError::EINVAL);
    }
    let thread = get_thread(pid as _)?;
    with_uspace(|uspace| -> LinuxResult<()> {
        XThread::from_thread(&thread).set_policy(policy as _);
        uspace.write(param, XThread::from_thread(&thread).get_priority() as _)?;
        Ok(())
    })
    .map_err(|_| LinuxError::EINVAL)?;
    Ok(0)
}

/// Get scheduling algorithm for a thread.
///
/// # Arguments
/// * `_pid` - Thread ID (currently unused)
pub fn sys_sched_getscheduler(pid: i32) -> LinuxResult<isize> {
    if pid < 0 {
        return Err(LinuxError::EINVAL);
    }
    let thread = get_thread(pid as _)?;
    Ok(XThread::from_thread(&thread).get_policy() as _)
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

fn sleep(clock: impl Fn() -> TimeValue, dur: TimeValue) -> TimeValue {
    let start = clock();
    while clock() < start + dur {
        if have_signals() {
            break;
        }
        axtask::yield_now();
    }
    clock() - start
}

/// Sleep for a specified time.
///
/// # Arguments
/// * `req` - Time to sleep
/// * `rem` - Remaining time if interrupted (NULL if not needed)
pub fn sys_nanosleep(req: UserConstPtr<timespec>, rem: UserPtr<timespec>) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        let req = uspace.read(req)?;

        if req.tv_nsec < 0 || req.tv_nsec > 999_999_999 || req.tv_sec < 0 {
            return Err(LinuxError::EINVAL);
        }
        let dur = timespec::to_time_value(req)?;
        trace!("sys_nanosleep <= {:?}", dur);

        let actual = sleep(axhal::time::monotonic_time, dur);

        if let Some(diff) = dur.checked_sub(actual) {
            nullable!(uspace.write(rem, timespec::from_time_value(diff)))?;
            Err(LinuxError::EINTR)
        } else {
            Ok(0)
        }
    })
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
    let clock = match clock_id as u32 {
        CLOCK_MONOTONIC => axhal::time::monotonic_time,
        CLOCK_REALTIME => axhal::time::wall_time,
        _ => {
            warn!("sys_clock_nanosleep: invalid clock_id {}", clock_id);
            return Err(LinuxError::EOPNOTSUPP);
        }
    };
    with_uspace(|uspace| {
        let req = uspace.read(req)?;
        if req.tv_nsec < 0 || req.tv_nsec > 999_999_999 || req.tv_sec < 0 {
            return Err(LinuxError::EINVAL);
        }
        let req = timespec::to_time_value(req)?;
        let dur = if flags & TIMER_ABSTIME as usize != 0 {
            req.saturating_sub(clock())
        } else {
            req
        };
        let actual = sleep(clock, dur);
        if let Some(diff) = dur.checked_sub(actual) {
            nullable!(uspace.write(rem, timespec::from_time_value(diff)))?;
            Err(LinuxError::EINTR)
        } else {
            Ok(0)
        }
    })
}

pub fn sys_getpriority(which: u32, who: u32) -> LinuxResult<isize> {
    match which {
        PRIO_PROCESS => {
            let _proc_ = get_process(who)?;
            Ok(20)
        }
        PRIO_PGRP => {
            if who != 0 {
                let _pg = get_process_group(who)?;
            }
            Ok(20)
        }
        PRIO_USER => {
            if who == 0 {
                Ok(20)
            } else {
                Err(LinuxError::ESRCH)
            }
        }
        _ => Err(LinuxError::EINVAL),
    }
}

pub fn sys_setpriority(pid: i32, _which: usize, prio: isize) -> LinuxResult<isize> {
    if pid < 0 {
        return Err(LinuxError::EINVAL);
    }
    let _thread = get_thread(pid as _)?;
    if prio < 0 {
        return Err(LinuxError::EACCES);
    }
    Ok(0)
}
