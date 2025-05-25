use alloc::sync::Arc;
use core::{any::Any, ffi::c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{FS_CONTEXT, FsContext};
use axfs_ng_vfs::DeviceId;
use axfs_ng_vfs::{Location, Metadata};
use axio::{PollState, Read};
use axsync::{Mutex, MutexGuard, RawMutex};
use linux_raw_sys::general::{AT_EMPTY_PATH, AT_FDCWD, AT_SYMLINK_NOFOLLOW, stat, statx};

use super::{add_file_like, get_file_like};

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

pub enum ResolveAtResult {
    File(Location<RawMutex>),
    Other(Arc<dyn FileLike>),
}
impl ResolveAtResult {
    pub fn into_file(self) -> Option<Location<RawMutex>> {
        match self {
            Self::File(file) => Some(file),
            Self::Other(_) => None,
        }
    }

    pub fn stat(&self) -> LinuxResult<Kstat> {
        match self {
            Self::File(file) => file.metadata().map(|it| metadata_to_kstat(&it)),
            Self::Other(file_like) => file_like.stat(),
        }
    }
}

pub fn resolve_at(dirfd: c_int, path: Option<&str>, flags: u32) -> LinuxResult<ResolveAtResult> {
    match path {
        Some("") | None => {
            if flags & AT_EMPTY_PATH == 0 {
                return Err(LinuxError::ENOENT);
            }
            let file_like = get_file_like(dirfd)?;
            let f = file_like.clone().into_any();
            Ok(if let Some(file) = f.downcast_ref::<File>() {
                ResolveAtResult::File(file.inner().inner().clone())
            } else if let Some(dir) = f.downcast_ref::<Directory>() {
                ResolveAtResult::File(dir.inner().clone())
            } else {
                ResolveAtResult::Other(file_like)
            })
        }
        Some(path) => with_fs(dirfd, |fs| {
            if flags & AT_SYMLINK_NOFOLLOW != 0 {
                fs.resolve_no_follow(path)
            } else {
                fs.resolve(path)
            }
            .map(ResolveAtResult::File)
        }),
    }
}

#[allow(dead_code)]
pub trait FileLike: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize>;
    fn write(&self, buf: &[u8]) -> LinuxResult<usize>;
    fn stat(&self) -> LinuxResult<Kstat>;
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    fn poll(&self) -> LinuxResult<PollState>;
    fn set_nonblocking(&self, nonblocking: bool) -> LinuxResult;

    fn from_fd(fd: c_int) -> LinuxResult<Arc<Self>>
    where
        Self: Sized + 'static,
    {
        get_file_like(fd)?
            .into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::EINVAL)
    }

    fn add_to_fd_table(self) -> LinuxResult<c_int>
    where
        Self: Sized + 'static,
    {
        add_file_like(Arc::new(self))
    }
}

/// File wrapper for `axfs::fops::File`.
pub struct File {
    inner: Mutex<axfs_ng::File<RawMutex>>,
}

impl File {
    pub fn new(inner: axfs_ng::File<RawMutex>) -> Self {
        Self {
            inner: Mutex::new(inner),
        }
    }

    /// Get the inner node of the file.
    pub fn inner(&self) -> MutexGuard<axfs_ng::File<RawMutex>> {
        self.inner.lock()
    }
}

impl FileLike for File {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        Ok(self.inner().read(buf)?)
    }

    fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        self.inner().write(buf)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        Ok(metadata_to_kstat(&self.inner().metadata()?))
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
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

/// Directory wrapper for `axfs::fops::Directory`.
pub struct Directory {
    inner: Location<RawMutex>,
    pub offset: Mutex<u64>,
}

impl Directory {
    pub fn new(inner: Location<RawMutex>) -> Self {
        Self {
            inner,
            offset: Mutex::new(0),
        }
    }

    /// Get the inner node of the directory.
    pub fn inner(&self) -> &Location<RawMutex> {
        &self.inner
    }
}

impl FileLike for Directory {
    fn read(&self, _buf: &mut [u8]) -> LinuxResult<usize> {
        Err(LinuxError::EBADF)
    }

