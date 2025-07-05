use alloc::sync::Arc;
use axhal::paging::{MappingFlags, PageSize, PageTable};
use memory_addr::VirtAddr;

use super::{Backend, PageIterWrapper, SharedPages, alloc::alloc_frame};

impl Backend {
    /// Creates a new allocation mapping backend.
    pub fn new_shared(
        start: VirtAddr,
        size: usize,
        source: Option<Arc<SharedPages>>,
        align: PageSize,
    ) -> Option<Self> {
        let pages = if let Some(source) = source {
            assert_eq!(source.align, align);
            assert_eq!(source.len(), size / align as usize);
            source
        } else {
            Arc::new(SharedPages {
                phys_pages: PageIterWrapper::new(start, start + size, align)?
                    .map(|_| alloc_frame(true, align).unwrap())
                    .collect(),
                align,
            })
        };
        Some(Self::Shared { pages })
    }

    pub(crate) fn map_shared(
        start: VirtAddr,
        pages: &SharedPages,
        flags: MappingFlags,
        pt: &mut PageTable,
    ) -> bool {
        debug!(
            "map_shared: [{:#x}, {:#x}) {:?}",
            start,
            start + pages.len() * pages.align as usize,
            flags,
        );
        // allocate all possible physical frames for populated mapping.
        for (i, frame) in pages.iter().enumerate() {
            let addr = start + i * pages.align as usize;
            if let Ok(tlb) = pt.map(addr, *frame, pages.align, flags) {
                tlb.flush();
            } else {
                return false;
            }
        }
        true
    }

    pub(crate) fn unmap_shared(start: VirtAddr, pages: &SharedPages, pt: &mut PageTable) -> bool {
        debug!(
            "unmap_shared: [{:#x}, {:#x})",
            start,
            start + pages.len() * pages.align as usize
        );
        for i in 0..pages.len() {
            let addr = start + i * pages.align as usize;
            if let Ok((_, page_size, tlb)) = pt.unmap(addr) {
                // Deallocate the physical frame if there is a mapping in the
                // page table.
                if page_size.is_huge() {
                    return false;
                }
                tlb.flush();
            } else {
                // Deallocation is needn't if the page is not mapped.
            }
        }
        true
    }
}
