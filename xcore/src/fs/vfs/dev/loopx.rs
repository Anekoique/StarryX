use alloc::sync::Arc;
use core::{
    any::Any,
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FsFile;
use axfs_ng_vfs::{DeviceId, VfsResult};
use axsync::{Mutex, RawMutex};

use xuspace::{UserPtr, UserSpaceAccess};
use xutils::ctypes::{
    BLKGETSIZE, BLKGETSIZE64, BLKRAGET, BLKRASET, BLKROGET, BLKROSET, LOOP_CLR_FD, LOOP_GET_STATUS,
    LOOP_SET_FD, LOOP_SET_STATUS, loop_info,
};

use super::VirtDeviceOps;
use crate::{fs::fd::get_file_like, task::with_uspace};

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
    fn ioctl(&self, op: usize, argp: UserPtr<c_void>) -> VfsResult<isize> {
        with_uspace(|uspace| {
            match op as u32 {
                LOOP_SET_FD => {
                    let fd = argp.address().as_usize() as i32;
                    if fd < 0 {
                        return Err(LinuxError::EBADF);
                    }
                    let f = get_file_like(fd)?;
                    let Ok(file) = f.into_any().downcast::<crate::fs::fd::File>() else {
                        return Err(LinuxError::EINVAL);
                    };
                    let mut guard = self.file.lock();
                    if guard.is_some() {
                        return Err(LinuxError::EBUSY);
                    }
                    *guard = Some(file.clone_inner());
                }
                LOOP_CLR_FD => {
                    let mut guard = self.file.lock();
                    if guard.is_none() {
                        return Err(LinuxError::ENXIO);
                    }
                    *guard = None;
                }
                LOOP_GET_STATUS => {
                    self.get_info(uspace.raw_ptr(argp.cast())?)?;
                }
                LOOP_SET_STATUS => {
                    self.set_info(uspace.raw_ptr(argp.cast())?)?;
                }
                BLKGETSIZE | BLKGETSIZE64 => {
                    let file = self.clone_file()?;
                    let sectors = file.lock().inner().len()? / 512;
                    if op as u32 == BLKGETSIZE {
                        uspace.write(argp.cast::<u32>(), sectors as u32)?;
                    } else {
                        uspace.write(argp.cast::<u64>(), sectors * 512)?;
                    }
                }
                BLKROGET => {
                    uspace.write(argp.cast::<u32>(), self.ro.load(Ordering::Relaxed) as u32)?;
                }
                BLKROSET => {
                    let ro = uspace.read(argp.cast::<u32>())?;
                    if ro != 0 && ro != 1 {
                        return Err(LinuxError::EINVAL);
                    }
                    self.ro.store(ro != 0, Ordering::Relaxed);
                }
                BLKRAGET => {
                    uspace.write(argp.cast::<u32>(), self.ra.load(Ordering::Relaxed))?;
                }
                BLKRASET => {
                    self.ra
                        .store(argp.address().as_usize() as _, Ordering::Relaxed);
                }
                _ => {
                    warn!("unknown ioctl for loop device: {op}");
                    return Err(LinuxError::ENOTTY);
                }
            }
            Ok(0)
        })
    }
}
