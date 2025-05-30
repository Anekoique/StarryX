use axerrno::{LinuxError, LinuxResult};
use axprocess::Pid;
use axtask::{TaskExtRef, current};
use starry_core::task::{ProcessData, get_process, time_stat_output};

use crate::{
    ctypes::{
        __kernel_old_timeval, RLIM_NLIMITS, RLIMIT_DATA, RLIMIT_NOFILE, RLIMIT_STACK,
        RUSAGE_CHILDREN, rlimit, rlimit64, rusage,
    },
    fs::AX_FILE_LIMIT,
    ptr::{UserConstPtr, UserPtr, nullable},
    time::TimeValueLike,
};

pub fn sys_getrlimit(resource: u32, rlimit: UserPtr<rlimit>) -> LinuxResult<isize> {
    if let Some(rlimit) = nullable!(rlimit.get_as_mut())? {
        match resource {
            RLIMIT_DATA => {}
            RLIMIT_STACK => {
                rlimit.rlim_cur = axconfig::TASK_STACK_SIZE as _;
                rlimit.rlim_max = axconfig::TASK_STACK_SIZE as _;
            }
            RLIMIT_NOFILE => {
                rlimit.rlim_cur = AX_FILE_LIMIT as _;
                rlimit.rlim_max = AX_FILE_LIMIT as _;
            }
            _ => return Err(LinuxError::EINVAL),
        }
        Ok(0)
    } else {
        Ok(0)
    }
}

pub fn sys_setrlimit(resource: u32, rlimit: UserPtr<rlimit>) -> LinuxResult<isize> {
    if let Some(_rlimit) = nullable!(rlimit.get_as_mut())? {
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
}

pub fn sys_prlimit64(
    pid: Pid,
    resource: u32,
    new_limit: UserConstPtr<rlimit64>,
    old_limit: UserPtr<rlimit64>,
) -> LinuxResult<isize> {
    debug!("resource: {}", resource);
    if resource >= RLIM_NLIMITS {
        return Err(LinuxError::EINVAL);
    }

    let proc = if pid == 0 {
        current().task_ext().thread.process().clone()
    } else {
        get_process(pid)?
    };
    let proc_data: &ProcessData = proc.data().unwrap();
    if let Some(old_limit) = nullable!(old_limit.get_as_mut())? {
        let limit = &proc_data.rlimits.read()[resource];
        old_limit.rlim_cur = limit.current;
        old_limit.rlim_max = limit.max;
    }

    if let Some(new_limit) = nullable!(new_limit.get_as_ref())? {
        if new_limit.rlim_cur > new_limit.rlim_max {
            return Err(LinuxError::EINVAL);
        }

        let limit = &mut proc_data.rlimits.write()[resource];
        if new_limit.rlim_max <= limit.max {
            limit.max = new_limit.rlim_max;
        } else {
            debug!(
                "new_limit.rlim_max: {}, limit.max: {}",
                new_limit.rlim_max, limit.max
            );
            return Err(LinuxError::EPERM);
        }

        limit.current = new_limit.rlim_cur;
    }

    Ok(0)
}

pub fn sys_getrusage(who: i32, usage: UserPtr<rusage>) -> LinuxResult<isize> {
    const RUSAGE_SELF: i32 = 0;
    if let Some(usage) = nullable!(usage.get_as_mut())? {
        match who {
            RUSAGE_SELF | RUSAGE_CHILDREN => {
                let (utime_ns, _, _, stime_ns, _, _) = time_stat_output();
                usage.ru_utime = __kernel_old_timeval::from_nanos(utime_ns as u64);
                usage.ru_stime = __kernel_old_timeval::from_nanos(stime_ns as u64);
                debug!("usage: {:?}", usage);
                Ok(0)
            }
            _ => Err(LinuxError::EINVAL),
        }
    } else {
        Err(LinuxError::EFAULT)
    }
}
