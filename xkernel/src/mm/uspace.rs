use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use memory_addr::{MemoryAddr, VirtAddr, VirtAddrRange};
use page_table_multiarch::MappingFlags;
use xerrno::{LinuxError, LinuxResult};
use xsync::Mutex;

use xuspace::UserSpaceAccess;
use xvma::VmSpace;

use super::MappedFiles;

/// Per-process userspace state.
///
/// `xvma::VmSpace` owns mapping layout; this wrapper keeps process-local heap
/// bounds, the cache-invalidation subscriptions of the files it maps, and
/// exposes the user-copy validation interface.
pub struct XUserSpace {
    pub aspace: Arc<Mutex<VmSpace>>,
    pub(crate) mapped_files: Arc<MappedFiles>,
    heap_bottom: AtomicUsize,
    heap_top: AtomicUsize,
}

impl XUserSpace {
    pub fn new(aspace: Arc<Mutex<VmSpace>>) -> Self {
        Self {
            mapped_files: MappedFiles::new(&aspace),
            aspace,
            heap_bottom: AtomicUsize::new(crate::config::USER_HEAP_BASE),
            heap_top: AtomicUsize::new(crate::config::USER_HEAP_BASE),
        }
    }

    /// Builds the state a forked process inherits.
    ///
    /// A child sharing the address space shares its subscriptions too; a child
    /// with its own copy re-subscribes so a truncation reaches both spaces.
    pub fn fork(&self, aspace: Arc<Mutex<VmSpace>>) -> LinuxResult<Self> {
        let mapped_files = if Arc::ptr_eq(&aspace, &self.aspace) {
            self.mapped_files.clone()
        } else {
            self.mapped_files.fork(&aspace)?
        };
        Ok(Self {
            mapped_files,
            aspace,
            heap_bottom: AtomicUsize::new(self.heap_bottom()),
            heap_top: AtomicUsize::new(self.heap_top()),
        })
    }

    /// Faults in the pages covering `range`, then runs `op` under the same
    /// address-space lock so no other mapping change can interleave.
    fn with_populated(
        &self,
        range: VirtAddrRange,
        access: MappingFlags,
        op: impl FnOnce(&mut VmSpace) -> xerrno::XResult,
    ) -> LinuxResult<()> {
        let start = range.start.align_down_4k();
        let mut aspace = self.aspace.lock();
        aspace.populate_area(start, range.end.align_up_4k() - start, access)?;
        op(&mut aspace)?;
        Ok(())
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
        self.with_populated(range, access_flags, |_| Ok(()))
    }

    fn copy_from_user(&self, address: VirtAddr, output: &mut [u8]) -> LinuxResult<()> {
        if output.is_empty() {
            return Ok(());
        }
        let range =
            VirtAddrRange::try_from_start_size(address, output.len()).ok_or(LinuxError::EFAULT)?;
        self.with_populated(range, MappingFlags::READ, |aspace| {
            aspace.read_bytes(address, output)
        })
    }

    fn copy_to_user(&self, address: VirtAddr, input: &[u8]) -> LinuxResult<()> {
        if input.is_empty() {
            return Ok(());
        }
        let range =
            VirtAddrRange::try_from_start_size(address, input.len()).ok_or(LinuxError::EFAULT)?;
        self.with_populated(range, MappingFlags::WRITE, |aspace| {
            aspace.write_bytes(address, input)
        })
    }
}
