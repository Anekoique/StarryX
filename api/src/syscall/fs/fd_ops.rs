use core::{
    ffi::{c_char, c_int},
    panic,
};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::OpenResult;
use axsync::RawMutex;
use axtask::current;
use axuspace::{UserConstPtr, UserSpace};
use starry_core::task::TaskExt;

use crate::{
    ctypes::{
        __kernel_mode_t, AT_FDCWD, F_DUPFD, F_DUPFD_CLOEXEC, F_GETFD, F_GETFL, F_SETFD, F_SETFL,
        FD_CLOEXEC, O_NONBLOCK, fs::flags_to_options,
    },
    fs::{
        Directory, FD_TABLE, File, FileLike, add_file_like, close_file_like, get_file_like, with_fs,
    },
    sys_getegid, sys_geteuid,
};

fn add_to_fd(result: OpenResult<RawMutex>, cloexec: bool) -> LinuxResult<i32> {
    match result {
        OpenResult::File(file) => File::new(file).add_to_fd_table(cloexec),
        OpenResult::Dir(dir) => Directory::new(dir).add_to_fd_table(cloexec),
    }
}

/// Open or create a file relative to a directory file descriptor.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor (AT_FDCWD for current working directory)
/// * `path` - Path to the file to open
/// * `flags` - Open flags controlling how the file is opened
/// * `mode` - File mode for newly created files
pub fn sys_openat(
    dirfd: c_int,
    path: UserConstPtr<c_char>,
    flags: i32,
    mode: __kernel_mode_t,
) -> LinuxResult<isize> {
    let uspace = UserSpace::new(TaskExt::from_task(&current()).process_data());
    let path = uspace.read_str(path)?;
    debug!(
        "sys_openat <= {} {:?} {:#o} {:#o}",
        dirfd, path, flags, mode
    );

    let options = flags_to_options(flags, mode, (sys_geteuid()? as _, sys_getegid()? as _));
    with_fs(dirfd, path, |fs| fs.open(&options, path))
        .and_then(|result| add_to_fd(result, options.cloexec))
        .map(|fd| fd as isize)
}

/// Open a file by filename and insert it into the file descriptor table.
///
/// # Arguments
/// * `path` - Path to the file to open
/// * `flags` - Open flags controlling how the file is opened
/// * `mode` - File mode for newly created files
pub fn sys_open(
    path: UserConstPtr<c_char>,
    flags: i32,
    mode: __kernel_mode_t,
) -> LinuxResult<isize> {
    sys_openat(AT_FDCWD as _, path, flags, mode)
}

/// Close a file descriptor.
///
/// # Arguments
/// * `fd` - File descriptor to close
pub fn sys_close(fd: c_int) -> LinuxResult<isize> {
    debug!("sys_close <= {}", fd);
    close_file_like(fd)?;
    Ok(0)
}

fn dup_fd(old_fd: c_int) -> LinuxResult<isize> {
    let f = get_file_like(old_fd)?;
    let new_fd = add_file_like(f, false)?;
    Ok(new_fd as _)
}

/// Duplicate a file descriptor.
///
/// # Arguments
/// * `old_fd` - File descriptor to duplicate
pub fn sys_dup(old_fd: c_int) -> LinuxResult<isize> {
    debug!("sys_dup <= {}", old_fd);
    dup_fd(old_fd)
}

/// Duplicate a file descriptor to a specific file descriptor number.
///
/// # Arguments
/// * `old_fd` - File descriptor to duplicate
/// * `new_fd` - Target file descriptor number
pub fn sys_dup2(old_fd: c_int, new_fd: c_int) -> LinuxResult<isize> {
    debug!("sys_dup2 <= old_fd: {}, new_fd: {}", old_fd, new_fd);
    let f = FD_TABLE.get(old_fd as _).ok_or(LinuxError::EBADF)?;

    if old_fd != new_fd {
        FD_TABLE.remove(new_fd as _);
        FD_TABLE
            .add_at(new_fd as _, f)
            .unwrap_or_else(|_| panic!("new_fd should be valid"));
    }

    Ok(new_fd as _)
}

/// Manipulate file descriptor properties.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `cmd` - Command to execute
/// * `arg` - Command argument
pub fn sys_fcntl(fd: c_int, cmd: c_int, arg: usize) -> LinuxResult<isize> {
    debug!("sys_fcntl <= fd: {} cmd: {} arg: {}", fd, cmd, arg);

    match cmd as u32 {
        F_DUPFD => dup_fd(fd),
        F_DUPFD_CLOEXEC => {
            let new_fd = dup_fd(fd)?;
            // Set CLOEXEC flag for the new fd
            FD_TABLE.set_cloexec(new_fd as usize, true);
            Ok(new_fd)
        }
        F_SETFL => {
            if fd == 0 || fd == 1 || fd == 2 {
                return Ok(0);
            }
            get_file_like(fd)?.set_nonblocking(arg & (O_NONBLOCK as usize) > 0)?;
            Ok(0)
        }
        F_GETFD => {
            // Get file descriptor flags
            Ok(FD_TABLE.has_cloexec(fd as usize) as isize)
        }
        F_SETFD => {
            // Set file descriptor flags
            FD_TABLE.set_cloexec(fd as usize, arg & (FD_CLOEXEC as usize) != 0);
            Ok(0)
        }
        F_GETFL => {
            warn!("unsupported fcntl parameters: F_GETFL, returning O_NONBLOCK");
            Ok(O_NONBLOCK as _)
        }
        _ => {
            warn!("unsupported fcntl parameters: cmd: {}", cmd);
            Ok(0)
        }
    }
}
