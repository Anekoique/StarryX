use core::ffi::{c_char, c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{FS_CONTEXT, FileFlags};
use axfs_ng_vfs::{Location, NodePermission};
use axsync::RawMutex;

use xcore::{
    fs::{fd::get_file_like, with_file, with_location},
    mm::PAGE_CACHE_MANAGER,
    task::{with_uspace, with_xprocess},
};
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::ctypes::{
    __kernel_fsid_t, AT_EMPTY_PATH, AT_FDCWD, AT_SYMLINK_FOLLOW,
    fs::{AccessMode, metadata_to_kstat},
    stat, statfs, statx,
};

/// Get file metadata by path and write into statbuf.
///
/// # Arguments
/// * `path` - Path to the file
/// * `statbuf` - Buffer to write file metadata
pub fn sys_stat(path: UserConstPtr<c_char>, statbuf: UserPtr<stat>) -> LinuxResult<isize> {
    sys_fstatat(AT_FDCWD, path, statbuf, 0)
}

/// Get file metadata by file descriptor and write into statbuf.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `statbuf` - Buffer to write file metadata
pub fn sys_fstat(fd: i32, statbuf: UserPtr<stat>) -> LinuxResult<isize> {
    sys_fstatat(fd, 0.into(), statbuf, AT_EMPTY_PATH)
}

/// Get symbolic link metadata and write into statbuf.
///
/// # Arguments
/// * `path` - Path to the symbolic link
/// * `statbuf` - Buffer to write file metadata
pub fn sys_lstat(path: UserConstPtr<c_char>, statbuf: UserPtr<stat>) -> LinuxResult<isize> {
    sys_fstatat(AT_FDCWD, path, statbuf, AT_SYMLINK_FOLLOW)
}

/// Get file metadata relative to a directory file descriptor.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor
/// * `path` - Path to the file
/// * `statbuf` - Buffer to write file metadata
/// * `flags` - Flags controlling the operation
pub fn sys_fstatat(
    dirfd: i32,
    path: UserConstPtr<c_char>,
    statbuf: UserPtr<stat>,
    flags: u32,
) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        let path = nullable!(uspace.read_str(path))?;

        trace!(
            "sys_fstatat <= dirfd: {}, path: {:?}, flags: {}",
            dirfd, path, flags
        );

        uspace.write(
            statbuf,
            with_location(dirfd, path, flags, |location| {
                PAGE_CACHE_MANAGER.sync_file(location.inode())?;
                location
                    .metadata()
                    .map(|metadata| metadata_to_kstat(&metadata))
            })
            .or_else(|err| {
                if err == LinuxError::EBADF {
                    get_file_like(dirfd)?.stat().map_err(|_| err)
                } else {
                    Err(err)
                }
            })?
            .into(),
        )?;

        Ok(0)
    })
}

/// Get extended file metadata.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor
/// * `path` - Path to the file
/// * `flags` - Flags controlling the operation
/// * `_mask` - Mask specifying which fields to return (currently unused)
/// * `statxbuf` - Buffer to write extended file metadata
pub fn sys_statx(
    dirfd: c_int,
    path: UserConstPtr<c_char>,
    flags: u32,
    _mask: u32,
    statxbuf: UserPtr<statx>,
) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        let path = nullable!(uspace.read_str(path))?;
        trace!(
            "sys_statx <= dirfd: {}, path: {:?}, flags: {}",
            dirfd, path, flags
        );

        uspace.write(
            statxbuf,
            with_location(dirfd, path, flags, |location| {
                PAGE_CACHE_MANAGER.sync_file(location.inode())?;
                location
                    .metadata()
                    .map(|metadata| metadata_to_kstat(&metadata))
            })
            .or_else(|err| {
                if err == LinuxError::EBADF {
                    get_file_like(dirfd)?.stat().map_err(|_| err)
                } else {
                    Err(err)
                }
            })?
            .into(),
        )?;

        Ok(0)
    })
}

