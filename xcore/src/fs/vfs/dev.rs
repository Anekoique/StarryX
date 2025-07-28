use alloc::{format, sync::Arc};
use core::{
    any::Any,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{FsContext, FsFile};
use axfs_ng_vfs::{DeviceId, Filesystem, NodeType, VfsResult};
use axsync::{Mutex, RawMutex};
use linux_raw_sys::loop_device::loop_info;
use rand::{RngCore, SeedableRng, rngs::SmallRng};

use crate::fs::{
    virt_file::{DirMaker, VirtDir},
    virt_fs::{VirtDevice, VirtDeviceOps, VirtFs},
};

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
}

/// /dev/loopX devices
pub struct LoopDevice {
    number: u32,
    dev_id: DeviceId,
    /// Underlying file for the loop device, if any.
    pub file: Mutex<Option<Arc<Mutex<FsFile<RawMutex>>>>>,
    /// Read-only flag for the loop device.
    pub ro: AtomicBool,
    /// Read-ahead size for the loop device, in bytes.
    pub ra: AtomicU32,
}
impl LoopDevice {
    fn new(number: u32, dev_id: DeviceId) -> Self {
        Self {
            number,
            dev_id,
            file: Mutex::new(None),
            ro: AtomicBool::new(false),
            ra: AtomicU32::new(512),
        }
    }

    /// Get information about the loop device.
    pub fn get_info(&self, dest: &mut loop_info) -> LinuxResult<()> {
        if self.file.lock().is_none() {
            return Err(LinuxError::ENXIO);
        }
        dest.lo_number = self.number as _;
        dest.lo_rdevice = self.dev_id.0 as _;
        Ok(())
    }

    /// Set information for the loop device.
    pub fn set_info(&self, _src: &loop_info) -> LinuxResult<()> {
        Ok(())
    }

    /// Clone the underlying file of the loop device.
    pub fn clone_file(&self) -> VfsResult<Arc<Mutex<FsFile<RawMutex>>>> {
        let file = self.file.lock().clone();
        file.ok_or(LinuxError::ENXIO)
    }
}

impl VirtDeviceOps for LoopDevice {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let file = self.file.lock().clone();
        file.ok_or(LinuxError::EPERM)?.lock().read_at(buf, offset)
    }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        if self.ro.load(Ordering::Relaxed) {
            return Err(LinuxError::EROFS);
        }
        let file = self.file.lock().clone();
        file.ok_or(LinuxError::EPERM)?.lock().write_at(buf, offset)
    }
    fn as_any(&self) -> &dyn Any {
        self
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
    .add("shm", VirtDir::<()>::builder(fs.clone(), None).build());

    for i in 0..16 {
        let dev_id = DeviceId::new(7, 0);
        root.add(
            format!("loop{i}"),
            VirtDevice::new(
                fs.clone(),
                NodeType::BlockDevice,
                dev_id,
                LoopDevice::new(i, dev_id),
            ),
        );
    }

    root.build()
}
