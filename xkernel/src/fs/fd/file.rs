use alloc::sync::Arc;
use core::{any::Any, ffi::c_int};

use xerrno::{LinuxError, LinuxResult};
use xfs::{FileFlags, FsFile};
use xio::{PollState, Read, SeekFrom};
use xsync::{Mutex, MutexGuard, RawMutex};
use xvfs::Location;

use xcache::FileMapping;

use xutils::ctypes::fs::{Kstat, metadata_to_kstat};

use crate::fs::{cache::CachedMapping, fd::get_file_like, file::FileLike};

/// File wrapper for `xfs::fops::File`.
pub struct File {
    inner: Arc<Mutex<FsFile<RawMutex>>>,
    /// Open access mode, immutable after open, so positionless I/O checks it
    /// without taking the file lock.
    access: FileFlags,
    cache: Option<CachedMapping>,
}

impl File {
    pub fn new(inner: FsFile<RawMutex>) -> LinuxResult<Self> {
        Self::from_shared(Arc::new(Mutex::new(inner)))
    }

    /// Create a new File from an existing Arc<Mutex<xfs::FsFile<RawMutex>>>
    pub fn from_shared(inner: Arc<Mutex<FsFile<RawMutex>>>) -> LinuxResult<Self> {
        let (access, mapping) = {
            let file = inner.lock();
            let node = file.get_file_node();
            if file.get_flags().contains(FileFlags::DIRECT) && node.cache_slot().is_some() {
                return Err(LinuxError::EOPNOTSUPP);
            }
            (
                file.get_flags() & (FileFlags::READ | FileFlags::WRITE),
                crate::fs::cache::mapping_for(node)?,
            )
        };
        Ok(Self {
            inner,
            access,
            cache: mapping.map(CachedMapping::new),
        })
    }

    fn check_access(&self, required: FileFlags) -> LinuxResult<()> {
        if self.access.contains(required) {
            Ok(())
        } else {
            Err(LinuxError::EBADF)
        }
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
        self.check_access(FileFlags::READ)?;
        match &self.cache {
            Some(cache) => cache.mapping().read_at(buf, offset),
            None => self.inner().read_at(buf, offset),
        }
    }

    /// Write a number of bytes starting from a given offset.
    pub fn write_at(&self, buf: &[u8], offset: u64) -> LinuxResult<usize> {
        self.check_access(FileFlags::WRITE)?;
        match &self.cache {
            Some(cache) => cache.mapping().write_at(buf, offset),
            None => self.inner().write_at(buf, offset),
        }
    }

    pub fn set_len(&self, len: u64) -> LinuxResult<()> {
        self.check_access(FileFlags::WRITE)?;
        match &self.cache {
            Some(cache) => cache.mapping().resize(len),
            None => self.inner().set_len(len),
        }
    }

    pub fn sync(&self, data_only: bool) -> LinuxResult<()> {
        match &self.cache {
            Some(cache) => cache.sync_range(0..cache.mapping().size(), data_only, true),
            None => self.inner().sync(data_only),
        }
    }

    pub fn is_empty(&self) -> LinuxResult<bool> {
        Ok(self.len()? == 0)
    }

    pub fn mapping(&self) -> Option<Arc<FileMapping>> {
        self.cache.as_ref().map(|cache| cache.mapping().clone())
    }

    /// Overrides [`xio::Seek`] so `SEEK_END` sees cached extends: the backing
    /// length is stale while the mapping holds unflushed writes.
    pub fn seek(&self, position: SeekFrom) -> LinuxResult<u64> {
        let mut file = self.inner();
        let next = match position {
            SeekFrom::Start(position) => position,
            SeekFrom::End(offset) => self
                .cache
                .as_ref()
                .map_or_else(
                    || file.access(FileFlags::empty())?.len(),
                    |cache| Ok(cache.mapping().size()),
                )?
                .checked_add_signed(offset)
                .ok_or(LinuxError::EINVAL)?,
            SeekFrom::Current(offset) => file
                .position
                .checked_add_signed(offset)
                .ok_or(LinuxError::EINVAL)?,
        };
        file.set_position(next);
        Ok(next)
    }
}

impl FileLike for File {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        let mut file = self.inner();
        file.access(FileFlags::READ)?;
        let read = match &self.cache {
            Some(cache) => cache.mapping().read_at(buf, file.position)?,
            None => return Ok(file.read(buf)?),
        };
        file.position += read as u64;
        Ok(read)
    }

    fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        let mut file = self.inner();
        file.access(FileFlags::WRITE)?;
        let Some(cache) = &self.cache else {
            return file.write(buf);
        };
        let (written, position) = if file.get_flags().contains(FileFlags::APPEND) {
            cache.mapping().append(buf)?
        } else {
            let written = cache.mapping().write_at(buf, file.position)?;
            (written, file.position + written as u64)
        };
        file.position = position;
        Ok(written)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        let mut stat = metadata_to_kstat(&self.inner().metadata()?);
        if let Some(cache) = &self.cache {
            stat.size = cache.mapping().size();
        }
        Ok(stat)
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
        match &self.cache {
            Some(cache) => Ok(cache.mapping().size()),
            None => self.inner().len(),
        }
    }
}

/// Directory wrapper for `xfs::fops::Directory`.
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
