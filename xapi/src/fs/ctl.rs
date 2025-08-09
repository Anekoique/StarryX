use alloc::ffi::CString;
use core::{
    ffi::{c_char, c_int, c_void},
    mem::offset_of,
};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{FS_CONTEXT, FileFlags};
use axfs_ng_vfs::{MetadataUpdate, NodePermission, NodeType, path::Path};

use xcore::{
    fs::{
        fd::{Directory, File, get_file_like},
        file::FileLike,
        vfs::VirtDevice,
        with_fs, with_location,
    },
    task::with_uspace,
};
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::{
    ctypes::{
        AT_EMPTY_PATH, AT_FDCWD, AT_REMOVEDIR, UTIME_NOW, UTIME_OMIT, linux_dirent64, sys::utimbuf,
        timespec, timeval,
    },
    time::{TimeValue, TimeValueLike, wall_time},
};

/// The ioctl() system call manipulates the underlying device parameters
/// of special files.
///
/// # Arguments
/// * `fd` - The file descriptor
/// * `op` - The request code. It is of type unsigned long in glibc and BSD,
///   and of type int in musl and other UNIX systems.
/// * `argp` - The argument to the request. It is a pointer to a memory location
pub fn sys_ioctl(fd: i32, op: usize, argp: UserPtr<c_void>) -> LinuxResult<isize> {
    trace!("sys_ioctl <= fd: {}, op: {}, argp: {:?}", fd, op, argp);
    get_file_like(fd)?
        .into_any()
        .downcast::<File>()
        .and_then(|file| {
            file.inner()
                .get_file_node()
                .into_any()
                .downcast::<VirtDevice>()
        })
        .map_err(|_| LinuxError::ENOTTY)?
        .inner()
        .ioctl(op, argp)
}

/// Change the current working directory.
///
/// # Arguments
/// * `path` - Path to the new working directory
pub fn sys_chdir(path: UserConstPtr<c_char>) -> LinuxResult<isize> {
    let path = with_uspace(|uspace| uspace.read_str(path))?;

    trace!("sys_chdir <= path: {}", path);
    with_fs(AT_FDCWD, path, |fs| {
        let entry = fs.resolve(path)?;
        fs.set_current_dir(entry)
    })?;
    Ok(0)
}

/// Change to the directory represented by the given file descriptor
///
/// # Arguments
/// * `fd` - File descriptor
pub fn sys_fchdir(fd: i32) -> LinuxResult<isize> {
    trace!("sys_fchdir <= fd: {}", fd);
    let dir = Directory::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?
        .inner()
        .clone();
    FS_CONTEXT.lock().set_current_dir(dir)?;
    Ok(0)
}

/// Create a directory.
///
/// # Arguments
/// * `path` - Path where to create the directory
/// * `mode` - Directory permissions
pub fn sys_mkdir(path: UserConstPtr<c_char>, mode: u32) -> LinuxResult<isize> {
    sys_mkdirat(AT_FDCWD, path, mode)
}

/// Create a directory relative to a directory file descriptor.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor
/// * `path` - Path where to create the directory
/// * `mode` - Directory permissions
pub fn sys_mkdirat(dirfd: i32, path: UserConstPtr<c_char>, mode: u32) -> LinuxResult<isize> {
    let path = with_uspace(|uspace| uspace.read_str(path))?;
    let mode = NodePermission::from_bits(mode as u16).ok_or(LinuxError::EINVAL)?;

    trace!(
        "sys_mkdirat <= dirfd: {}, path: {}, mode: {:?}",
        dirfd, path, mode
    );
    with_fs(dirfd, path, |fs| fs.create_dir(path, mode))?;
    Ok(0)
}

// Directory buffer for getdents64 syscall
struct DirBuffer<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> DirBuffer<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, offset: 0 }
    }

    fn remaining_space(&self) -> usize {
        self.buf.len().saturating_sub(self.offset)
    }

    fn write_entry(&mut self, d_ino: u64, d_off: i64, d_type: NodeType, name: &[u8]) -> bool {
        const NAME_OFFSET: usize = offset_of!(linux_dirent64, d_name);

        let len = NAME_OFFSET + name.len() + 1;
        let len = len.next_multiple_of(align_of::<linux_dirent64>());
        if self.remaining_space() < len {
            return false;
        }

        unsafe {
            let entry_ptr = self.buf.as_mut_ptr().add(self.offset);
            entry_ptr.cast::<linux_dirent64>().write(linux_dirent64 {
                d_ino,
                d_off,
                d_reclen: len as _,
                d_type: d_type as _,
                d_name: Default::default(),
            });

            let name_ptr = entry_ptr.add(NAME_OFFSET);
            name_ptr.copy_from_nonoverlapping(name.as_ptr(), name.len());
            name_ptr.add(name.len()).write(0);
        }

        self.offset += len;
        true
    }
}

