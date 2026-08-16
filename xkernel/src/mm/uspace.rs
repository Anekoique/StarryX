use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use memory_addr::{MemoryAddr, VirtAddr, VirtAddrRange};
use page_table_multiarch::MappingFlags;
use xerrno::{LinuxError, LinuxResult};
use xsync::{Mutex, RawMutex};

use xuspace::UserSpaceAccess;
use xvma::{VmObject, VmSpace};

/// Per-process userspace state.
///
/// Mapping layout and file-backed metadata are both owned by the single
/// `xvma::VmSpace`; this wrapper only keeps process-local heap bounds and
/// exposes the user-copy validation interface.
pub struct XUserSpace {
    pub aspace: Arc<Mutex<VmSpace>>,
    heap_bottom: AtomicUsize,
    heap_top: AtomicUsize,
}

impl XUserSpace {
    pub fn new(aspace: Arc<Mutex<VmSpace>>) -> Self {
        Self {
            aspace,
            heap_bottom: AtomicUsize::new(crate::config::USER_HEAP_BASE),
            heap_top: AtomicUsize::new(crate::config::USER_HEAP_BASE),
        }
    }

    pub fn heap_bottom(&self) -> usize {
        self.heap_bottom.load(Ordering::Acquire)
    }

    pub fn set_heap_bottom(&self, bottom: usize) {
        self.heap_bottom.store(bottom, Ordering::Release);
    }

    pub fn heap_top(&self) -> usize {
        self.heap_top.load(Ordering::Acquire)
    }

    pub fn set_heap_top(&self, top: usize) {
        self.heap_top.store(top, Ordering::Release);
    }
}

impl UserSpaceAccess for &XUserSpace {
    fn check_region_access(
        &self,
        range: VirtAddrRange,
        access_flags: MappingFlags,
    ) -> LinuxResult<()> {
        let aspace = self.aspace.lock();
        if !aspace.check_region_access(range, access_flags) {
            warn!(
                "check_region_access: range={:?}, access_flags={:?}",
                range, access_flags
            );
            return Err(LinuxError::EFAULT);
        }
        Ok(())
    }

    fn populate_region(&self, range: VirtAddrRange, access_flags: MappingFlags) -> LinuxResult<()> {
        let page_start = range.start.align_down_4k();
        let page_end = range.end.align_up_4k();
        self.aspace
            .lock()
            .populate_area(page_start, page_end - page_start, access_flags)?;
        Ok(())
    }

    fn copy_from_user(&self, address: VirtAddr, output: &mut [u8]) -> LinuxResult<()> {
        if output.is_empty() {
            return Ok(());
        }
        let range =
            VirtAddrRange::try_from_start_size(address, output.len()).ok_or(LinuxError::EFAULT)?;
        let mut aspace = self.aspace.lock();
        aspace.populate_area(
            range.start.align_down_4k(),
            range.end.align_up_4k() - range.start.align_down_4k(),
            MappingFlags::READ,
        )?;
        aspace.read_bytes(address, output)?;
        Ok(())
    }

    fn copy_to_user(&self, address: VirtAddr, input: &[u8]) -> LinuxResult<()> {
        if input.is_empty() {
            return Ok(());
        }
        let range =
            VirtAddrRange::try_from_start_size(address, input.len()).ok_or(LinuxError::EFAULT)?;
        let mut aspace = self.aspace.lock();
        aspace.populate_area(
            range.start.align_down_4k(),
            range.end.align_up_4k() - range.start.align_down_4k(),
            MappingFlags::WRITE,
        )?;
        aspace.write_bytes(address, input)?;
        Ok(())
    }
}

/// Temporary kernel adapter for file-backed mappings.
///
/// A future `xcache::FileMapping` will implement `VmObject` directly.
pub struct FileWrapper(pub Arc<Mutex<xfs::FsFile<RawMutex>>>);

impl VmObject for FileWrapper {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> LinuxResult<usize> {
        self.0.lock().read_at(buf, offset)
    }

    fn byte_len(&self) -> LinuxResult<u64> {
        self.0.lock().len()
    }
}
