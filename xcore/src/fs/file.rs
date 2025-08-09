use alloc::sync::Arc;
use core::{any::Any, ffi::c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axfs_ng_vfs::Location;
use axio::PollState;
use axsync::RawMutex;
use inherit_methods_macro::inherit_methods;

use xutils::ctypes::fs::Kstat;

use super::fd::{add_file_like, get_file_like};

pub trait FileLike: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize>;
    fn write(&self, buf: &[u8]) -> LinuxResult<usize>;
    fn stat(&self) -> LinuxResult<Kstat>;
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    fn poll(&self) -> LinuxResult<PollState>;
    fn set_nonblocking(&self, nonblocking: bool);
    fn is_nonblocking(&self) -> bool {
        false
    }

    fn from_fd(fd: c_int, required: FileFlags, forbidden: FileFlags) -> LinuxResult<Arc<Self>>
    where
        Self: Sized + 'static,
    {
        get_file_like(fd)?
            .validate(required, forbidden)?
            .clone()
            .into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::EINVAL)
    }

    fn add_to_fd_table(self, flags: FileFlags, cloexec: bool) -> LinuxResult<c_int>
    where
        Self: Sized + 'static,
    {
        add_file_like(Arc::new(self), flags, cloexec)
    }

    fn get_location(&self) -> Option<Location<RawMutex>> {
        None
    }

    fn len(&self) -> LinuxResult<u64> {
        Ok(0)
    }
}

#[derive(Clone)]
pub struct XFile {
    pub file: Arc<dyn FileLike>,
    pub flags: FileFlags,
}

impl XFile {
    pub fn new(file: Arc<dyn FileLike>, flags: FileFlags) -> Self {
        Self { file, flags }
    }

    pub fn validate(
        &self,
        required: FileFlags,
        forbidden: FileFlags,
    ) -> LinuxResult<&Arc<dyn FileLike>> {
        if self.flags.contains(required) && !self.flags.intersects(forbidden) {
            Ok(&self.file)
        } else {
            Err(LinuxError::EBADF)
        }
    }

    pub fn is<T: FileLike + 'static>(&self) -> bool {
        self.file.clone().into_any().is::<T>()
    }

    pub fn into_type<T: FileLike + 'static>(self) -> LinuxResult<Arc<T>> {
        self.file
            .clone()
            .into_any()
            .downcast::<T>()
            .map_err(|_| LinuxError::EINVAL)
    }
}

#[inherit_methods(from = "self.file")]
impl XFile {
    pub fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        self.validate(FileFlags::READ, FileFlags::PATH)?.read(buf)
    }
    pub fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        self.validate(FileFlags::WRITE, FileFlags::PATH)?.write(buf)
    }
    pub fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self.file.clone().into_any()
    }
    pub fn stat(&self) -> LinuxResult<Kstat>;
    pub fn poll(&self) -> LinuxResult<PollState>;
    pub fn set_nonblocking(&self, nonblocking: bool);
    pub fn is_nonblocking(&self) -> bool;
    pub fn get_location(&self) -> Option<Location<RawMutex>>;
    pub fn len(&self) -> LinuxResult<u64>;
}
