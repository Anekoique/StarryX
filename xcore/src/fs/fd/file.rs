use alloc::sync::Arc;
use core::{any::Any, ffi::c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{FileFlags, FsFile};
use axfs_ng_vfs::Location;
use axio::{PollState, Read};
use axsync::{Mutex, MutexGuard, RawMutex};

use xutils::ctypes::fs::{Kstat, metadata_to_kstat};

use crate::{
    fs::{fd::get_file_like, file::FileLike},
    mm::PAGE_CACHE_MANAGER,
};

/// File wrapper for `axfs::fops::File`.
pub struct File {
    inner: Arc<Mutex<FsFile<RawMutex>>>,
}

impl File {
    pub fn new(inner: FsFile<RawMutex>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// Create a new File from an existing Arc<Mutex<axfs_ng::FsFile<RawMutex>>>
    pub fn from_shared(inner: Arc<Mutex<FsFile<RawMutex>>>) -> Self {
        Self { inner }
    }

    /// Get the inner node of the file.
    pub fn inner(&self) -> MutexGuard<'_, FsFile<RawMutex>> {
        self.inner.lock()
    }

    /// Get a clone of the shared inner Arc
    pub fn clone_inner(&self) -> Arc<Mutex<FsFile<RawMutex>>> {
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
        PAGE_CACHE_MANAGER
            .get_cache(inner.inode()?)
            .map(|cache| cache.write_at(buf, offset))
            .unwrap_or_else(|| inner.write_at(buf, offset))
    }

    pub fn set_len(&self, len: u64) -> LinuxResult<()> {
        let inner = self.inner();
        PAGE_CACHE_MANAGER
            .get_cache(inner.inode()?)
            .map(|cache| cache.evict_from_pos(len as _))
            .unwrap_or(inner.set_len(len))
    }

    pub fn sync(&self, data_only: bool) -> LinuxResult<()> {
        let inner = self.inner();
        PAGE_CACHE_MANAGER
            .get_cache(inner.inode()?)
            .map(|cache| cache.sync())
            .unwrap_or(Ok(()))
            .and_then(|_| inner.sync(data_only))
    }

    pub fn is_empty(&self) -> LinuxResult<bool> {
        Ok(self.len()? == 0)
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

    fn set_nonblocking(&self, _nonblocking: bool) {}

    fn from_fd(fd: c_int, required: FileFlags, forbidden: FileFlags) -> LinuxResult<Arc<Self>> {
        let file = get_file_like(fd)?
            .validate(required, forbidden)?
            .clone()
            .into_any();

        file.downcast::<Self>().map_err(|any| {
            if any.is::<Directory>() {
                LinuxError::EISDIR
            } else {
                LinuxError::ESPIPE
            }
        })
    }

    fn get_location(&self) -> Option<Location<RawMutex>> {
        Some(self.inner().inner().clone())
    }

    fn len(&self) -> LinuxResult<u64> {
        let inner = self.inner();
        Ok(PAGE_CACHE_MANAGER
            .get_cache(inner.inode()?)
            .map(|cache| cache.get_size())
            .unwrap_or(inner.len()?))
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

    pub fn inode(&self) -> u64 {
        self.inner.inode()
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

    fn set_nonblocking(&self, _nonblocking: bool) {}

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
