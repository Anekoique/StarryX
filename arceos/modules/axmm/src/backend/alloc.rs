use ::alloc::vec::Vec;
use axalloc::{PhysPage, PhysPageSet, global_allocator};
use axhal::mem::{phys_to_virt, virt_to_phys};
use axhal::paging::{MappingFlags, PageSize, PageTable};
use memory_addr::{PAGE_SIZE_4K, PageIter4K, PhysAddr, VirtAddr};

use super::Backend;

pub(crate) fn alloc_frame(zeroed: bool) -> Option<PhysAddr> {
    let vaddr = VirtAddr::from(global_allocator().alloc_pages(1, PAGE_SIZE_4K).ok()?);
    if zeroed {
        unsafe { core::ptr::write_bytes(vaddr.as_mut_ptr(), 0, PAGE_SIZE_4K) };
    }
    let paddr = virt_to_phys(vaddr);
    Some(paddr)
}

pub(crate) fn dealloc_frame(frame: PhysAddr) {
    let vaddr = phys_to_virt(frame);
    global_allocator().dealloc_pages(vaddr.as_usize(), 1);
}

struct FrameGuard(Vec<PhysAddr>);

impl FrameGuard {
    fn new() -> Self {
        Self(Vec::new())
    }
    fn add(&mut self, frame: PhysAddr) {
        self.0.push(frame);
    }
    fn release(self) {
        core::mem::forget(self);
    }
}
impl Drop for FrameGuard {
    fn drop(&mut self) {
        for frame in self.0.drain(..) {
            dealloc_frame(frame);
        }
    }
}

impl Backend {
    /// Creates a new allocation mapping backend.
    ///
    /// # Arguments
    ///
    /// * `populate` - Whether to populate physical frames when creating the mapping
    /// * `shared` - Whether to use shared pages for this allocation
    ///
    /// # Returns
    ///
    /// A new `Backend::Alloc` instance configured with the specified parameters
    pub fn new_alloc(populate: bool, shared: bool) -> Self {
        Self::Alloc {
            populate,
            pages: core::cell::RefCell::new(if shared {
                Some(PhysPageSet::new())
            } else {
                None
            }),
        }
    }

    /// Maps a virtual address range with allocation-based backing.
    ///
    /// For populated mappings, all physical frames are allocated immediately.
    /// For lazy mappings, allocation is deferred to page fault handling.
    /// Shared pages are tracked in the provided PhysPageSet collection if enabled.
    pub(crate) fn map_alloc(
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
        pages: &mut Option<PhysPageSet>,
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

        let mut guard = FrameGuard::new();
        let page_iter = match PageIter4K::new(start, start + size) {
            Some(iter) => iter,
            None => return false,
        };

        for addr in page_iter {
            let frame = match alloc_frame(true) {
                Some(f) => f,
                None => return false,
            };

            guard.add(frame);

            if pt.map(addr, frame, PageSize::Size4K, flags).is_err() {
                return false;
            }

            if let Some(shared_pages) = pages {
                let virt_addr = phys_to_virt(frame);
                shared_pages.push_page(PhysPage::new(virt_addr));
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
        pages: &mut Option<PhysPageSet>,
    ) -> bool {
        debug!("unmap_alloc: [{:#x}, {:#x})", start, start + size);
        for addr in PageIter4K::new(start, start + size).unwrap() {
            if let Ok((frame, page_size, tlb)) = pt.unmap(addr) {
                if page_size.is_huge() {
                    return false;
                }
                tlb.flush();
                if let Some(page_set) = pages {
                    if !page_set.remove_by_paddr(frame, phys_to_virt) {
                        dealloc_frame(frame);
                    }
                } else {
                    dealloc_frame(frame);
                }
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
    ) -> bool {
        if populate {
            false // Populated mappings should not trigger page faults.
        } else if let Some(frame) = alloc_frame(true) {
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
