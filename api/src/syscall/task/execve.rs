use core::ffi::c_char;

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FS_CONTEXT;
use axhal::arch::TrapFrame;
use axtask::{TaskExtRef, current};
use starry_core::mm::{load_user_app, map_trampoline};

use crate::ptr::UserConstPtr;

pub fn sys_execve(
    tf: &mut TrapFrame,
    path: UserConstPtr<c_char>,
    argv: UserConstPtr<UserConstPtr<c_char>>,
    envp: UserConstPtr<UserConstPtr<c_char>>,
) -> LinuxResult<isize> {
    let mut path = path.get_as_str()?.to_string();

    // Add "./" prefix if path doesn't start with "/" or "./"
    if !path.starts_with('/') && !path.starts_with("./") {
        path = format!("./{}", path);
    }

    let mut args: Vec<String> = argv
        .get_as_null_terminated()?
        .iter()
        .map(|arg| arg.get_as_str().map(|s| s.to_string()))
        .collect::<Result<Vec<_>, _>>()?;

    // Add "./" prefix to the first arg if it doesn't start with "/" or "./"
    if let Some(first_arg) = args.get_mut(0) {
        if !first_arg.starts_with('/') && !first_arg.starts_with("./") {
            *first_arg = format!("./{}", first_arg);
        }
    }

    let envs = envp
        .get_as_null_terminated()?
        .iter()
        .map(|env| env.get_as_str().map(|s| s.to_string()))
        .collect::<Result<Vec<_>, _>>()?;

    info!(
        "sys_execve: path: {:?}, args: {:?}, envs: {:?}",
        path, args, envs
    );

    let curr = current();
    let curr_ext = curr.task_ext();

    if curr_ext.thread.process().threads().len() > 1 {
        // TODO: handle multi-thread case
        error!("sys_execve: multi-thread not supported");
        return Err(LinuxError::EAGAIN);
    }

    let mut aspace = curr_ext.process_data().aspace.lock();
    aspace.unmap_user_areas()?;
    let mut vma_mapping = curr_ext.process_data().vma_mapping.write();
    vma_mapping.clear();
    map_trampoline(&mut aspace)?;
    axhal::arch::flush_tlb(None);

    let (entry_point, user_stack_base) = load_user_app(&mut aspace, Some(&path), &args, &envs)
        .map_err(|_| {
            error!("Failed to load app {}", path);
            LinuxError::ENOENT
        })?;
    drop(aspace);

    let name = path
        .rsplit_once('/')
        .map_or(path.as_str(), |(_, name)| name);
    curr.set_name(name);
    *curr_ext.process_data().exe_path.write() = FS_CONTEXT.lock().canonicalize(path)?.to_string();

    // TODO: fd close-on-exec

    tf.set_ip(entry_point.as_usize());
    tf.set_sp(user_stack_base.as_usize());
    Ok(0)
}
