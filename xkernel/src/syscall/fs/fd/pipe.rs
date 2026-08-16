use core::ffi::c_int;

use xerrno::LinuxResult;
use xfs::FileFlags;

use crate::{
    fs::{
        fd::{Pipe, close_file_like},
        file::FileLike,
    },
    task::with_uspace,
};
use xuspace::UserPtr;
use xutils::ctypes::{O_CLOEXEC, O_NONBLOCK};

/// Create a pipe with optional flags.
///
/// # Arguments
/// * `fds` - Array to store the read and write file descriptors
/// * `flags` - Pipe creation flags
pub fn sys_pipe2(fds: UserPtr<[c_int; 2]>, flags: i32) -> LinuxResult<isize> {
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

    let result = [read_fd, write_fd];
    with_uspace(|uspace| uspace.write(fds, result))?;
    debug!("sys_pipe2 <= fds: {:?}", result);
    Ok(0)
}
