use alloc::sync::Arc;
use core::{any::Any, ffi::c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axfs_ng_vfs::Location;
use axio::{PollState, Read};
use axsync::{Mutex, MutexGuard, RawMutex};

use xcore::{
    fs::{FileLike, Kstat, get_file_like, metadata_to_kstat},
    mm::PAGE_CACHE_MANAGER,
};

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

    pub fn len(&self) -> LinuxResult<u64> {
        let inner = self.inner();
        Ok(PAGE_CACHE_MANAGER
            .get_cache(inner.inode()?)
            .map(|cache| cache.get_size() as u64)
            .unwrap_or(inner.len()?))
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

    fn from_fd(fd: c_int, required: FileFlags, forbidden: FileFlags) -> LinuxResult<Arc<Self>> {
        get_file_like(fd)?
            .validate(required, forbidden)?
            .clone()
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

    fn from_fd(fd: c_int, required: FileFlags, forbidden: FileFlags) -> LinuxResult<Arc<Self>> {
        get_file_like(fd)?
            .validate(required, forbidden)?
            .clone()
            .into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::ENOTDIR)
    }

    fn get_location(&self) -> Option<Location<RawMutex>> {
        Some(self.inner.clone())
    }
}
