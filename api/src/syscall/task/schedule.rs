use axerrno::{LinuxError, LinuxResult};
use axprocess::Pid;
use axtask::{current, get_task_by_id_raw};

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
    pid: Pid,
    cpuset_size: usize,
    mask: UserPtr<usize>,
) -> LinuxResult<isize> {
    let len = axconfig::SMP.min(cpuset_size);
    // Get the user buffer as usize slice
    let mask_slice = mask.get_as_mut_slice(len)?;

    // Parse CPU mask from user buffer
    let mut cpumask = axtask::AxCpuMask::new();
    let bits_per_usize = core::mem::size_of::<usize>() * 8;

    for cpu_id in 0..len {
        let usize_index = cpu_id / bits_per_usize;
        let bit_index = cpu_id % bits_per_usize;

        if usize_index < mask_slice.len() && (mask_slice[usize_index] & (1usize << bit_index)) != 0
        {
            cpumask.set(cpu_id, true);
        }
    }

    // Check if the mask is empty (not allowed)
    if cpumask.is_empty() {
        return Err(LinuxError::EINVAL);
    }

    // Set the task's affinity using the task ID
    if pid == 0 {
        // For current task (pid = 0), use current task's TaskId
        let current_task_id = current().id();
        if axtask::set_affinity(current_task_id, cpumask) {
            Ok(0)
        } else {
            Err(LinuxError::EINVAL)
        }
    } else {
        // For specific task, get the task first to extract its TaskId
        match get_task_by_id_raw(pid as u64) {
            Some(task) => {
                let task_id = task.id();
                if axtask::set_affinity(task_id, cpumask) {
                    Ok(0)
                } else {
                    Err(LinuxError::EINVAL)
                }
            }
            None => Err(LinuxError::ESRCH),
        }
    }
}

pub fn sys_sched_getaffinity(
    pid: Pid,
    cpuset_size: usize,
    mask: UserPtr<usize>,
) -> LinuxResult<isize> {
    // Get the target task - current task if pid is 0, otherwise find by pid
    let cpuset = if pid == 0 {
        current().as_task_ref().cpumask()
    } else {
        get_task_by_id_raw(pid as u64)
            .ok_or(LinuxError::ESRCH)?
            .cpumask()
    };

    let len = axconfig::SMP.min(cpuset_size);
    let mask_slice = mask.get_as_mut_slice(len)?;
    mask_slice.fill(0);

    let bits_per_usize = core::mem::size_of::<usize>() * 8;

    for cpu_id in 0..len {
        if cpuset.get(cpu_id) {
            let usize_index = cpu_id / bits_per_usize;
            let bit_index = cpu_id % bits_per_usize;
            if usize_index < mask_slice.len() {
                mask_slice[usize_index] |= 1usize << bit_index;
            }
        }
    }

    Ok(len as isize)
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
