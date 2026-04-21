pub mod loopx;
pub mod tty;

pub use loopx::LoopDevice;
pub use tty::Tty;

use alloc::{format, string::ToString, sync::Arc};
use core::{any::Any, ffi::c_void};

use axerrno::LinuxResult;
use axfs_ng::FsContext;
use axfs_ng_vfs::{DeviceId, Filesystem, NodeType, VfsResult};
use axsync::{Mutex, RawMutex};
use chrono::{Datelike, Timelike};
use rand::{Rng, SeedableRng, rngs::SmallRng};

use xuspace::{UserPtr, UserSpaceAccess};
use xutils::{ctypes::sys::rtc_time, time::wall_time_nanos};

use super::{
    virt_file::{DirMaker, VirtDir, VirtFile},
    virt_fs::{VirtDevice, VirtDeviceOps, VirtFs},
};
use crate::task::with_uspace;

pub const RTC0_DEVICE_ID: DeviceId = DeviceId::new(250, 0);

const RANDOM_SEED: &[u8; 32] = b"0123456789abcdef0123456789abcdef";

/// Initialize the device filesystem with common devices and mount /dev/shm
pub fn init_devfs() -> LinuxResult<Filesystem<RawMutex>> {
    let fs = VirtFs::new_with("devtmpfs".into(), 0x01021994, create_dev_root);
    let mp = axfs_ng_vfs::Mountpoint::new_root(&fs);

    FsContext::new(mp.root_location())
        .resolve("/shm")?
        .mount(&super::tmp::init_tmpfs())?;

    Ok(fs)
}

struct Null;
impl VirtDeviceOps for Null {
    fn read_at(&self, _buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        Ok(0)
    }
    fn write_at(&self, buf: &[u8], _offset: u64) -> VfsResult<usize> {
        Ok(buf.len())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn ioctl(&self, _op: usize, _argp: UserPtr<c_void>) -> VfsResult<isize> {
        Ok(0)
    }
}

struct Zero;
impl VirtDeviceOps for Zero {
    fn read_at(&self, buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        buf.fill(0);
        Ok(buf.len())
    }
    fn write_at(&self, _buf: &[u8], _offset: u64) -> VfsResult<usize> {
        Ok(0)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn ioctl(&self, _op: usize, _argp: UserPtr<c_void>) -> VfsResult<isize> {
        Ok(0)
    }
}

struct Full;
impl VirtDeviceOps for Full {
    fn read_at(&self, buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        buf.fill(0);
        Ok(buf.len())
    }
    fn write_at(&self, buf: &[u8], _offset: u64) -> VfsResult<usize> {
        Ok(buf.len())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn ioctl(&self, _op: usize, _argp: UserPtr<c_void>) -> VfsResult<isize> {
        Ok(0)
    }
}

struct Random {
    rng: Mutex<SmallRng>,
}
impl Random {
    pub fn new() -> Self {
        Self {
            rng: Mutex::new(SmallRng::from_seed(*RANDOM_SEED)),
        }
    }
}
impl VirtDeviceOps for Random {
    fn read_at(&self, buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        self.rng.lock().fill_bytes(buf);
        Ok(buf.len())
    }
    fn write_at(&self, buf: &[u8], _offset: u64) -> VfsResult<usize> {
        Ok(buf.len())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn ioctl(&self, _op: usize, _argp: UserPtr<c_void>) -> VfsResult<isize> {
        Ok(0)
    }
}

pub struct Rtc;
impl VirtDeviceOps for Rtc {
    fn read_at(&self, _buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        Ok(0)
    }
    fn write_at(&self, _buf: &[u8], _offset: u64) -> VfsResult<usize> {
        Ok(0)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn ioctl(&self, _op: usize, argp: UserPtr<c_void>) -> VfsResult<isize> {
        with_uspace(|uspace| {
            let wall = chrono::DateTime::from_timestamp_nanos(wall_time_nanos() as _);
            uspace.write(
                argp.cast::<rtc_time>(),
                rtc_time {
                    tm_sec: wall.second() as _,
                    tm_min: wall.minute() as _,
                    tm_hour: wall.hour() as _,
                    tm_mday: wall.day() as _,
                    tm_mon: wall.month0() as _,
                    tm_year: (wall.year() - 1900) as _,
                    tm_wday: 0,
                    tm_yday: 0,
                    tm_isdst: 0,
                },
            )?;
            Ok(0)
        })
    }
}

/// Create the root directory structure for /dev filesystem
fn create_dev_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::<()>::builder(fs.clone(), None);

    root.add(
        "null",
        VirtDevice::new(
            fs.clone(),
            NodeType::CharacterDevice,
            DeviceId::new(1, 3),
            Null,
        ),
    )
    .add(
        "zero",
        VirtDevice::new(
            fs.clone(),
            NodeType::CharacterDevice,
            DeviceId::new(1, 5),
            Zero,
        ),
    )
    .add(
        "full",
        VirtDevice::new(
            fs.clone(),
            NodeType::CharacterDevice,
            DeviceId::new(1, 7),
            Full,
        ),
    )
    .add(
        "random",
        VirtDevice::new(
            fs.clone(),
            NodeType::CharacterDevice,
            DeviceId::new(1, 8),
            Random::new(),
        ),
    )
    .add(
        "urandom",
        VirtDevice::new(
            fs.clone(),
            NodeType::CharacterDevice,
            DeviceId::new(1, 9),
            Random::new(),
        ),
    )
    .add(
        "rtc0",
        VirtDevice::new(fs.clone(), NodeType::CharacterDevice, RTC0_DEVICE_ID, Rtc),
    )
    .add(
        "stdin",
        VirtFile::new_symlink(fs.clone(), || "/proc/self/fd/0".to_string()),
    )
    .add(
        "stdout",
        VirtFile::new_symlink(fs.clone(), || "/proc/self/fd/1".to_string()),
    )
    .add(
        "stderr",
        VirtFile::new_symlink(fs.clone(), || "/proc/self/fd/2".to_string()),
    )
    .add(
        "tty",
        VirtDevice::new(
            fs.clone(),
            NodeType::CharacterDevice,
            DeviceId::new(5, 0),
            tty::Tty::new(),
        ),
    )
    .add(
        "fd",
        VirtFile::new_symlink(fs.clone(), || "/proc/self/fd".to_string()),
    )
    .add("shm", VirtDir::<()>::builder(fs.clone(), None).build());

    for i in 0..16 {
        let dev_id = DeviceId::new(7, 0);
        root.add(
            format!("loop{i}"),
            VirtDevice::new(
                fs.clone(),
                NodeType::BlockDevice,
                dev_id,
                loopx::LoopDevice::new(i, dev_id),
            ),
        );
    }

    root.build()
}
