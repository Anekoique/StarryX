#![allow(dead_code)]
use alloc::{format, sync::Arc};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{FsContext, FsFile};
use axfs_ng_vfs::{DeviceId, Filesystem, NodeType, VfsResult};
use axsync::{Mutex, RawMutex};
use linux_raw_sys::loop_device::loop_info;
use rand::{RngCore, SeedableRng, rngs::SmallRng};

use crate::fs::{
    virt_file::{VirtDevice, VirtDeviceOps},
    virt_fs::{DirMaker, VirtDir, VirtDirBuilder, VirtFs},
};

/// The device ID for /dev/rtc0
pub const RTC0_DEVICE_ID: DeviceId = DeviceId::new(250, 0);

const RANDOM_SEED: &[u8; 32] = b"0123456789abcdef0123456789abcdef";

/// Initialize the device filesystem with common devices and mount /dev/shm
pub fn init_devfs() -> LinuxResult<Filesystem<RawMutex>> {
    let fs = VirtFs::new_with("devtmpfs".into(), 0x01021994, create_dev_root);
    let mp = axfs_ng_vfs::Mountpoint::new_root(&fs);

    // Mount /dev/shm as memory filesystem
    FsContext::new(mp.root_location())
        .resolve("/shm")?
        .mount(&super::tmp::init_tmpfs())?;

    Ok(fs)
}

/// Device operations enumeration for all supported device types
#[derive(Clone)]
enum DeviceOps {
    Null,
    Zero,
    Full,
    Random,
    Rtc,
}

impl VirtDeviceOps for DeviceOps {
    fn read_at(&self, buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        match self {
            Self::Null => Ok(0),
            Self::Zero => {
                buf.fill(0);
                Ok(buf.len())
            }
            Self::Full => {
                buf.fill(0);
                Ok(buf.len())
            }
            Self::Random => {
                let mut rng = SmallRng::from_seed(*RANDOM_SEED);
                rng.fill_bytes(buf);
                Ok(buf.len())
            }
            Self::Rtc => Ok(0),
        }
    }

    fn write_at(&self, buf: &[u8], _offset: u64) -> VfsResult<usize> {
        match self {
            Self::Null | Self::Random => Ok(buf.len()),
            Self::Zero | Self::Rtc => Ok(0),
            Self::Full => Err(LinuxError::ENOSPC),
        }
    }
}

/// Macro to simplify device specification with name, major, minor, and operations
macro_rules! device_spec {
    ($name:literal, $major:expr, $minor:expr, $ops:expr) => {
        (
            $name,
            NodeType::CharacterDevice,
            DeviceId::new($major, $minor),
            $ops,
        )
    };
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
}

/// Helper function to add a device to the virtual directory builder
fn add_device(
    root: &mut VirtDirBuilder,
    fs: &Arc<VirtFs>,
    name: &str,
    node_type: NodeType,
    device_id: DeviceId,
    ops: DeviceOps,
) {
    root.add(name, VirtDevice::new(fs.clone(), node_type, device_id, ops));
}

/// Create the root directory structure for /dev filesystem
fn create_dev_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::builder(fs.clone());

    let devices = [
        device_spec!("null", 1, 3, DeviceOps::Null),
        device_spec!("zero", 1, 5, DeviceOps::Zero),
        device_spec!("full", 1, 7, DeviceOps::Full),
        device_spec!("random", 1, 8, DeviceOps::Random),
        device_spec!("urandom", 1, 9, DeviceOps::Random),
        device_spec!("rtc0", 250, 0, DeviceOps::Rtc),
        device_spec!("rtc", 251, 0, DeviceOps::Rtc),
    ];

    for (name, node_type, device_id, ops) in devices {
        add_device(&mut root, &fs, name, node_type, device_id, ops);
    }
    root.add("shm", VirtDir::builder(fs.clone()).build());

    for i in 0..16 {
        let dev_id = DeviceId::new(7, i);
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
