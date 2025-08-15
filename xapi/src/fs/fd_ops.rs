use core::ffi::{c_char, c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{FileFlags, OpenResult};
use axfs_ng_vfs::NodePermission;
use axsync::{Mutex, RawMutex};

use xcore::{
    fs::{
        fd::{Directory, DummyFd, FD_TABLE, File, add_file_like, close_file_like, get_file_like},
        file::FileLike,
        vfs::is_virtual_fs,
        with_fs,
    },
    mm::{InodeWrapper, PAGE_CACHE_MANAGER},
    task::with_uspace,
};
use xuspace::{UserConstPtr, UserSpaceAccess};
use xutils::ctypes::{
    __kernel_mode_t, AT_FDCWD, F_DUPFD, F_DUPFD_CLOEXEC, F_GETFD, F_GETFL, F_SETFD, F_SETFL,
    F_SETLK, FD_CLOEXEC, O_CLOEXEC, O_NONBLOCK, O_RDWR, O_WRONLY, fs::flags_to_options,
};

use crate::task::{sys_getegid, sys_geteuid};

fn add_to_fd(
    path: &str,
    flags: FileFlags,
    result: OpenResult<RawMutex>,
    cloexec: bool,
) -> LinuxResult<i32> {
    match result {
        OpenResult::File(file) => {
            if !is_virtual_fs(path) {
                PAGE_CACHE_MANAGER.get_or_create(InodeWrapper(Mutex::new(file.get_file_node())));
            }
            File::new(file).add_to_fd_table(flags, cloexec)
        }
        OpenResult::Dir(dir) => Directory::new(dir).add_to_fd_table(flags, cloexec),
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
    let path = with_uspace(|uspace| uspace.read_str(path))?;
    debug!(
        "sys_openat <= {} {:?} {:#o} {:#o}",
        dirfd, path, flags, mode
    );

    PAGE_CACHE_MANAGER.clear_stale_cache();
    let options = flags_to_options(flags, mode, (sys_geteuid()? as _, sys_getegid()? as _));
    with_fs(dirfd, path, |fs| fs.open(&options, path))
        .and_then(|result| {
            add_to_fd(
                path,
                options.to_flags()?,
                result,
                flags as u32 & O_CLOEXEC != 0,
            )
        })
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
    PAGE_CACHE_MANAGER.clear_stale_cache();
    Ok(0)
}

fn dup_fd(old_fd: c_int) -> LinuxResult<isize> {
    let f = get_file_like(old_fd)?;
    let new_fd = add_file_like(f.file.clone(), f.flags, false)?;
    Ok(new_fd as _)
}

/// Duplicate a file descriptor.
///
/// # Arguments
/// * `old_fd` - File descriptor to duplicate
pub fn sys_dup(old_fd: c_int) -> LinuxResult<isize> {
    trace!("sys_dup <= {}", old_fd);
    dup_fd(old_fd)
}

/// Duplicate a file descriptor to a specific file descriptor number.
///
/// # Arguments
/// * `old_fd` - File descriptor to duplicate
/// * `new_fd` - Target file descriptor number
pub fn sys_dup2(old_fd: c_int, new_fd: c_int) -> LinuxResult<isize> {
    trace!("sys_dup2 <= old_fd: {}, new_fd: {}", old_fd, new_fd);
    let f = FD_TABLE.get(old_fd as _).ok_or(LinuxError::EBADF)?;

    if old_fd != new_fd {
        FD_TABLE.remove(new_fd as _);
        FD_TABLE
            .add_at(new_fd as _, f)
            .map_err(|_| LinuxError::EBADF)?;
    }

    Ok(new_fd as _)
}

pub fn sys_dup3(old_fd: c_int, new_fd: c_int, flags: c_int) -> LinuxResult<isize> {
    trace!(
        "sys_dup3 <= old_fd: {}, new_fd: {}, flags: {}",
        old_fd, new_fd, flags
    );
    if flags < 0 {
        return Err(LinuxError::EINVAL);
    }
    if old_fd == new_fd {
        return Err(LinuxError::EINVAL);
    }
    sys_dup2(old_fd, new_fd)?;
    FD_TABLE.set_cloexec(new_fd as usize, flags == O_CLOEXEC as _);
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
    if !FD_TABLE.is_assigned(fd as usize) {
        return Err(LinuxError::EBADF);
    }

    match cmd as u32 {
        F_DUPFD => dup_fd(fd),
        F_DUPFD_CLOEXEC => {
            let new_fd = dup_fd(fd)?;
            // Set CLOEXEC flag for the new fd
            FD_TABLE.set_cloexec(new_fd as usize, true);
            Ok(new_fd)
        }
        F_SETFL => {
            // if fd == 0 || fd == 1 || fd == 2 {
            //     return Ok(0);
            // }
            get_file_like(fd)?.set_nonblocking(arg & (O_NONBLOCK as usize) > 0);
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
            let f = get_file_like(fd)?;
            let perm = NodePermission::from_bits_truncate(f.stat()?.mode as _);

            let mut ret = 0;

            if f.is_nonblocking() {
                ret |= O_NONBLOCK;
            }

            ret |= match (
                perm.contains(NodePermission::OWNER_READ),
                perm.contains(NodePermission::OWNER_WRITE),
            ) {
                (true, true) => O_RDWR,
                (false, true) => O_WRONLY,
                _ => 0,
            };

            Ok(ret as _)
        }
        F_SETLK => Err(LinuxError::EINVAL),
        _ => {
            warn!("unsupported fcntl parameters: cmd: {}", cmd);
            Ok(0)
        }
    }
}

pub fn sys_dummy_fd() -> LinuxResult<isize> {
    DummyFd
        .add_to_fd_table(FileFlags::READ | FileFlags::WRITE, false)
        .map(|fd| fd as isize)
}
