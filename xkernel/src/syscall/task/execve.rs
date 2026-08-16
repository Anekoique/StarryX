use alloc::string::ToString;
use core::ffi::c_char;

use xerrno::{LinuxError, LinuxResult};
use xfs::FS_CONTEXT;
use xhal::arch::TrapFrame;
use xtask::{TaskExtRef, current};

use crate::{
    fs::fd::FD_TABLE,
    mm::{load_app, load_file},
    task::XProcess,
};
use xuspace::{UserConstPtr, UserSpaceAccess};

/// Execute a program.
///
/// # Arguments
/// * `tf` - Trap frame to modify for the new program
/// * `path` - Path to the executable
/// * `argv` - Program arguments (null-terminated array)
/// * `envp` - Environment variables (null-terminated array)
pub fn sys_execve(
    tf: &mut TrapFrame,
    path: UserConstPtr<c_char>,
    argv: UserConstPtr<UserConstPtr<c_char>>,
    envp: UserConstPtr<UserConstPtr<c_char>>,
) -> LinuxResult<isize> {
    let process = current().task_ext().process();
    let xprocess = process.data::<XProcess>().unwrap();
    let uspace = xprocess.uspace();
    let path = uspace.read_str(path)?.to_string();

    let mut args = uspace.read_str_array(argv)?;
    let envs = uspace.read_str_array(envp)?;

    // Add "./" prefix to the first arg if it doesn't start with "/" or "./"
    if let Some(first_arg) = args.get_mut(0) {
        *first_arg = path.clone();
    }

    info!(
        "sys_execve: path: {:?}, args: {:?}, envs: {:?}",
        path, args, envs
    );

    if process.threads().len() > 1 {
        // TODO: handle multi-thread case
        error!("sys_execve: multi-thread not supported");
        return Err(LinuxError::EAGAIN);
    }

    let (file_data, new_args) = load_file(Some(&path), &args)?;

    let mut aspace = uspace.aspace.lock();
    let loaded = load_app(&mut aspace, file_data, &new_args, &envs, false)?;
    let entry_point = loaded.entry;
    let user_stack_base = loaded.user_sp;
    drop(aspace);

    // Publish the new vDSO trampoline to this process's signal manager.
    xprocess
        .signal
        .set_default_restorer(loaded.vdso_rt_sigreturn.as_usize());

    let name = path
        .rsplit_once('/')
        .map_or(path.as_str(), |(_, name)| name);
    current().set_name(name);
    *xprocess.exe_path.write() = FS_CONTEXT.lock().canonicalize(path)?.to_string();

    // Close all file descriptors marked with FD_CLOEXEC
    FD_TABLE.close_on_exec();

    tf.set_ip(entry_point.as_usize());
    tf.set_sp(user_stack_base.as_usize());
    Ok(0)
}
