use alloc::{sync::Arc, vec::Vec};
use axerrno::{LinuxError, LinuxResult};
use axprocess::{Pid, Process};
use axtask::{TaskExtRef, current};
use starry_core::task::ProcessData;

use crate::{
    ptr::{UserPtr, nullable},
    task::WaitOptions,
};

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

pub fn sys_wait4(pid: i32, exit_code_ptr: UserPtr<i32>, options: u32) -> LinuxResult<isize> {
    let options = WaitOptions::from_bits_truncate(options);
    info!("sys_wait4 <= pid: {:?}, options: {:?}", pid, options);

    let curr = current();
    let proc_data = curr.task_ext().process_data();
    let process = curr.task_ext().thread.process();

    let pid = if pid == -1 {
        WaitPid::Any
    } else if pid == 0 {
        WaitPid::Pgid(process.group().pgid())
    } else if pid > 0 {
        WaitPid::Pid(pid as _)
    } else {
        WaitPid::Pgid(-pid as _)
    };

    let children = process
        .children()
        .into_iter()
        .filter(|child| pid.apply(child))
        .filter(|child| {
            options.contains(WaitOptions::WALL)
                || (options.contains(WaitOptions::WCLONE)
                    == child.data::<ProcessData>().unwrap().is_clone_child())
        })
        .collect::<Vec<_>>();
    if children.is_empty() {
        return Err(LinuxError::ECHILD);
    }

    let exit_code = nullable!(exit_code_ptr.get_as_mut())?;
    loop {
        if let Some(child) = children.iter().find(|child| child.is_zombie()) {
            if !options.contains(WaitOptions::WNOWAIT) {
                child.free();
            }
            if let Some(exit_code) = exit_code {
                *exit_code = child.exit_code();
            }
            return Ok(child.pid() as _);
        } else if options.contains(WaitOptions::WNOHANG) {
            return Ok(0);
        } else {
            proc_data.child_exit_wq.wait();
        }
    }
}