/// Get directory entries.
///
/// # Arguments
/// * `fd` - Directory file descriptor
/// * `buf` - Buffer to store directory entries
/// * `len` - Buffer length
pub fn sys_getdents64(fd: i32, buf: UserPtr<u8>, len: usize) -> LinuxResult<isize> {
    let buf = with_uspace(|uspace| uspace.raw_slice(buf, len))?;
    trace!(
        "sys_getdents64 <= fd: {}, buf: {:p}, len: {}",
        fd,
        buf.as_ptr(),
        buf.len()
    );

    let mut buffer = DirBuffer::new(buf);

    let dir = Directory::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?;
    let mut dir_offset = dir.offset.lock();

    dir.inner()
        .read_dir(*dir_offset, &mut |name: &str, ino, node_type, offset| {
            if !buffer.write_entry(ino, offset as _, node_type, name.as_bytes()) {
                return false;
            }
            *dir_offset = offset;
            true
        })?;
    if buffer.offset == 0 {
        return Err(LinuxError::EINVAL);
    }
    Ok(buffer.offset as _)
}

/// Create a hard link relative to directory file descriptors.
///
/// # Arguments
/// * `old_dirfd` - Directory file descriptor for the old path
/// * `old_path` - Path to the existing file
/// * `new_dirfd` - Directory file descriptor for the new path
/// * `new_path` - Path for the new link
/// * `flags` - Link flags
pub fn sys_linkat(
    old_dirfd: c_int,
    old_path: UserConstPtr<c_char>,
    new_dirfd: c_int,
    new_path: UserConstPtr<c_char>,
    flags: u32,
) -> LinuxResult<isize> {
    let (old_path, new_path) = with_uspace(|uspace| -> LinuxResult<_> {
        let old_path = nullable!(uspace.read_str(old_path))?;
        let new_path = uspace.read_str(new_path)?;
        Ok((old_path, new_path))
    })?;
    trace!(
        "sys_linkat <= old_dirfd: {}, old_path: {:?}, new_dirfd: {}, new_path: {}, flags: {}",
        old_dirfd, old_path, new_dirfd, new_path, flags
    );
    let (new_dir, new_name) = with_fs(new_dirfd, new_path, |fs| {
        fs.resolve_nonexistent(new_path.into())
    })?;

    with_location(old_dirfd, old_path, flags, |location| {
        if flags != 0 {
            warn!("Unsupported flags: {flags}");
        }
        if location.is_dir() {
            return Err(LinuxError::EPERM);
        }
        new_dir.link(new_name, location)
    })?;
    Ok(0)
}

/// Create a hard link.
///
/// # Arguments
/// * `old_path` - Path to the existing file
/// * `new_path` - Path for the new link
pub fn sys_link(
    old_path: UserConstPtr<c_char>,
    new_path: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    sys_linkat(AT_FDCWD, old_path, AT_FDCWD, new_path, 0)
}

/// Remove a file or directory relative to a directory file descriptor.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor
/// * `path` - Path to the file or directory to remove
/// * `flags` - Flags (0 for file, AT_REMOVEDIR for directory)
pub fn sys_unlinkat(dirfd: i32, path: UserConstPtr<c_char>, flags: usize) -> LinuxResult<isize> {
    let path = with_uspace(|uspace| uspace.read_str(path))?;
    trace!(
        "sys_unlinkat <= dirfd: {}, path: {:?}, flags: {}",
        dirfd, path, flags
    );

    with_fs(dirfd, path, |fs| {
        if flags == AT_REMOVEDIR as _ {
            fs.remove_dir(path)
        } else {
            fs.remove_file(path)
        }
    })?;
    Ok(0)
}