    fn write(&self, _buf: &[u8]) -> LinuxResult<usize> {
        Err(LinuxError::EBADF)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        Ok(metadata_to_kstat(&self.inner.metadata()?))
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
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

    fn from_fd(fd: c_int) -> LinuxResult<Arc<Self>> {
        get_file_like(fd)?
            .into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::ENOTDIR)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Kstat {
    pub dev: u64,
    pub ino: u64,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub size: u64,
    pub blocks: u64,
    pub blksize: u32,
    pub rdev: DeviceId,
    pub atime_sec: isize,
    pub atime_nsec: isize,
    pub mtime_sec: isize,
    pub mtime_nsec: isize,
    pub ctime_sec: isize,
    pub ctime_nsec: isize,
}

impl Default for Kstat {
    fn default() -> Self {
        Self {
            dev: 0,
            ino: 1,
            nlink: 1,
            uid: 1,
            gid: 1,
            mode: 0,
            size: 0,
            blocks: 0,
            blksize: 4096,
            rdev: DeviceId::default(),
            atime_sec: 0,
            atime_nsec: 0,
            mtime_sec: 0,
            mtime_nsec: 0,
            ctime_sec: 0,
            ctime_nsec: 0,
        }
    }
}

impl From<Kstat> for stat {
    fn from(value: Kstat) -> Self {
        // SAFETY: valid for stat
        let mut stat: stat = unsafe { core::mem::zeroed() };
        stat.st_dev = value.dev as _;
        stat.st_rdev = value.rdev.0 as _;
        stat.st_ino = value.ino as _;
        stat.st_nlink = value.nlink as _;
        stat.st_mode = value.mode as _;
        stat.st_uid = value.uid as _;
        stat.st_gid = value.gid as _;
        stat.st_size = value.size as _;
        stat.st_blksize = value.blksize as _;
        stat.st_blocks = value.blocks as _;
        stat.st_atime = value.atime_sec as _;
        stat.st_atime_nsec = value.atime_nsec as _;
        stat.st_mtime = value.mtime_sec as _;
        stat.st_mtime_nsec = value.mtime_nsec as _;
        stat.st_ctime = value.ctime_sec as _;
        stat.st_ctime_nsec = value.ctime_nsec as _;

        stat
    }
}

impl From<Kstat> for statx {
    fn from(value: Kstat) -> Self {
        let mut statx: statx = unsafe { core::mem::zeroed() };
        statx.stx_blksize = value.blksize as _;
        statx.stx_attributes = value.mode as _;
        statx.stx_nlink = value.nlink as _;
        statx.stx_uid = value.uid as _;
        statx.stx_gid = value.gid as _;
        statx.stx_mode = value.mode as _;
        statx.stx_ino = value.ino as _;
        statx.stx_size = value.size as _;
        statx.stx_blocks = value.blocks as _;
        statx.stx_atime.tv_sec = value.atime_sec as _;
        statx.stx_atime.tv_nsec = value.atime_nsec as _;
        statx.stx_mtime.tv_sec = value.mtime_sec as _;
        statx.stx_mtime.tv_nsec = value.mtime_nsec as _;
        statx.stx_ctime.tv_sec = value.ctime_sec as _;
        statx.stx_ctime.tv_nsec = value.ctime_nsec as _;

        statx
    }
}

pub fn metadata_to_kstat(metadata: &Metadata) -> Kstat {
    let ty = metadata.node_type as u8;
    let perm = metadata.mode.bits() as u32;
    let mode = ((ty as u32) << 12) | perm;
    Kstat {
        dev: metadata.device,
        ino: metadata.inode,
        mode,
        nlink: metadata.nlink as _,
        uid: metadata.uid,
        gid: metadata.gid,
        size: metadata.size,
        blksize: metadata.block_size as _,
        blocks: metadata.blocks,
        rdev: metadata.rdev,
        atime_sec: metadata.atime.as_secs() as isize,
        atime_nsec: metadata.atime.subsec_nanos() as isize,
        mtime_sec: metadata.mtime.as_secs() as isize,
        mtime_nsec: metadata.mtime.subsec_nanos() as isize,
        ctime_sec: metadata.ctime.as_secs() as isize,
        ctime_nsec: metadata.ctime.subsec_nanos() as isize,
    }
}
