use core::ffi::c_int;

use axerrno::LinuxResult;
use axfs_ng::FileFlags;

use xcore::{
    fs::{
        fd::{Pipe, close_file_like},
        file::FileLike,
    },
    task::with_uspace,
};
use xuspace::{UserPtr, UserSpaceAccess};
use xutils::ctypes::{O_CLOEXEC, O_NONBLOCK};

/// Create a pipe with optional flags.
///
/// # Arguments
/// * `fds` - Array to store the read and write file descriptors
/// * `flags` - Pipe creation flags
pub fn sys_pipe2(fds: UserPtr<[c_int; 2]>, flags: i32) -> LinuxResult<isize> {
    let fds = with_uspace(|uspace| uspace.raw_ptr(fds))?;
    let fate_flags = FileFlags::READ | FileFlags::WRITE;

    let (read_end, write_end) = Pipe::new();
    if flags as u32 & O_NONBLOCK != 0 {
        read_end.set_nonblocking(true);
        write_end.set_nonblocking(true);
    }
    let read_fd = read_end.add_to_fd_table(fate_flags, flags as u32 & O_CLOEXEC != 0)?;
    let write_fd = write_end
        .add_to_fd_table(fate_flags, flags as u32 & O_CLOEXEC != 0)
        .inspect_err(|_| close_file_like(read_fd).unwrap())?;

    fds[0] = read_fd;
    fds[1] = write_fd;

    debug!("sys_pipe2 <= fds: {:?}", fds);
    Ok(0)
}
