//! Mock block devices that store data in RAM.

use crate::BlockDriverOps;
use axdriver_base::{BaseDriverOps, DevError, DevResult, DeviceType};

const BLOCK_SIZE: usize = 512;

/// A RAM disk that loads from memory.
#[derive(Default)]
pub struct RamDisk(&'static mut [u8]);

impl RamDisk {
    /// Creates a new RAM disk with the given base address and size.
    ///
    /// # Panics
    /// Panics if the base address or size is not aligned to `BLOCK_SIZE`.
    ///
    /// # Safety
    /// The caller must ensure that the memory is valid and accessible.
    pub const unsafe fn new(base: usize, size: usize) -> Self {
        assert!(
            base % BLOCK_SIZE == 0,
            "Base address must be a multiple of BLOCK_SIZE"
        );
        assert!(
            size % BLOCK_SIZE == 0,
            "Size must be a multiple of BLOCK_SIZE"
        );
        let data = core::slice::from_raw_parts_mut(base as *mut u8, size);
        RamDisk(data)
    }
}

impl BaseDriverOps for RamDisk {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn device_name(&self) -> &str {
        "ramdisk"
    }
}

impl BlockDriverOps for RamDisk {
    #[inline]
    fn num_blocks(&self) -> u64 {
        (self.0.len() / BLOCK_SIZE) as u64
    }

    #[inline]
    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }

    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> DevResult {
        if buf.len() % BLOCK_SIZE != 0 {
            return Err(DevError::InvalidParam);
        }
        let offset = block_id as usize * BLOCK_SIZE;
        if offset + buf.len() > self.0.len() {
            return Err(DevError::Io);
        }
        buf.copy_from_slice(&self.0[offset..offset + buf.len()]);
        Ok(())
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> DevResult {
        if buf.len() % BLOCK_SIZE != 0 {
            return Err(DevError::InvalidParam);
        }
        let offset = block_id as usize * BLOCK_SIZE;
        if offset + buf.len() > self.0.len() {
            return Err(DevError::Io);
        }
        self.0[offset..offset + buf.len()].copy_from_slice(buf);
        Ok(())
    }

    fn flush(&mut self) -> DevResult {
        Ok(())
    }
}