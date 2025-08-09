use alloc::{sync::Arc, vec::Vec};

use axerrno::{LinuxError, LinuxResult};
use axtask::{TaskExtRef, current};

use xprocess::{Pid, Process};
use xuspace::{UserPtr, UserSpaceAccess, nullable};
use xutils::ctypes::task::WaitOptions;

use xcore::task::XProcess;

#[derive(Debug, Clone, Copy)]
enum WaitPid {
    /// Wait for any child process
    Any,
    /// Wait for the child whose process ID is equal to the value.
    Pid(Pid),
    /// Wait for any child process whose process group ID is equal to the value.
    Pgid(Pid),
}

impl WaitPid {
    fn apply(&self, child: &Arc<Process>) -> bool {
        match self {
            WaitPid::Any => true,
            WaitPid::Pid(pid) => child.pid() == *pid,
            WaitPid::Pgid(pgid) => child.group().pgid() == *pgid,
        }
    }
}

/// Wait for a child process to change state.
///
/// # Arguments
/// * `pid` - Process ID to wait for (-1 for any, 0 for same group, >0 specific PID)
/// * `exit_code_ptr` - Buffer to store exit status (NULL if not needed)
/// * `options` - Wait options (WNOHANG, WUNTRACED, etc.)
pub fn sys_wait4(pid: i32, exit_code_ptr: UserPtr<i32>, options: u32) -> LinuxResult<isize> {
    let Some(options) = WaitOptions::from_bits(options) else {
        return Err(LinuxError::EINVAL);
    };
    info!("sys_wait4 <= pid: {:?}, options: {:?}", pid, options);

    let process = current().task_ext().process();
    let xprocess = process.data::<XProcess>().unwrap();
    let uspace = xprocess.uspace();

    let pid = if pid == -1 {
        WaitPid::Any
    } else if pid == 0 {
        WaitPid::Pgid(process.group().pgid())
    } else if pid > 0 {
        WaitPid::Pid(pid as _)
    } else {
        if pid == i32::MIN {
            return Err(LinuxError::ESRCH);
        }
        WaitPid::Pgid(-pid as _)
    };

    let children = process
        .children()
        .into_iter()
        .filter(|child| pid.apply(child))
        .filter(|child| {
            options.contains(WaitOptions::WALL)
                || (options.contains(WaitOptions::WCLONE)
                    == child.data::<XProcess>().unwrap().is_clone_child())
        })
        .collect::<Vec<_>>();
    if children.is_empty() {
        return Err(LinuxError::ECHILD);
    }

    loop {
        if let Some(child) = children.iter().find(|child| child.is_zombie()) {
            if !options.contains(WaitOptions::WNOWAIT) {
                child.free();
            }
            nullable!(uspace.write(exit_code_ptr, child.exit_code()))?;
            return Ok(child.pid() as _);
        } else if options.contains(WaitOptions::WNOHANG) {
            return Ok(0);
        } else {
            xprocess.child_exit_wq.wait();
        }
    }
}
