use alloc::sync::Arc;
use core::ffi::c_int;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use xprocess::Pid;
use xsignal::SignalInfo;

use xcore::{
    fs::{
        fd::{FD_TABLE, PidFd, add_file_like},
        file::FileLike,
    },
    task::{XProcess, get_thread, send_signal_thread},
};
use xuspace::UserPtr;
use xutils::ctypes::PIDFD_NONBLOCK;

use crate::task::make_queue_signal_info;

/// Open a file descriptor for a process.
///
/// # Arguments
/// * `pid` - Process ID to open
/// * `flags` - Currently must be 0
pub fn sys_pidfd_open(pid: Pid, flags: u32) -> LinuxResult<isize> {
    debug!("sys_pidfd_open <= pid: {}, flags: {:#x}", pid, flags);
    if flags != 0 && flags != PIDFD_NONBLOCK {
        return Err(LinuxError::EINVAL);
    }
    let thread = get_thread(pid)?;

    let pidfd = PidFd::new(pid, Arc::downgrade(&thread), flags & PIDFD_NONBLOCK != 0);
    let fd = pidfd.add_to_fd_table(FileFlags::READ | FileFlags::WRITE, true)?;

    Ok(fd as isize)
}

/// Get a file descriptor from another process via pidfd.
///
/// # Arguments
/// * `pidfd` - File descriptor referring to a process
/// * `targetfd` - File descriptor to get from the target process
/// * `flags` - Currently must be 0
pub fn sys_pidfd_getfd(pidfd: c_int, targetfd: c_int, flags: u32) -> LinuxResult<isize> {
    if flags != 0 {
        return Err(LinuxError::EINVAL);
    }

    let pid_file = PidFd::from_fd(pidfd, FileFlags::READ | FileFlags::WRITE, FileFlags::PATH)?;
    if !pid_file.is_alive() {
        return Err(LinuxError::ESRCH);
    }

    let thread = pid_file.get_thread()?;
    let xfile = FD_TABLE
        .deref_from(&XProcess::from_thread(&thread).ns)
        .get(targetfd as _)
        .ok_or(LinuxError::EBADF)?;
    let new_fd = add_file_like(xfile.file.clone(), xfile.flags, true)?;
    Ok(new_fd as isize)
}

/// Send a signal to a process via pidfd.
///
/// # Arguments
/// * `pidfd` - File descriptor referring to a process
/// * `sig` - Signal number to send
/// * `info` - Signal info (currently unused, should be null)
/// * `flags` - Currently must be 0
pub fn sys_pidfd_send_signal(
    pidfd: c_int,
    sig: u32,
    info: UserPtr<SignalInfo>,
    flags: u32,
) -> LinuxResult<isize> {
    if flags != 0 {
        return Err(LinuxError::EINVAL);
    }

    let pid_file = PidFd::from_fd(pidfd, FileFlags::READ | FileFlags::WRITE, FileFlags::PATH)?;
    if !pid_file.is_alive() {
        return Err(LinuxError::ESRCH);
    }

    let info = make_queue_signal_info(pid_file.pid(), sig, info)?;
    send_signal_thread(pid_file.get_thread()?.as_ref(), info)?;
    Ok(0)
}
