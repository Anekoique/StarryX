use alloc::sync::Arc;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FS_CONTEXT;
use axhal::arch::{TrapFrame, UspaceContext};
use axsync::Mutex;
use axtask::{TaskExtRef, current};
use spin::RwLock;

use axprocess::Pid;
use axsignal::Signo;
use axuspace::{UserPtr, UserSpaceAccess};
use xcore::{
    fs::FD_TABLE,
    mm::{XUserSpace, copy_from_kernel},
    task::{XProcess, XTaskExt, XThread, add_thread_to_table, new_user_task},
};

use crate::{
    ctypes::{SIGCHLD, task::CloneFlags},
    ipc::IPC_MANAGER,
};

/// Create a child process or thread.
///
/// # Arguments
/// * `tf` - Trap frame containing register state
/// * `flags` - Clone flags controlling behavior
/// * `stack` - Stack pointer for the new task (0 for same stack)
/// * `parent_tid` - Address to store parent thread ID
/// * `child_tid` - Address to store child thread ID
/// * `tls` - Thread-local storage pointer
pub fn sys_clone(
    tf: &TrapFrame,
    flags: u32,
    stack: usize,
    parent_tid: usize,
    #[cfg(any(target_arch = "x86_64", target_arch = "loongarch64"))] child_tid: usize,
    tls: usize,
    #[cfg(not(any(target_arch = "x86_64", target_arch = "loongarch64")))] child_tid: usize,
) -> LinuxResult<isize> {
    const FLAG_MASK: u32 = 0xff;
    let exit_signal = flags & FLAG_MASK;
    let flags = CloneFlags::from_bits_truncate(flags & !FLAG_MASK);

    let curr = current();
    let process = curr.task_ext().process();
    let xprocess = process.data::<XProcess>().unwrap();
    let uspace = xprocess.uspace();

    info!(
        "sys_clone <= flags: {:?}, exit_signal: {}, stack: {:#x}, ptid: {:#x}, ctid: {:#x}, tls: {:#x}",
        flags, exit_signal, stack, parent_tid, child_tid, tls
    );

    if exit_signal != 0 && flags.contains(CloneFlags::THREAD | CloneFlags::PARENT) {
        return Err(LinuxError::EINVAL);
    }
    if flags.contains(CloneFlags::THREAD) && !flags.contains(CloneFlags::VM | CloneFlags::SIGHAND) {
        return Err(LinuxError::EINVAL);
    }
    let exit_signal = Signo::from_repr(exit_signal as u8);

    let mut new_uctx = UspaceContext::from(tf);
    if stack != 0 {
        new_uctx.set_sp(stack);
    }
    if flags.contains(CloneFlags::SETTLS) {
        new_uctx.set_tls(tls);
    }
    new_uctx.set_retval(0);

    let set_child_tid = if flags.contains(CloneFlags::CHILD_SETTID) {
        Some(uspace.raw_ptr(UserPtr::<u32>::from(child_tid))?)
    } else {
        None
    };
    let mut new_task = new_user_task(curr.name(), new_uctx, set_child_tid);

    let tid = new_task.id().as_u64() as Pid;
    if flags.contains(CloneFlags::PARENT_SETTID) {
        uspace.write(UserPtr::<Pid>::from(parent_tid), tid)?;
    }

    let process = if flags.contains(CloneFlags::THREAD) {
        new_task
            .ctx_mut()
            .set_page_table_root(uspace.aspace.lock().page_table_root());

        process
    } else {
        let parent = if flags.contains(CloneFlags::PARENT) {
            process.parent().ok_or(LinuxError::EINVAL)?
        } else {
            process.clone()
        };
        let builder = parent.fork(tid);

        let aspace = if flags.contains(CloneFlags::VM) {
            uspace.aspace.clone()
        } else {
            let mut aspace = uspace.aspace.lock().try_clone()?;
            copy_from_kernel(&mut aspace)?;
            Arc::new(Mutex::new(aspace))
        };

        let vma_manager = RwLock::new(uspace.vma_manager.read().clone());

        new_task
            .ctx_mut()
            .set_page_table_root(aspace.lock().page_table_root());

        let signal_actions = if flags.contains(CloneFlags::SIGHAND) {
            parent
                .data::<XProcess>()
                .map_or_else(Arc::default, |it| it.signal.actions.clone())
        } else {
            Arc::default()
        };
        let process_data = XProcess::new(
            xprocess.exe_path.read().clone(),
            XUserSpace::new(aspace, vma_manager),
            signal_actions,
            exit_signal,
            Some(xprocess.rlimits.read().clone()),
        );

        if flags.contains(CloneFlags::FILES) {
            FD_TABLE
                .deref_from(&process_data.ns)
                .init_shared(FD_TABLE.share());
        } else {
            FD_TABLE
                .deref_from(&process_data.ns)
                .init_new(FD_TABLE.copy_inner());
        }

        if flags.contains(CloneFlags::FS) {
            FS_CONTEXT
                .deref_from(&process_data.ns)
                .init_shared(FS_CONTEXT.share());
        } else {
            FS_CONTEXT
                .deref_from(&process_data.ns)
                .init_new(FS_CONTEXT.copy_inner());
        }

        if flags.contains(CloneFlags::NEWIPC) {
            IPC_MANAGER
                .deref_from(&process_data.ns)
                .init_new(IPC_MANAGER.copy_inner());
        } else {
            IPC_MANAGER
                .deref_from(&process_data.ns)
                .init_shared(IPC_MANAGER.share());
        }

        builder.data(process_data).build()
    };

    let thread_data = XThread::new(process.data().unwrap());
    if flags.contains(CloneFlags::CHILD_CLEARTID) {
        thread_data.set_clear_child_tid(child_tid);
    }

    let thread = process.new_thread(tid).data(thread_data).build();
    add_thread_to_table(&thread);
    new_task.init_task_ext(XTaskExt::new(thread));
    axtask::spawn_task(new_task);

    Ok(tid as _)
}

/// Create a child process (fork).
///
/// # Arguments
/// * `tf` - Trap frame containing register state
pub fn sys_fork(tf: &TrapFrame) -> LinuxResult<isize> {
    sys_clone(tf, SIGCHLD, 0, 0, 0, 0)
}
