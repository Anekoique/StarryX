//! Trusted physical-frame and page-table mechanisms for StarryX.
//!
//! Process-visible mapping policy belongs to `xvma`. This crate retains the
//! low-level operations that construct page tables, map static kernel/device
//! ranges, allocate frames, and perform architecture-facing PTE changes.

#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

mod aspace;
mod frame;
mod utils;

pub use self::aspace::{AddressSpace, ProtectionTransaction};
pub use self::frame::{Frame, StaticFrameRange, init_frame_database};
pub use self::utils::{MappingFlags, PAGE_SIZE_4K, PageIter, PageIter4K, PageSize};

use kspin::SpinNoIrq;
use lazyinit::LazyInit;
use memory_addr::{PhysAddr, va};
use xerrno::XResult;
use xhal::mem::phys_to_virt;

static KERNEL_ASPACE: LazyInit<SpinNoIrq<AddressSpace>> = LazyInit::new();

/// Creates a new address space for kernel itself.
pub fn new_kernel_aspace() -> XResult<AddressSpace> {
    let mut aspace = AddressSpace::new_empty(
        va!(xconfig::plat::KERNEL_ASPACE_BASE),
        xconfig::plat::KERNEL_ASPACE_SIZE,
    )?;
    for r in xhal::mem::memory_regions() {
        let flags = r.flags.into();
        // SAFETY: xhal describes platform and kernel-image ranges whose
        // physical storage remains present for the complete kernel lifetime.
        let frames = unsafe { StaticFrameRange::new(r.paddr, r.size, flags) }
            .expect("xhal returned an invalid static frame range");
        aspace.map_static_range(phys_to_virt(r.paddr), frames, flags, PageSize::Size4K)?;
    }
    Ok(aspace)
}

/// Returns the globally unique kernel address space.
pub fn kernel_aspace() -> &'static SpinNoIrq<AddressSpace> {
    &KERNEL_ASPACE
}

/// Imports the immortal kernel page-table hierarchy into a user address space.
#[cfg(feature = "copy-from")]
pub fn copy_kernel_mappings(destination: &mut AddressSpace) -> XResult {
    destination.copy_static_mappings_from(&KERNEL_ASPACE.lock())
}

/// Returns the root physical address of the kernel page table.
pub fn kernel_page_table_root() -> PhysAddr {
    KERNEL_ASPACE.lock().page_table_root()
}

/// Initializes virtual memory management.
///
/// It mainly sets up the kernel virtual memory address space and recreate a
/// fine-grained kernel page table.
pub fn init_memory_management() {
    info!("Initialize virtual memory management...");

    let kernel_aspace = new_kernel_aspace().expect("failed to initialize kernel address space");
    debug!("kernel address space init OK: {:#x?}", kernel_aspace);
    KERNEL_ASPACE.init_once(SpinNoIrq::new(kernel_aspace));
    xhal::paging::set_kernel_page_table_root(kernel_page_table_root());
}

/// Initializes kernel paging for secondary CPUs.
pub fn init_memory_management_secondary() {
    xhal::paging::set_kernel_page_table_root(kernel_page_table_root());
}
