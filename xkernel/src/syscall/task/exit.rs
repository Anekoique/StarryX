use core::sync::atomic::Ordering;

use xtask::{TaskExtRef, current};

use xprocess::Pid;
use xsignal::{SignalInfo, Signo};
use xuspace::{UserPtr, nullable};
use xutils::ctypes::{SI_KERNEL, robust_list_head};

use crate::{
    fs::fd::FD_TABLE,
    ipc::clear_proc_shm,
    task::{FutexKey, XProcess, XThread, send_signal_process, send_signal_thread},
};

use crate::syscall::task::exit_robust_list;

pub fn do_exit(exit_code: i32, group_exit: bool) -> ! {
    let curr = current();
    let thread = curr.task_ext().thread_ref();
    let xthread = thread.data::<XThread>().unwrap();
    let process = thread.process();
    let xprocess = process.data::<XProcess>().unwrap();
    let uspace = xprocess.uspace();

    info!("{:?} exit with code: {}", thread, exit_code);

    let clear_child_tid = UserPtr::<Pid>::from(xthread.clear_child_tid());
    if uspace.write(clear_child_tid, 0).is_ok() {
        let key = FutexKey::new(xthread.clear_child_tid());
        let guard = xprocess.futex_table_for(&key).get(&key);
        if let Some(futex) = guard {
            futex.wq.notify_one(false);
        }
        xtask::yield_now();
    }
    let head: UserPtr<robust_list_head> = xthread.robust_list_head.load(Ordering::SeqCst).into();
    if let Ok(Some(mut head_value)) = nullable!(uspace.read(head))
        && let Err(err) = exit_robust_list(&mut head_value)
    {
        warn!("exit robust list failed: {:?}", err);
    }

    if thread.exit(exit_code) {
        let mut aspace = uspace.aspace.lock();
        if let Err(error) = aspace.unmap_user_areas() {
            warn!("failed to release user address space on exit: {error:?}");
        }
        uspace.mapped_files.prune(&aspace);
        drop(aspace);
        process.exit();
        if let Some(parent) = process.parent() {
            if let Some(signo) = xprocess.exit_signal {
                let _ = send_signal_process(&parent, SignalInfo::new(signo, SI_KERNEL as _));
            }
            if let Some(data) = parent.data::<XProcess>() {
                data.child_exit_wq.notify_all(false)
            }
        }
        // TODO: clear namespace resources
        // FIXME: xns should drop all the resources
        FD_TABLE.clear();
        clear_proc_shm(process.pid());
    }
    if group_exit && !process.is_group_exited() {
        process.group_exit();
        let sig = SignalInfo::new(Signo::SIGKILL, SI_KERNEL as _);
        for thr in process.threads() {
            let _ = send_signal_thread(&thr, sig.clone());
        }
    }
    xtask::exit(exit_code)
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
