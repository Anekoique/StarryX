use core::sync::atomic::Ordering;

use axtask::{TaskExtRef, current};

use xprocess::Pid;
use xsignal::{SignalInfo, Signo};
use xuspace::{UserPtr, UserSpaceAccess, nullable};
use xutils::ctypes::{SI_KERNEL, robust_list_head};

use xcore::{
    fs::fd::FD_TABLE,
    ipc::IPC_MANAGER,
    task::{FutexKey, XProcess, XThread, send_signal_process, send_signal_thread},
};

use crate::task::exit_robust_list;

pub fn do_exit(exit_code: i32, group_exit: bool) -> ! {
    let curr = current();
    let thread = curr.task_ext().thread_ref();
    let xthread = thread.data::<XThread>().unwrap();
    let process = thread.process();
    let xprocess = process.data::<XProcess>().unwrap();
    let uspace = xprocess.uspace();

    info!("{:?} exit with code: {}", thread, exit_code);

    let clear_child_tid = UserPtr::<Pid>::from(xthread.clear_child_tid());
    if let Ok(clear_tid) = uspace.raw_ptr(clear_child_tid) {
        *clear_tid = 0;

        let key = FutexKey::new(clear_tid as *const _ as usize);
        let guard = xprocess.futex_table_for(&key).get(&key);
        if let Some(futex) = guard {
            futex.wq.notify_one(false);
        }
        axtask::yield_now();
    }
    let head: UserPtr<robust_list_head> = xthread.robust_list_head.load(Ordering::SeqCst).into();
    if let Ok(Some(head)) = nullable!(uspace.raw_ptr(head))
        && let Err(err) = exit_robust_list(head)
    {
        warn!("exit robust list failed: {:?}", err);
    }

    if thread.exit(exit_code) {
        process.exit();
        if let Some(parent) = process.parent() {
            if let Some(signo) = xprocess.exit_signal {
                let _ = send_signal_process(&parent, SignalInfo::new(signo, SI_KERNEL as _));
            }
            if let Some(data) = parent.data::<XProcess>() {
                data.child_exit_wq.notify_all(false)
            }
        }

        process.exit();
        // TODO: clear namespace resources
        // FIXME: axns should drop all the resources
        FD_TABLE.clear();
        IPC_MANAGER.clear();
    }
    if group_exit && !process.is_group_exited() {
        process.group_exit();
        let sig = SignalInfo::new(Signo::SIGKILL, SI_KERNEL as _);
        for thr in process.threads() {
            let _ = send_signal_thread(&thr, sig.clone());
        }
    }
    axtask::exit(exit_code)
}

/// Terminate the calling thread.
///
/// # Arguments
/// * `exit_code` - Exit status code
pub fn sys_exit(exit_code: i32) -> ! {
    do_exit(exit_code << 8, false)
}

/// Terminate all threads in the current process.
///
/// # Arguments
/// * `exit_code` - Exit status code
pub fn sys_exit_group(exit_code: i32) -> ! {
    do_exit(exit_code << 8, true)
}
