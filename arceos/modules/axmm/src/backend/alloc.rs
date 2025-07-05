use ::alloc::vec::Vec;
use axalloc::global_allocator;
use axhal::mem::{phys_to_virt, virt_to_phys};
use axhal::paging::{MappingFlags, PageSize, PageTable};
use memory_addr::{PAGE_SIZE_4K, PhysAddr, VirtAddr};

use super::{Backend, PageIterWrapper};

pub(crate) fn alloc_frame(zeroed: bool, align: PageSize) -> Option<PhysAddr> {
    let page_size: usize = align.into();
    let num_pages = page_size / PAGE_SIZE_4K;
    let vaddr = VirtAddr::from(global_allocator().alloc_pages(num_pages, page_size).ok()?);
    if zeroed {
        unsafe { core::ptr::write_bytes(vaddr.as_mut_ptr(), 0, page_size) };
    }
    let paddr = virt_to_phys(vaddr);
    Some(paddr)
}

pub(crate) fn dealloc_frame(frame: PhysAddr, align: PageSize) {
    let page_size: usize = align.into();
    let num_pages = page_size / PAGE_SIZE_4K;
    let vaddr = phys_to_virt(frame);
    global_allocator().dealloc_pages(vaddr.as_usize(), num_pages);
}

struct FrameGuard {
    frames: Vec<PhysAddr>,
    align: PageSize,
}

impl FrameGuard {
    fn new(align: PageSize) -> Self {
        Self {
            frames: Vec::new(),
            align,
        }
    }

    fn add(&mut self, frame: PhysAddr) {
        self.frames.push(frame);
    }

    fn release(self) {
        core::mem::forget(self);
    }
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        for frame in self.frames.drain(..) {
            dealloc_frame(frame, self.align);
        }
    }
}

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
