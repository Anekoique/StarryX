//! FrameInfo
//!
//! A simple physical FrameInfo manager is provided to track and manage
//! the reference count for every 4KB memory page frame in the system.
//!
//! There is a [`FrameInfo`] struct for each physical page frame
//! that keeps track of its reference count.
//! NOTE: If the page is huge page, its [`FrameInfo`] is placed at the
//! starting physical address.
use core::{
    array,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::boxed::Box;
use axalloc::global_allocator;
use axhal::mem::{phys_to_virt, virt_to_phys};
use lazy_static::lazy_static;
use memory_addr::{PhysAddr, VirtAddr};

use crate::utils::{PAGE_SIZE_4K, PageSize};

const FRAME_SHIFT: usize = 12;
pub const MAX_FRAME_NUM: usize = axconfig::plat::PHYS_MEMORY_SIZE >> FRAME_SHIFT;

#[derive(Default)]
pub(crate) struct FrameInfo {
    ref_count: AtomicUsize,
}

lazy_static! {
    static ref FRAME_INFO_TABLE: FrameRefTable = FrameRefTable::default();
}

pub(crate) fn frame_table() -> &'static FrameRefTable {
    &FRAME_INFO_TABLE
}

pub(crate) struct FrameRefTable {
    data: Box<[FrameInfo; MAX_FRAME_NUM]>,
}

impl Default for FrameRefTable {
    fn default() -> Self {
        FrameRefTable {
            data: Box::new(array::from_fn(|_| FrameInfo::default())),
        }
    }
}

impl FrameRefTable {
    fn info(&self, paddr: PhysAddr) -> &FrameInfo {
        let index = (paddr.as_usize() - axconfig::plat::PHYS_MEMORY_BASE) >> FRAME_SHIFT;
        &self.data[index]
    }

    /// Increases the reference count of the frame associated with a physical address.
    ///
    /// # Parameters
    /// - `paddr`: It must be an aligned physical address; if it's a huge page,
    ///   it must be the starting physical address.
    pub fn inc_ref(&self, paddr: PhysAddr) {
        self.info(paddr).ref_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Decreases the reference count of the frame associated with a physical address.
    ///
    /// - `paddr`: It must be an aligned physical address; if it's a huge page,
    ///   it must be the starting physical address.
    ///
    /// # Returns
    /// The updated reference count after decrementing.
    pub fn dec_ref(&self, paddr: PhysAddr) -> usize {
        self.info(paddr).ref_count.fetch_sub(1, Ordering::SeqCst)
    }

    /// Returns the `FrameInfo` structure associated with a given physical address.
    ///
    /// # Parameters
    /// - `paddr`: It must be an aligned physical address; if it's a huge page,
    ///   it must be the starting physical address.
    ///
    /// # Returns
    /// A reference to the `FrameInfo` associated with the given physical address.
    pub fn ref_count(&self, paddr: PhysAddr) -> usize {
        self.info(paddr).ref_count.load(Ordering::SeqCst)
    }
}

pub fn alloc_frame(zeroed: bool, align: PageSize) -> Option<PhysAddr> {
    let page_size: usize = align.into();
    let num_pages = page_size / PAGE_SIZE_4K;
    let vaddr = VirtAddr::from(global_allocator().alloc_pages(num_pages, page_size).ok()?);
    if zeroed {
        unsafe { core::ptr::write_bytes(vaddr.as_mut_ptr(), 0, page_size) };
    }
    let paddr = virt_to_phys(vaddr);
    #[cfg(feature = "cow")]
    frame_table().inc_ref(paddr);
    Some(paddr)
}

pub fn dealloc_frame(frame: PhysAddr, align: PageSize) {
    #[cfg(feature = "cow")]
    if frame_table().dec_ref(frame) > 1 {
        return;
    }
    let page_size: usize = align.into();
    let num_pages = page_size / PAGE_SIZE_4K;
    let vaddr = phys_to_virt(frame);
    global_allocator().dealloc_pages(vaddr.as_usize(), num_pages);
}