/// Remove a directory.
///
/// # Arguments
/// * `path` - Path to the directory to remove
pub fn sys_rmdir(path: UserConstPtr<c_char>) -> LinuxResult<isize> {
    sys_unlinkat(AT_FDCWD, path, AT_REMOVEDIR as _)
}

/// Remove a file.
///
/// # Arguments
/// * `path` - Path to the file to remove
pub fn sys_unlink(path: UserConstPtr<c_char>) -> LinuxResult<isize> {
    sys_unlinkat(AT_FDCWD, path, 0)
}

/// Get current working directory.
///
/// # Arguments
/// * `buf` - Buffer to store the current working directory path
/// * `size` - Size of the buffer
pub fn sys_getcwd(buf: UserPtr<u8>, size: isize) -> LinuxResult<isize> {
    let size: usize = size.try_into().map_err(|_| LinuxError::EFAULT)?;
    let buf = with_uspace(|uspace| nullable!(uspace.raw_slice(buf, size)))?;

    let Some(buf) = buf else {
        return Ok(0);
    };

    with_fs(AT_FDCWD, ".", |fs| {
        let cwd = fs.current_dir().absolute_path()?;

        let cwd = CString::new(cwd.as_str()).map_err(|_| LinuxError::EINVAL)?;
        let cwd = cwd.as_bytes_with_nul();

        if cwd.len() <= buf.len() {
            buf[..cwd.len()].copy_from_slice(cwd);
            Ok(buf.as_ptr() as isize)
        } else {
            Err(LinuxError::ERANGE)
        }
    })
}

/// Create a symbolic link.
///
/// # Arguments
/// * `target` - Target path the symbolic link points to
/// * `linkpath` - Path where the symbolic link will be created
pub fn sys_symlink(
    target: UserConstPtr<c_char>,
    linkpath: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    sys_symlinkat(target, AT_FDCWD, linkpath)
}

/// Create a symbolic link relative to a directory file descriptor.
///
/// # Arguments
/// * `target` - Target path the symbolic link points to
/// * `new_dirfd` - Directory file descriptor
/// * `linkpath` - Path where the symbolic link will be created
pub fn sys_symlinkat(
    target: UserConstPtr<c_char>,
    new_dirfd: i32,
    linkpath: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    let (target, linkpath) = with_uspace(|uspace| -> LinuxResult<_> {
        let target = uspace.read_str(target)?;
        let linkpath = uspace.read_str(linkpath)?;
        Ok((target, linkpath))
    })?;

    trace!(
        "sys_symlinkat <= target: {}, new_dirfd: {}, linkpath: {}",
        target, new_dirfd, linkpath
    );
    with_fs(new_dirfd, linkpath, |fs| fs.symlink(target, linkpath))?;
    Ok(0)
}

/// Read value of a symbolic link.
///
/// # Arguments
/// * `path` - Path to the symbolic link
/// * `buf` - Buffer to store the link target
/// * `size` - Size of the buffer
pub fn sys_readlink(
    path: UserConstPtr<c_char>,
    buf: UserPtr<u8>,
    size: usize,
) -> LinuxResult<isize> {
    sys_readlinkat(AT_FDCWD, path, buf, size)
}

/// Read value of a symbolic link relative to a directory file descriptor.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor
/// * `path` - Path to the symbolic link
/// * `buf` - Buffer to store the link target
/// * `size` - Size of the buffer
pub fn sys_readlinkat(
    dirfd: i32,
    path: UserConstPtr<c_char>,
    buf: UserPtr<u8>,
    size: usize,
) -> LinuxResult<isize> {
    let (path, buf) = with_uspace(|uspace| -> LinuxResult<_> {
        let path = uspace.read_str(path)?;
        let buf = uspace.raw_slice(buf, size)?;
        Ok((path, buf))
    })?;

    with_fs(dirfd, path, |fs| {
        let entry = fs.resolve_no_follow(path)?;
        let link = entry.read_link()?;
        trace!("sys_readlinkat => link: {}", link);
        let read = size.min(link.len());
        buf[..read].copy_from_slice(&link.as_bytes()[..read]);
        Ok(read as isize)
    })
}

