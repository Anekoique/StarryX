use core::ffi::c_int;

use axerrno::LinuxResult;
use axuspace::{UserPtr, UserSpaceAccess};
use xcore::task::with_uspace;

use crate::{
    ctypes::O_CLOEXEC,
    fs::{FileLike, Pipe, close_file_like},
};

/// Create a pipe with optional flags.
///
/// # Arguments
/// * `fds` - Array to store the read and write file descriptors
/// * `flags` - Pipe creation flags
pub fn sys_pipe2(fds: UserPtr<[c_int; 2]>, flags: i32) -> LinuxResult<isize> {
    if flags != 0 {
        warn!("sys_pipe2: unsupported flags: {}", flags);
    }

    let fds = with_uspace(|uspace| uspace.raw_ptr(fds))?;

    let (read_end, write_end) = Pipe::new();
    let read_fd = read_end.add_to_fd_table(flags as u32 & O_CLOEXEC != 0)?;
    let write_fd = write_end
        .add_to_fd_table(flags as u32 & O_CLOEXEC != 0)
        .inspect_err(|_| close_file_like(read_fd).unwrap())?;

    fds[0] = read_fd;
    fds[1] = write_fd;

    info!("sys_pipe2 <= fds: {:?}", fds);
    Ok(0)
}