/// Check file accessibility.
///
/// # Arguments
/// * `path` - Path to the file
/// * `mode` - Access mode to check (R_OK, W_OK, X_OK)
pub fn sys_access(path: UserConstPtr<c_char>, mode: u32) -> LinuxResult<isize> {
    sys_faccessat2(AT_FDCWD, path, mode, 0)
}

/// Check file accessibility relative to a directory file descriptor.
///
/// # Arguments
/// * `dirfd` - Directory file descriptor
/// * `path` - Path to the file
/// * `mode` - Access mode to check (R_OK, W_OK, X_OK)
/// * `flags` - Flags controlling the operation
pub fn sys_faccessat2(
    dirfd: c_int,
    path: UserConstPtr<c_char>,
    mode: u32,
    flags: u32,
) -> LinuxResult<isize> {
    let access_mode = AccessMode::from_bits(mode).ok_or(LinuxError::EINVAL)?;
    let path = with_uspace(|uspace| nullable!(uspace.read_str(path)))?;

    let mut required_mode = NodePermission::empty();
    if access_mode.contains(AccessMode::R_OK) {
        required_mode |= NodePermission::OWNER_READ;
    }
    if access_mode.contains(AccessMode::W_OK) {
        required_mode |= NodePermission::OWNER_WRITE;
    }
    if access_mode.contains(AccessMode::X_OK) {
        required_mode |= NodePermission::OWNER_EXEC;
    }
    debug!(
        "dirfd: {}, path: {:?}, mode: {:?}, flags: {:?}",
        dirfd, path, mode, flags
    );
    let required_mode = required_mode.bits();

    let file_mode = with_location(dirfd, path, flags, |location| {
        location
            .metadata()
            .map(|metadata| metadata_to_kstat(&metadata))
    })
    .or_else(|err| {
        if err == LinuxError::EBADF {
            get_file_like(dirfd)?.stat().map_err(|_| err)
        } else {
            Err(err)
        }
    })?
    .mode;

    let uid = with_xprocess(|process| process.uid());
    if (file_mode as u16 & required_mode) != required_mode
        && (access_mode.contains(AccessMode::X_OK) || uid != 0)
    {
        return Err(LinuxError::EACCES);
    }

    Ok(0)
}

fn statfs(loc: &Location<RawMutex>, buf: UserPtr<statfs>) -> LinuxResult<()> {
    let stat = loc.filesystem().stat()?;
    let dest = with_uspace(|uspace| uspace.raw_ptr(buf))?;
    dest.f_type = stat.fs_type as _;
    dest.f_bsize = stat.block_size as _;
    dest.f_blocks = stat.blocks as _;
    dest.f_bfree = stat.blocks_free as _;
    dest.f_bavail = stat.blocks_available as _;
    dest.f_files = stat.file_count as _;
    dest.f_ffree = stat.free_file_count as _;
    // TODO: fsid
    dest.f_fsid = __kernel_fsid_t {
        val: [0, loc.mountpoint().device() as _],
    };
    dest.f_namelen = stat.name_length as _;
    dest.f_frsize = stat.fragment_size as _;
    dest.f_flags = stat.mount_flags as _;
    Ok(())
}

/// Get filesystem statistics by path.
///
/// # Arguments
/// * `path` - Path to a file in the filesystem
/// * `buf` - Buffer to write filesystem statistics
pub fn sys_statfs(path: UserConstPtr<c_char>, buf: UserPtr<statfs>) -> LinuxResult<isize> {
    statfs(
        &FS_CONTEXT
            .lock()
            .resolve(with_uspace(|uspace| uspace.read_str(path))?)?
            .mountpoint()
            .root_location(),
        buf,
    )?;
    Ok(0)
}

/// Get filesystem statistics by file descriptor.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `buf` - Buffer to write filesystem statistics
pub fn sys_fstatfs(fd: i32, buf: UserPtr<statfs>) -> LinuxResult<isize> {
    with_file(fd, FileFlags::empty(), FileFlags::empty(), |file| {
        statfs(file.inner().inner(), buf)
    })
    .map(|_| 0)
}
