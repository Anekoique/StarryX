use xerrno::{LinuxError, LinuxResult};

use xprocess::Pid;
use xuspace::{UserConstPtr, UserPtr, nullable};
use xutils::{
    ctypes::{
        __kernel_old_timeval, RLIM_NLIMITS, RLIMIT_DATA, RLIMIT_NOFILE, RLIMIT_STACK,
        RUSAGE_CHILDREN, rlimit, rlimit64, rusage,
    },
    time::TimeValueLike,
};

use crate::{
    fs::fd::FD_LIMIT,
    task::{XProcess, get_process, with_uspace},
    time::time_stat_output,
};

/// Get resource limits.
///
/// # Arguments
/// * `resource` - Resource type (RLIMIT_DATA, RLIMIT_STACK, RLIMIT_NOFILE)
/// * `rlimit` - Buffer to store resource limits
pub fn sys_getrlimit(resource: u32, rlimit: UserPtr<rlimit>) -> LinuxResult<isize> {
    trace!(
        "sys_getrlimit <= resource: {}, rlimit: {:?}",
        resource, rlimit
    );
    with_uspace(|uspace| {
        if let Some(mut value) = nullable!(uspace.read(rlimit))? {
            match resource {
                RLIMIT_DATA => {}
                RLIMIT_STACK => {
                    value.rlim_cur = xconfig::TASK_STACK_SIZE as _;
                    value.rlim_max = xconfig::TASK_STACK_SIZE as _;
                }
                RLIMIT_NOFILE => {
                    value.rlim_cur = FD_LIMIT as _;
                    value.rlim_max = FD_LIMIT as _;
                }
                _ => return Err(LinuxError::EINVAL),
            }
            uspace.write(rlimit, value)?;
            Ok(0)
        } else {
            Ok(0)
        }
    })
}

/// Set resource limits.
///
/// # Arguments
/// * `resource` - Resource type (RLIMIT_DATA, RLIMIT_STACK, RLIMIT_NOFILE)
/// * `rlimit` - New resource limits to set
pub fn sys_setrlimit(resource: u32, rlimit: UserPtr<rlimit>) -> LinuxResult<isize> {
    trace!(
        "sys_setrlimit <= resource: {}, rlimit: {:?}",
        resource, rlimit
    );
    with_uspace(|uspace| {
        if let Some(_rlimit) = nullable!(uspace.read(rlimit))? {
            match resource {
                RLIMIT_DATA => {}
                RLIMIT_STACK => {}
                RLIMIT_NOFILE => {}
                _ => return Err(LinuxError::EINVAL),
            }
            // Currently do not support set resources
            Ok(0)
        } else {
            Err(LinuxError::EINVAL)
        }
    })
}

/// Get and set resource limits for a process.
///
/// # Arguments
/// * `pid` - Process ID (0 for current process)
/// * `resource` - Resource type
/// * `new_limit` - New resource limits to set (NULL to only get)
/// * `old_limit` - Buffer to store current resource limits (NULL to only set)
pub fn sys_prlimit64(
    pid: Pid,
    resource: u32,
    new_limit: UserConstPtr<rlimit64>,
    old_limit: UserPtr<rlimit64>,
) -> LinuxResult<isize> {
    trace!(
        "sys_prlimit64 <= pid: {}, resource: {}, new_limit: {:?}, old_limit: {:?}",
        pid, resource, new_limit, old_limit
    );
    if resource >= RLIM_NLIMITS {
        return Err(LinuxError::EINVAL);
    }

    let proc = get_process(pid)?;
    let xprocess = XProcess::from_process(&proc);
    let uspace = xprocess.uspace();
    if let Some(mut old_value) = nullable!(uspace.read(old_limit))? {
        let limit = &xprocess.rlimits.read()[resource];
        old_value.rlim_cur = limit.current;
        old_value.rlim_max = limit.max;
        uspace.write(old_limit, old_value)?;
    }

    if let Some(new_limit) = nullable!(uspace.read(new_limit))? {
        if new_limit.rlim_cur > new_limit.rlim_max {
            return Err(LinuxError::EINVAL);
        }

        let limit = &mut xprocess.rlimits.write()[resource];
        if new_limit.rlim_max <= limit.max {
            limit.max = new_limit.rlim_max;
        } else {
            return Ok(0);
        }

        limit.current = new_limit.rlim_cur;
    }

    Ok(0)
}

/// Get resource usage statistics.
///
/// # Arguments
/// * `who` - Target for resource usage (RUSAGE_SELF, RUSAGE_CHILDREN)
/// * `usage` - Buffer to store resource usage statistics
pub fn sys_getrusage(who: i32, usage: UserPtr<rusage>) -> LinuxResult<isize> {
    trace!("sys_getrusage <= who: {}, usage: {:?}", who, usage);
    const RUSAGE_SELF: i32 = 0;
    if !matches!(who, RUSAGE_SELF | RUSAGE_CHILDREN) {
        return Err(LinuxError::EINVAL);
    }
    if usage.is_null() {
        return Err(LinuxError::EFAULT);
    }

    let (utime_ns, _, _, stime_ns, _, _) = time_stat_output();
    let value = rusage {
        ru_utime: __kernel_old_timeval::from_nanos(utime_ns as u64),
        ru_stime: __kernel_old_timeval::from_nanos(stime_ns as u64),
        ru_maxrss: 0,
        ru_ixrss: 0,
        ru_idrss: 0,
        ru_isrss: 0,
        ru_minflt: 0,
        ru_majflt: 0,
        ru_nswap: 0,
        ru_inblock: 0,
        ru_oublock: 0,
        ru_msgsnd: 0,
        ru_msgrcv: 0,
        ru_nsignals: 0,
        ru_nvcsw: 0,
        ru_nivcsw: 0,
    };
    with_uspace(|uspace| uspace.write(usage, value))?;
    Ok(0)
}
