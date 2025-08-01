//! SD card driver for VisionFive2

use crate::BlockDriverOps;
use axdriver_base::{BaseDriverOps, DevResult, DeviceType};
pub use visionfive2_sd::Vf2SdDriver;
use visionfive2_sd::{SDIo, SleepOps};

pub struct VF2SD {
    driver: Vf2SdDriver<SdIoImpl, SleepOpsImpl>,
}

impl VF2SD {
    pub fn new() -> Self {
        let mut r = Self {
            driver: Vf2SdDriver::new(SdIoImpl),
        };
        r.init();
        r
    }
    pub fn init(&mut self) {
        self.driver.init();
    }
}

impl BaseDriverOps for VF2SD {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn device_name(&self) -> &str {
        "VisionFive2_SD"
    }
}

impl BlockDriverOps for VF2SD {
    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> DevResult {
        self.driver.read_block(block_id as usize, buf);
        Ok(())
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> DevResult {
        self.driver.write_block(block_id as usize, buf);
        Ok(())
    }
    fn flush(&mut self) -> DevResult {
        Ok(())
    }

    #[inline]
    fn num_blocks(&self) -> u64 {
        8000000
    }

    #[inline]
    fn block_size(&self) -> usize {
        512
    }
}

pub struct SdIoImpl;
pub const SDIO_BASE: usize = 0xffffffc016020000;

impl SDIo for SdIoImpl {
    fn read_reg_at(&self, offset: usize) -> u32 {
        let addr = (SDIO_BASE + offset) as *mut u32;
        unsafe { addr.read_volatile() }
    }
    fn write_reg_at(&mut self, offset: usize, val: u32) {
        let addr = (SDIO_BASE + offset) as *mut u32;
        unsafe { addr.write_volatile(val) }
    }
    fn read_data_at(&self, offset: usize) -> u64 {
        let addr = (SDIO_BASE + offset) as *mut u64;
        unsafe { addr.read_volatile() }
    }
    fn write_data_at(&mut self, offset: usize, val: u64) {
        let addr = (SDIO_BASE + offset) as *mut u64;
        unsafe { addr.write_volatile(val) }
    }
}

pub struct SleepOpsImpl;

const TIMER_FREQ: usize = 4000000;

impl SleepOps for SleepOpsImpl {
    fn sleep_ms(ms: usize) {
        let start = read_timer();
        while read_timer() - start < ms * TIMER_FREQ / 1000 {
            core::hint::spin_loop();
        }
    }
    fn sleep_ms_until(ms: usize, f: impl FnMut() -> bool) {
        sleep_ms_until(ms, f)
    }
}

fn read_timer() -> usize {
    riscv::register::time::read()
}

fn sleep_ms_until(ms: usize, mut f: impl FnMut() -> bool) {
    let start = read_timer();
    while read_timer() - start < ms * TIMER_FREQ / 1000 {
        if f() {
            return;
        }
        core::hint::spin_loop();
    }
}