/// Change ownership of a file.
///
/// # Arguments
/// * `path` - Path to the file
/// * `uid` - New user ID
/// * `gid` - New group ID
pub fn sys_chown(path: UserConstPtr<c_char>, uid: i32, gid: i32) -> LinuxResult<isize> {
    sys_fchownat(AT_FDCWD, path, uid, gid, 0)
}
/// Change ownership of a symbolic link itself.
///
/// # Arguments
/// * `path` - Path to the symbolic link
/// * `uid` - New user ID
/// * `gid` - New group ID
pub fn sys_lchown(path: UserConstPtr<c_char>, uid: i32, gid: i32) -> LinuxResult<isize> {
    use linux_raw_sys::general::AT_SYMLINK_NOFOLLOW;
    sys_fchownat(AT_FDCWD, path, uid, gid, AT_SYMLINK_NOFOLLOW)
}

/// Change ownership of a file by file descriptor.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `uid` - New user ID
/// * `gid` - New group ID
pub fn sys_fchown(fd: i32, uid: i32, gid: i32) -> LinuxResult<isize> {
    sys_fchownat(fd, 0.into(), uid, gid, AT_EMPTY_PATH)
}

/// Change ownership of a file relative to a directory file descriptor.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor
/// * `path` - Path to the file
/// * `uid` - New user ID
/// * `gid` - New group ID
/// * `flags` - Control flags
pub fn sys_fchownat(
    dirfd: i32,
    path: UserConstPtr<c_char>,
    uid: i32,
    gid: i32,
    flags: u32,
) -> LinuxResult<isize> {
    let path = with_uspace(|uspace| nullable!(uspace.read_str(path)))?;
    let uid = if uid < 0 { 0 } else { uid as u32 };
    let gid = if gid < 0 { 0 } else { gid as u32 };

    with_location(dirfd, path, flags, |location| {
        location.update_metadata(MetadataUpdate {
            owner: Some((uid, gid)),
            ..Default::default()
        })
    })?;
    Ok(0)
}

/// Change file permissions.
///
/// # Arguments
/// * `path` - Path to the file
/// * `mode` - New permission mode
pub fn sys_chmod(path: UserConstPtr<c_char>, mode: u32) -> LinuxResult<isize> {
    sys_fchmodat(AT_FDCWD, path, mode, 0)
}

/// Change file permissions by file descriptor.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `mode` - New permission mode
pub fn sys_fchmod(fd: i32, mode: u32) -> LinuxResult<isize> {
    sys_fchmodat(fd, 0.into(), mode, AT_EMPTY_PATH)
}

/// Change file permissions relative to a directory file descriptor.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor
/// * `path` - Path to the file
/// * `mode` - New permission mode
/// * `flags` - Control flags
pub fn sys_fchmodat(
    dirfd: i32,
    path: UserConstPtr<c_char>,
    mode: u32,
    flags: u32,
) -> LinuxResult<isize> {
    let path = with_uspace(|uspace| nullable!(uspace.read_str(path)))?;
    with_location(dirfd, path, flags, |location| {
        location.update_metadata(MetadataUpdate {
            mode: Some(NodePermission::from_bits(mode as u16).ok_or(LinuxError::EINVAL)?),
            ..Default::default()
        })
    })?;
    Ok(0)
}

fn update_times(
    dirfd: i32,
    path: UserConstPtr<c_char>,
    atime: Option<TimeValue>,
    mtime: Option<TimeValue>,
    flags: u32,
) -> LinuxResult<()> {
    let path = with_uspace(|uspace| nullable!(uspace.read_str(path)))?;
    with_location(dirfd, path, flags, |location| {
        location.update_metadata(MetadataUpdate {
            atime,
            mtime,
            ..Default::default()
        })
    })?;
    Ok(())
}

/// Change file access and modification times.
///
/// # Arguments
/// * `path` - Path to the file
/// * `times` - New access and modification times (NULL for current time)
pub fn sys_utime(path: UserConstPtr<c_char>, times: UserConstPtr<utimbuf>) -> LinuxResult<isize> {
    let times = with_uspace(|uspace| nullable!(uspace.read(times)))?;
    let atime = times.map_or_else(wall_time, |it| TimeValue::from_secs(it.actime as _));
    let mtime = times.map_or_else(wall_time, |it| TimeValue::from_secs(it.modtime as _));
    update_times(AT_FDCWD, path, Some(atime), Some(mtime), 0)?;
    Ok(0)
}

