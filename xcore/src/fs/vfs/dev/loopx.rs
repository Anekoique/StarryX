use alloc::sync::Arc;
use core::{
    any::Any,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FsFile;
use axfs_ng_vfs::{DeviceId, VfsResult};
use axsync::{Mutex, RawMutex};
use linux_raw_sys::loop_device::loop_info;

use super::VirtDeviceOps;

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
    pub fn new(number: u32, dev_id: DeviceId) -> Self {
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
