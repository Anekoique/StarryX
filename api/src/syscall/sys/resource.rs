use axerrno::{LinuxError, LinuxResult};
use axprocess::Pid;
use axtask::{TaskExtRef, current};
use starry_core::task::{ProcessData, get_process};

use crate::{
    ctypes::{RLIM_NLIMITS, RLIMIT_DATA, RLIMIT_NOFILE, RLIMIT_STACK, rlimit, rlimit64, timeval},
    fs::AX_FILE_LIMIT,
    ptr::{UserConstPtr, UserPtr, nullable},
};

// Resource usage constants (using raw values to match i32 parameter type)
const RUSAGE_SELF_VAL: i32 = 0;
const RUSAGE_CHILDREN_VAL: i32 = -1;

// Resource usage structure for our implementation
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StarryRusage {
    pub ru_utime: timeval,    // user time used
    pub ru_stime: timeval,    // system time used
    pub ru_maxrss: isize,     // maximum resident set size
    pub ru_ixrss: isize,      // integral shared memory size
    pub ru_idrss: isize,      // integral unshared data size
    pub ru_isrss: isize,      // integral unshared stack size
    pub ru_minflt: isize,     // page reclaims
    pub ru_majflt: isize,     // page faults
    pub ru_nswap: isize,      // swaps
    pub ru_inblock: isize,    // block input operations
    pub ru_oublock: isize,    // block output operations
    pub ru_msgsnd: isize,     // messages sent
    pub ru_msgrcv: isize,     // messages received
    pub ru_nsignals: isize,   // signals received
    pub ru_nvcsw: isize,      // voluntary context switches
    pub ru_nivcsw: isize,     // involuntary context switches
}

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

pub fn sys_getrusage(who: i32, usage: UserPtr<StarryRusage>) -> LinuxResult<isize> {
    debug!("sys_getrusage <= who: {}", who);
    
    if let Some(usage) = nullable!(usage.get_as_mut())? {
        match who {
            RUSAGE_SELF_VAL => {
                // 提供基本的资源使用信息
                *usage = StarryRusage {
                    ru_utime: timeval { tv_sec: 0, tv_usec: 0 },  // 用户时间
                    ru_stime: timeval { tv_sec: 0, tv_usec: 0 },  // 系统时间
                    ru_maxrss: 0,      // 最大内存使用量
                    ru_ixrss: 0,       // 共享内存大小
                    ru_idrss: 0,       // 非共享数据大小
                    ru_isrss: 0,       // 非共享栈大小
                    ru_minflt: 0,      // 页面回收
                    ru_majflt: 0,      // 页面错误
                    ru_nswap: 0,       // 交换次数
                    ru_inblock: 0,     // 输入块操作
                    ru_oublock: 0,     // 输出块操作
                    ru_msgsnd: 0,      // 发送消息数
                    ru_msgrcv: 0,      // 接收消息数
                    ru_nsignals: 0,    // 接收信号数
                    ru_nvcsw: 0,       // 自愿上下文切换
                    ru_nivcsw: 0,      // 非自愿上下文切换
                };
            }
            RUSAGE_CHILDREN_VAL => {
                // 子进程资源使用 - 返回零值
                *usage = StarryRusage {
                    ru_utime: timeval { tv_sec: 0, tv_usec: 0 },
                    ru_stime: timeval { tv_sec: 0, tv_usec: 0 },
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
            }
            _ => return Err(LinuxError::EINVAL),
        }
        Ok(0)
    } else {
        Err(LinuxError::EFAULT)
    }
}
