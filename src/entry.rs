use alloc::{borrow::ToOwned, string::String, sync::Arc};
use axfs_ng::FS_CONTEXT;
use axhal::arch::UspaceContext;
use axprocess::{Pid, init_proc};
use axsignal::Signo;
use axsync::Mutex;
use axvma::VmaManager;
use linux_raw_sys::general::AT_FDCWD;
use spin::RwLock;
use xapi::{fs::with_fs, ipc::IPC_MANAGER};
use xcore::{
    fs::FD_TABLE,
    mm::{XUserSpace, copy_from_kernel, load_app, map_trampoline, new_aspace},
    task::{XProcess, XTaskExt, XThread, add_thread_to_table, new_user_task},
};

pub fn run_user_app(args: &[String], envs: &[String]) -> Option<i32> {
    let mut uspace = new_aspace()
        .and_then(|mut it| {
            copy_from_kernel(&mut it)?;
            map_trampoline(&mut it)?;
            Ok(it)
        })
        .expect("Failed to create user address space");

    let exe_path = &args[0];
    let name = with_fs(AT_FDCWD, exe_path, |fs| {
        let loc = fs.resolve(exe_path)?;
        let name = loc.name().to_owned();
        fs.set_current_dir(loc.parent().unwrap())?;
        Ok(name)
    })
    .expect("Failed to resolve executable path");

    let (entry_vaddr, ustack_top) = load_app(&mut uspace, None, args, envs)
        .unwrap_or_else(|e| panic!("Failed to load user app: {}", e));

    let uctx = UspaceContext::new(entry_vaddr.into(), ustack_top, 2333);

    let mut task = new_user_task(&name, uctx, None);
    task.ctx_mut().set_page_table_root(uspace.page_table_root());

    let process_data = XProcess::new(
        exe_path.clone(),
        XUserSpace::new(Arc::new(Mutex::new(uspace)), RwLock::new(VmaManager::new())),
        Arc::default(),
        Some(Signo::SIGCHLD),
        None,
    );

    FD_TABLE
        .deref_from(&process_data.ns)
        .init_new(FD_TABLE.copy_inner());
    FS_CONTEXT
        .deref_from(&process_data.ns)
        .init_new(FS_CONTEXT.copy_inner());
    IPC_MANAGER
        .deref_from(&process_data.ns)
        .init_new(IPC_MANAGER.copy_inner());

    let tid = task.id().as_u64() as Pid;
    let process = init_proc().fork(tid).data(process_data).build();

    let thread = process
        .new_thread(tid)
        .data(XThread::new(process.data().unwrap()))
        .build();
    add_thread_to_table(&thread);

    task.init_task_ext(XTaskExt::new(thread));

    let task = axtask::spawn_task(task);

    // TODO: we need a way to wait on the process but not only the main task
    task.join()
}
