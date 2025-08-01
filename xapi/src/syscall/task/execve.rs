use alloc::{format, string::ToString};
use core::ffi::c_char;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FS_CONTEXT;
use axhal::arch::TrapFrame;
use axtask::{TaskExtRef, current};

use axuspace::{UserConstPtr, UserSpaceAccess};
use xcore::{
    fs::FD_TABLE,
    mm::{load_app, load_file},
    task::XProcess,
};

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
    let mut path = uspace.read_str(path)?.to_string();

    // Add "./" prefix if path doesn't start with "/" or "./"
    if !path.starts_with('/') && !path.starts_with("./") {
        path = format!("./{}", path);
    }

    let mut args = uspace.read_str_array(argv)?;
    let envs = uspace.read_str_array(envp)?;

    // Add "./" prefix to the first arg if it doesn't start with "/" or "./"
    if let Some(first_arg) = args.get_mut(0) {
        if !first_arg.starts_with('/') && !first_arg.starts_with("./") {
            *first_arg = format!("./{}", first_arg);
        }
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
    let (entry_point, user_stack_base) = load_app(&mut aspace, file_data, &new_args, &envs, false)?;
    uspace.vma_manager.write().clear();
    drop(aspace);

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
