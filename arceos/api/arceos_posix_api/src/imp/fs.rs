use alloc::sync::Arc;
use axfs_ng::{FS_CONTEXT, FsContext, OpenOptions, OpenResult};
use axfs_ng_vfs::{DirEntry, Location, Metadata};
use core::ffi::{c_char, c_int};

use axerrno::{LinuxError, LinuxResult};
use axio::{PollState, Read, Seek, SeekFrom};
use axsync::{Mutex, RawMutex};

use super::fd_ops::{FileLike, get_file_like};
use crate::{ctypes, utils::char_ptr_to_str};

pub const AT_FDCWD: c_int = -100;

pub fn with_fs<R>(
    dirfd: c_int,
    f: impl FnOnce(&mut FsContext<RawMutex>) -> LinuxResult<R>,
) -> LinuxResult<R> {
    let mut fs = FS_CONTEXT.lock();
    if dirfd == AT_FDCWD {
        f(&mut fs)
    } else {
        let dir = Directory::from_fd(dirfd)?.inner.clone();
        f(&mut fs.with_current_dir(dir)?)
    }
}

/// File wrapper for `axfs::fops::File`.
pub struct File {
    inner: Mutex<axfs_ng::File<RawMutex>>,
}

impl File {
    fn new(inner: axfs_ng::File<RawMutex>) -> Self {
        Self {
            inner: Mutex::new(inner),
        }
    }

    fn add_to_fd_table(self) -> LinuxResult<c_int> {
        super::fd_ops::add_file_like(Arc::new(self))
    }

    fn from_fd(fd: c_int) -> LinuxResult<Arc<Self>> {
        let f = super::fd_ops::get_file_like(fd)?;
        f.into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::EINVAL)
    }

    /// Get the inner node of the file.
    pub fn inner(&self) -> &Mutex<axfs_ng::File<RawMutex>> {
        &self.inner
    }
}

fn duration_to_timespec(duration: core::time::Duration) -> ctypes::timespec {
    ctypes::timespec {
        tv_sec: duration.as_secs() as i64,
        tv_nsec: duration.subsec_nanos() as i64,
    }
}

