use alloc::sync::Arc;
use core::{any::Any, ffi::c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axfs_ng_vfs::{DeviceId, Location, Metadata};
use axhal::time::TimeValue;
use axio::PollState;
use axsync::RawMutex;
use inherit_methods_macro::inherit_methods;
use linux_raw_sys::general::{stat, statx, statx_timestamp};

use super::{add_file_like, get_file_like};

pub trait FileLike: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize>;
    fn write(&self, buf: &[u8]) -> LinuxResult<usize>;
    fn stat(&self) -> LinuxResult<Kstat>;
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    fn poll(&self) -> LinuxResult<PollState>;
    fn set_nonblocking(&self, nonblocking: bool) -> LinuxResult;
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

    pub fn check_type<T: FileLike + 'static>(&self) -> bool {
        self.file.clone().into_any().downcast::<T>().is_ok()
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
    pub fn set_nonblocking(&self, nonblocking: bool) -> LinuxResult;
    pub fn is_nonblocking(&self) -> bool;
    pub fn get_location(&self) -> Option<Location<RawMutex>>;
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