/// Change file access and modification times with microsecond precision.
///
/// # Arguments
/// * `path` - Path to the file
/// * `times` - Array of two timeval structures for access and modification times (NULL for current time)
pub fn sys_utimes(path: UserConstPtr<c_char>, times: UserConstPtr<timeval>) -> LinuxResult<isize> {
    let times = with_uspace(|uspace| nullable!(uspace.read_slice(times, 2)))?;
    let (atime, mtime) = match times {
        Some(times) => (
            timeval::to_time_value(times[0])?,
            timeval::to_time_value(times[1])?,
        ),
        None => (wall_time(), wall_time()),
    };
    update_times(AT_FDCWD, path, Some(atime), Some(mtime), 0)?;
    Ok(0)
}

/// Change file access and modification times with nanosecond precision.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor
/// * `path` - Path to the file (NULL to use dirfd as file descriptor)
/// * `times` - Array of two timespec structures for access and modification times (NULL for current time)
/// * `flags` - Control flags
pub fn sys_utimensat(
    dirfd: i32,
    path: UserConstPtr<c_char>,
    times: UserConstPtr<timespec>,
    mut flags: u32,
) -> LinuxResult<isize> {
    if path.is_null() {
        flags |= AT_EMPTY_PATH;
    }
    fn utime_to_duration(time: &timespec) -> LinuxResult<Option<TimeValue>> {
        match time.tv_nsec {
            val if val == UTIME_OMIT as _ => Ok(None),
            val if val == UTIME_NOW as _ => Ok(Some(wall_time())),
            _ => Ok(Some(timespec::to_time_value(*time)?)),
        }
    }
    let times = with_uspace(|uspace| nullable!(uspace.read_slice(times, 2)))?;
    let (atime, mtime) = match times {
        Some([atime, mtime]) => (utime_to_duration(atime)?, utime_to_duration(mtime)?),
        None => (Some(wall_time()), Some(wall_time())),
        _ => unreachable!(),
    };
    if atime.is_none() && mtime.is_none() {
        return Ok(0);
    }
    update_times(dirfd, path, atime, mtime, flags)?;
    Ok(0)
}

/// Rename a file.
///
/// # Arguments
/// * `old_path` - Current path of the file
/// * `new_path` - New path for the file
pub fn sys_rename(
    old_path: UserConstPtr<c_char>,
    new_path: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    sys_renameat(AT_FDCWD, old_path, AT_FDCWD, new_path)
}

/// Rename a file relative to directory file descriptors.
///
/// # Arguments
/// * `old_dirfd` - Directory file descriptor for the old path
/// * `old_path` - Current path of the file
/// * `new_dirfd` - Directory file descriptor for the new path
/// * `new_path` - New path for the file
pub fn sys_renameat(
    old_dirfd: i32,
    old_path: UserConstPtr<c_char>,
    new_dirfd: i32,
    new_path: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    sys_renameat2(old_dirfd, old_path, new_dirfd, new_path, 0)
}

/// Rename a file relative to directory file descriptors with flags.
///
/// # Arguments
/// * `old_dirfd` - Directory file descriptor for the old path
/// * `old_path` - Current path of the file
/// * `new_dirfd` - Directory file descriptor for the new path
/// * `new_path` - New path for the file
/// * `flags` - Rename flags
pub fn sys_renameat2(
    old_dirfd: i32,
    old_path: UserConstPtr<c_char>,
    new_dirfd: i32,
    new_path: UserConstPtr<c_char>,
    flags: u32,
) -> LinuxResult<isize> {
    let (old_path, new_path) = with_uspace(|uspace| -> LinuxResult<_> {
        let old_path = uspace.read_str(old_path)?;
        let new_path = uspace.read_str(new_path)?;
        Ok((old_path, new_path))
    })?;
    trace!(
        "sys_renameat2 <= old_dirfd: {}, old_path: {:?}, new_dirfd: {}, new_path: {}, flags: {}",
        old_dirfd, old_path, new_dirfd, new_path, flags
    );

    let (old_dir, old_name) = with_fs(old_dirfd, old_path, |fs| {
        fs.resolve_parent(Path::new(old_path))
    })?;
    let (new_dir, new_name) = with_fs(new_dirfd, new_path, |fs| {
        fs.resolve_nonexistent(new_path.into())
    })?;

    old_dir.rename(&old_name, &new_dir, new_name)?;
    Ok(0)
}
