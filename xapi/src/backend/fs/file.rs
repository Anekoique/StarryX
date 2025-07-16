use alloc::sync::Arc;
use core::{any::Any, ffi::c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axfs_ng_vfs::{DeviceId, Location, Metadata};
use axhal::time::TimeValue;
use axio::{PollState, Read};
use axsync::{Mutex, MutexGuard, RawMutex};
use xcore::mm::PAGE_CACHE_MANAGER;

use super::{add_file_like, get_file_like};
use crate::ctypes::{stat, statx, statx_timestamp};

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

    fn add_to_fd_table(self, cloexec: bool) -> LinuxResult<c_int>
    where
        Self: Sized + 'static,
    {
        add_file_like(Arc::new(self), cloexec)
    }

    fn get_location(&self) -> Option<Location<RawMutex>> {
        None
    }
}

/// File wrapper for `axfs::fops::File`.
pub struct File {
    inner: Arc<Mutex<axfs_ng::FsFile<RawMutex>>>,
}

impl File {
    pub fn new(inner: axfs_ng::FsFile<RawMutex>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// Create a new File from an existing Arc<Mutex<axfs_ng::FsFile<RawMutex>>>
    pub fn from_shared(inner: Arc<Mutex<axfs_ng::FsFile<RawMutex>>>) -> Self {
        Self { inner }
    }

    /// Get the inner node of the file.
    pub fn inner(&self) -> MutexGuard<axfs_ng::FsFile<RawMutex>> {
        self.inner.lock()
    }

    /// Get a clone of the shared inner Arc
    pub fn clone_inner(&self) -> Arc<Mutex<axfs_ng::FsFile<RawMutex>>> {
        self.inner.clone()
    }

    /// Read a number of bytes starting from a given offset.
    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> LinuxResult<usize> {
        let inner = self.inner();
        if !inner.get_flags().contains(FileFlags::DIRECT)
            && let Some(cache) = PAGE_CACHE_MANAGER.get_cache(inner.inode()?)
        {
            cache.read_at(buf, offset)
        } else {
            inner.read_at(buf, offset)
        }
    }

    /// Write a number of bytes starting from a given offset.
    pub fn write_at(&self, buf: &[u8], offset: u64) -> LinuxResult<usize> {
        let mut inner = self.inner();
        if let Some(cache) = PAGE_CACHE_MANAGER.get_cache(inner.inode()?) {
            cache.write_at(buf, offset)
        } else {
            inner.write_at(buf, offset)
        }
    }
}

impl FileLike for File {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        let mut inner = self.inner();
        if let Some(cache) = PAGE_CACHE_MANAGER.get_cache(inner.inode()?) {
            let position = inner.position;
            cache
                .read_at(buf, position)
                .inspect(|n| inner.set_position(position + *n as u64))
        } else {
            Ok(inner.read(buf)?)
        }
    }

    fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        let mut inner = self.inner();
        if !inner.get_flags().contains(FileFlags::APPEND)
            && let Some(cache) = PAGE_CACHE_MANAGER.get_cache(inner.inode()?)
        {
            let position = inner.position;
            cache
                .write_at(buf, position)
                .inspect(|n| inner.set_position(position + *n as u64))
        } else {
            inner.write(buf)
        }
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

    fn from_fd(fd: c_int) -> LinuxResult<Arc<Self>> {
        get_file_like(fd)?
            .into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::EBADF)
    }

    fn get_location(&self) -> Option<Location<RawMutex>> {
        Some(self.inner().inner().clone())
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

    fn get_location(&self) -> Option<Location<RawMutex>> {
        Some(self.inner.clone())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Kstat {
    pub dev: u64,
    pub ino: u64,
    pub nlink: u32,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub blksize: u32,
    pub blocks: u64,
    pub rdev: DeviceId,
    pub atime: TimeValue,
    pub mtime: TimeValue,
    pub ctime: TimeValue,
}

impl Default for Kstat {
    fn default() -> Self {
        Self {
            dev: 0,
            ino: 1,
            nlink: 1,
            mode: 0,
            uid: 1,
            gid: 1,
            size: 0,
            blksize: 4096,
            blocks: 0,
            rdev: DeviceId::default(),
            atime: TimeValue::default(),
            mtime: TimeValue::default(),
            ctime: TimeValue::default(),
        }
    }
}

impl From<Kstat> for stat {
    fn from(value: Kstat) -> Self {
        // SAFETY: valid for stat
        let mut stat: stat = unsafe { core::mem::zeroed() };
        stat.st_dev = value.dev as _;
        stat.st_ino = value.ino as _;
        stat.st_nlink = value.nlink as _;
        stat.st_mode = value.mode as _;
        stat.st_uid = value.uid as _;
        stat.st_gid = value.gid as _;
        stat.st_size = value.size as _;
        stat.st_blksize = value.blksize as _;
        stat.st_blocks = value.blocks as _;
        stat.st_rdev = value.rdev.0 as _;

        stat.st_atime = value.atime.as_secs() as _;
        stat.st_atime_nsec = value.atime.subsec_nanos() as _;
        stat.st_mtime = value.mtime.as_secs() as _;
        stat.st_mtime_nsec = value.mtime.subsec_nanos() as _;
        stat.st_ctime = value.ctime.as_secs() as _;
        stat.st_ctime_nsec = value.ctime.subsec_nanos() as _;

        stat
    }
}

impl From<Kstat> for statx {
    fn from(value: Kstat) -> Self {
        // SAFETY: valid for statx
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
        statx.stx_rdev_major = value.rdev.major();
        statx.stx_rdev_minor = value.rdev.minor();

        fn time_to_statx(time: &TimeValue) -> statx_timestamp {
            statx_timestamp {
                tv_sec: time.as_secs() as _,
                tv_nsec: time.subsec_nanos() as _,
                __reserved: 0,
            }
        }
        statx.stx_atime = time_to_statx(&value.atime);
        statx.stx_ctime = time_to_statx(&value.ctime);
        statx.stx_mtime = time_to_statx(&value.mtime);

        statx.stx_dev_major = (value.dev >> 32) as _;
        statx.stx_dev_minor = value.dev as _;

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
        atime: metadata.atime,
        mtime: metadata.mtime,
        ctime: metadata.ctime,
    }
}