impl FileLike for File {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        Ok(self.inner.lock().read(buf)?)
    }

    fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        Ok(self.inner.lock().write(buf)?)
    }

    fn stat(&self) -> LinuxResult<ctypes::stat> {
        Ok(metadata_to_stat(&self.inner.lock().metadata()?))
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        Ok(PollState {
            readable: true,
            writable: true,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) -> LinuxResult {
        Ok(())
    }
}

/// Convert open flags to [`OpenOptions`].
fn flags_to_options(flags: c_int, _mode: ctypes::mode_t) -> OpenOptions {
    let flags = flags as u32;
    let mut options = OpenOptions::new();
    match flags & 0b11 {
        ctypes::O_RDONLY => options.read(true),
        ctypes::O_WRONLY => options.write(true),
        _ => options.read(true).write(true),
    };
    if flags & ctypes::O_APPEND != 0 {
        options.append(true);
    }
    if flags & ctypes::O_TRUNC != 0 {
        options.truncate(true);
    }
    if flags & ctypes::O_CREAT != 0 {
        options.create(true);
    }
    if flags & ctypes::O_EXEC != 0 {
        options.execute(true);
    }
    if flags & ctypes::O_EXCL != 0 {
        options.create_new(true);
    }
    if flags & ctypes::O_DIRECTORY != 0 {
        options.directory(true);
    }
    options
}

fn add_to_fd(result: OpenResult<RawMutex>) -> LinuxResult<c_int> {
    match result {
        OpenResult::File(file) => {
            let file = File::new(file);
            let fd = file.add_to_fd_table()?;
            Ok(fd)
        }
        OpenResult::Dir(dir) => {
            let dir = Directory::new(dir);
            let fd = dir.add_to_fd_table()?;
            Ok(fd)
        }
    }
}

pub fn metadata_to_stat(metadata: &Metadata) -> ctypes::stat {
    let ty = metadata.node_type as u8;
    let perm = metadata.mode.bits() as u32;
    let st_mode = ((ty as u32) << 12) | perm;
    ctypes::stat {
        st_dev: metadata.device,
        st_ino: metadata.inode,
        st_mode,
        st_nlink: metadata.nlink as _,
        st_uid: metadata.uid,
        st_gid: metadata.gid,
        st_rdev: 0,
        st_size: metadata.size as _,
        st_blksize: metadata.block_size as _,
        st_blocks: metadata.blocks as _,
        st_atime: duration_to_timespec(metadata.atime),
        st_mtime: duration_to_timespec(metadata.mtime),
        st_ctime: duration_to_timespec(metadata.ctime),
    }
}

/// Open a file by `filename` and insert it into the file descriptor table.
///
/// Return its index in the file table (`fd`). Return `EMFILE` if it already
/// has the maximum number of files open.
pub fn sys_open(filename: *const c_char, flags: c_int, mode: ctypes::mode_t) -> c_int {
    let filename = char_ptr_to_str(filename);
    debug!("sys_open <= {:?} {:#o} {:#o}", filename, flags, mode);
    let options = flags_to_options(flags, mode);
    filename
        .and_then(|filename| {
            options
                .open(&*FS_CONTEXT.lock(), filename)
                .map_err(Into::into)
        })
        .and_then(add_to_fd)
        .unwrap_or_else(|e| -(e as i32))
}

/// Open or create a file.
/// fd: file descriptor
/// filename: file path to be opened or created
/// flags: open flags
/// mode: see man 7 inode
/// return new file descriptor if succeed, or return -1.
pub fn sys_openat(dirfd: c_int, name: *const c_char, flags: c_int, mode: ctypes::mode_t) -> c_int {
    let name = match char_ptr_to_str(name) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    debug!(
        "sys_openat <= {} {:?} {:#o} {:#o}",
        dirfd, name, flags, mode
    );

    syscall_body!(sys_openat, {
        let options = flags_to_options(flags, mode);
        with_fs(dirfd, |fs| Ok(options.open(fs, name)?)).and_then(add_to_fd)
    })
}

/// Set the position of the file indicated by `fd`.
///
/// Return its position after seek.
pub fn sys_lseek(fd: c_int, offset: ctypes::off_t, whence: c_int) -> ctypes::off_t {
    debug!("sys_lseek <= {} {} {}", fd, offset, whence);
    syscall_body!(sys_lseek, {
        let pos = match whence {
            0 => SeekFrom::Start(offset as _),
            1 => SeekFrom::Current(offset as _),
            2 => SeekFrom::End(offset as _),
            _ => return Err(LinuxError::EINVAL),
        };
        let off = File::from_fd(fd)?.inner.lock().seek(pos)?;
        Ok(off)
    })
}

/// Get the file metadata by `path` and write into `buf`.
///
/// Return 0 if success.
pub unsafe fn sys_stat(path: *const c_char, buf: *mut ctypes::stat) -> c_int {
    let path = char_ptr_to_str(path);
    debug!("sys_stat <= {:?} {:#x}", path, buf as usize);
    syscall_body!(sys_stat, {
        if buf.is_null() {
            return Err(LinuxError::EFAULT);
        }
        let mut options = OpenOptions::new();
        options.read(true);
        let file = OpenOptions::new()
            .read(true)
            .open(&*FS_CONTEXT.lock(), path?)?
            .into_file()?;
        let st = File::new(file).stat()?;
        unsafe { *buf = st };
        Ok(0)
    })
}

/// Get file metadata by `fd` and write into `buf`.
///
/// Return 0 if success.
pub unsafe fn sys_fstat(fd: c_int, buf: *mut ctypes::stat) -> c_int {
    debug!("sys_fstat <= {} {:#x}", fd, buf as usize);
    syscall_body!(sys_fstat, {
        if buf.is_null() {
            return Err(LinuxError::EFAULT);
        }

        unsafe { *buf = get_file_like(fd)?.stat()? };
        Ok(0)
    })
}

/// Get the metadata of the symbolic link and write into `buf`.
///
/// Return 0 if success.
pub unsafe fn sys_lstat(path: *const c_char, buf: *mut ctypes::stat) -> ctypes::ssize_t {
    let path = char_ptr_to_str(path);
    debug!("sys_lstat <= {:?} {:#x}", path, buf as usize);
    syscall_body!(sys_lstat, {
        if buf.is_null() {
            return Err(LinuxError::EFAULT);
        }
        unsafe { *buf = Default::default() }; // TODO
        Ok(0)
    })
}

/// Get the path of the current directory.
pub fn sys_getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
    debug!("sys_getcwd <= {:#x} {}", buf as usize, size);
    syscall_body!(sys_getcwd, {
        if buf.is_null() {
            return Ok(core::ptr::null::<c_char>() as _);
        }
        let dst = unsafe { core::slice::from_raw_parts_mut(buf as *mut u8, size as _) };
        let cwd = FS_CONTEXT.lock().current_dir().absolute_path()?;
        let cwd = cwd.as_bytes();
        if cwd.len() < size {
            dst[..cwd.len()].copy_from_slice(cwd);
            dst[cwd.len()] = 0;
            Ok(buf)
        } else {
            Err(LinuxError::ERANGE)
        }
    })
}

/// Rename `old` to `new`
/// If new exists, it is first removed.
///
/// Return 0 if the operation succeeds, otherwise return -1.
pub fn sys_rename(old: *const c_char, new: *const c_char) -> c_int {
    syscall_body!(sys_rename, {
        let old_path = char_ptr_to_str(old)?;
        let new_path = char_ptr_to_str(new)?;
        debug!("sys_rename <= old: {:?}, new: {:?}", old_path, new_path);
        FS_CONTEXT.lock().rename(old_path, new_path)?;
        Ok(0)
    })
}

/// Directory wrapper for `axfs::fops::Directory`.
pub struct Directory {
    inner: Location<RawMutex>,
    pub offset: Mutex<u64>,
}

impl Directory {
    fn new(inner: Location<RawMutex>) -> Self {
        Self {
            inner,
            offset: Mutex::new(0),
        }
    }

    pub fn inner(&self) -> &Location<RawMutex> {
        &self.inner
    }

    fn add_to_fd_table(self) -> LinuxResult<c_int> {
        super::fd_ops::add_file_like(Arc::new(self))
    }

    /// Open a directory by `fd`.
    pub fn from_fd(fd: c_int) -> LinuxResult<Arc<Self>> {
        let f = super::fd_ops::get_file_like(fd)?;
        f.into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::EINVAL)
    }
}

impl FileLike for Directory {
    fn read(&self, _buf: &mut [u8]) -> LinuxResult<usize> {
        Err(LinuxError::EBADF)
    }

    fn write(&self, _buf: &[u8]) -> LinuxResult<usize> {
        Err(LinuxError::EBADF)
    }

    fn stat(&self) -> LinuxResult<ctypes::stat> {
        Err(LinuxError::EBADF)
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        Ok(PollState {
            readable: true,
            writable: false,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) -> LinuxResult {
        Ok(())
    }
}
