use axhal::paging::{MappingFlags, PageTable};
use memory_addr::VirtAddr;

use super::Backend;
use crate::frame::{alloc_frame, dealloc_frame};
use crate::utils::{FrameGuard, PageIterWrapper, PageSize};

impl Backend {
    /// Creates a new allocation mapping backend.
    pub const fn new_alloc(populate: bool, align: PageSize) -> Self {
        Self::Alloc { populate, align }
    }

    pub(crate) fn map_alloc(
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
        align: PageSize,
    ) -> bool {
        debug!(
            "map_alloc: [{:#x}, {:#x}) {:?} (populate={})",
            start,
            start + size,
            flags,
            populate
        );
        if !populate {
            return true;
        }
        let mut guard = FrameGuard::new(align);
        let page_iter = match PageIterWrapper::new(start, start + size, align) {
            Some(iter) => iter,
            None => return false,
        };

        for addr in page_iter {
            let frame = match alloc_frame(true, align) {
                Some(f) => f,
                None => return false,
            };
            guard.add(frame);

            if pt.map(addr, frame, align, flags).is_err() {
                return false;
            }
        }
        guard.release();
        true
    }

    pub(crate) fn unmap_alloc(
        start: VirtAddr,
        size: usize,
        pt: &mut PageTable,
        _populate: bool,
        align: PageSize,
    ) -> bool {
        debug!("unmap_alloc: [{:#x}, {:#x})", start, start + size);
        for addr in PageIterWrapper::new(start, start + size, align).unwrap() {
            if let Ok((frame, _, tlb)) = pt.unmap(addr) {
                tlb.flush();
                dealloc_frame(frame, align);
            } else {
                // Deallocation is needn't if the page is not mapped.
            }
        }
        true
    }

    pub(crate) fn handle_page_fault_alloc(
        vaddr: VirtAddr,
        orig_flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
        align: PageSize,
    ) -> bool {
        if populate {
            false // Populated mappings should not trigger page faults.
        } else if let Some(frame) = alloc_frame(true, align) {
            // Allocate a physical frame lazily and map it to the fault address.
            // `vaddr` does not need to be aligned. It will be automatically
            // aligned during `pt.map` regardless of the page size.
            pt.map(vaddr, frame, PageSize::Size4K, orig_flags)
                .map(|tlb| tlb.flush())
                .is_ok()
        } else {
            false
        }
    }
}
