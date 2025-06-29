use core::{
    ffi::{c_char, c_int},
    panic,
};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{OpenOptions, OpenResult};
use axsync::RawMutex;

use crate::{
    ctypes::{
        __kernel_mode_t, AT_FDCWD, F_DUPFD, F_DUPFD_CLOEXEC, F_GETFD, F_GETFL, F_SETFD, F_SETFL,
        FD_CLOEXEC, O_APPEND, O_CLOEXEC, O_CREAT, O_DIRECTORY, O_EXCL, O_NONBLOCK, O_PATH,
        O_RDONLY, O_TRUNC, O_WRONLY,
    },
    fs::{
        Directory, FD_TABLE, File, FileLike, add_file_like, close_file_like, get_file_like, with_fs,
    },
    ptr::UserConstPtr,
    sys_getegid, sys_geteuid,
};

const O_EXEC: u32 = O_PATH;

/// Convert open flags to [`OpenOptions`].
fn flags_to_options(flags: c_int, mode: __kernel_mode_t, (uid, gid): (u32, u32)) -> OpenOptions {
    let flags = flags as u32;
    let mut options = OpenOptions::new();
    options.mode(mode).user(uid, gid);
    match flags & 0b11 {
        O_RDONLY => options.read(true),
        O_WRONLY => options.write(true),
        _ => options.read(true).write(true),
    };
    if flags & O_APPEND != 0 {
        options.append(true);
    }
    if flags & O_TRUNC != 0 {
        options.truncate(true);
    }
    if flags & O_CREAT != 0 {
        options.create(true);
    }
    if flags & O_EXEC != 0 {
        options.execute(true);
    }
    if flags & O_EXCL != 0 {
        options.create_new(true);
    }
    if flags & O_DIRECTORY != 0 {
        options.directory(true);
    }
    if flags & O_CLOEXEC != 0 {
        options.cloexec(true);
    }
    options
}

fn add_to_fd(result: OpenResult<RawMutex>, cloexec: bool) -> LinuxResult<i32> {
    match result {
        OpenResult::File(file) => File::new(file).add_to_fd_table(cloexec),
        OpenResult::Dir(dir) => Directory::new(dir).add_to_fd_table(cloexec),
    }
}

/// Open or create a file.
/// fd: file descriptor
/// filename: file path to be opened or created
/// flags: open flags
/// mode: see man 7 inode
/// return new file descriptor if succeed, or return -1.
pub fn sys_openat(
    dirfd: c_int,
    path: UserConstPtr<c_char>,
    flags: i32,
    mode: __kernel_mode_t,
) -> LinuxResult<isize> {
    let path = path.get_as_str()?;
    debug!(
        "sys_openat <= {} {:?} {:#o} {:#o}",
        dirfd, path, flags, mode
    );

    let options = flags_to_options(flags, mode, (sys_geteuid()? as _, sys_getegid()? as _));
    with_fs(dirfd, path, |fs| fs.open(&options, path))
        .and_then(|result| add_to_fd(result, options.cloexec))
        .map(|fd| fd as isize)
}

/// Open a file by `filename` and insert it into the file descriptor table.
///
/// Return its index in the file table (`fd`). Return `EMFILE` if it already
/// has the maximum number of files open.
pub fn sys_open(
    path: UserConstPtr<c_char>,
    flags: i32,
    mode: __kernel_mode_t,
) -> LinuxResult<isize> {
    sys_openat(AT_FDCWD as _, path, flags, mode)
}

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

pub fn sys_dup(old_fd: c_int) -> LinuxResult<isize> {
    debug!("sys_dup <= {}", old_fd);
    dup_fd(old_fd)
}

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
