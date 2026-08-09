//! [ArceOS](https://github.com/arceos-org/arceos) memory management module.

#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

mod aspace;
mod backend;
#[cfg(feature = "cow")]
mod frame;
mod utils;

pub use self::aspace::AddrSpace;
pub use self::backend::{Backend, shared::SharedPages};
pub use self::frame::{alloc_frame, dealloc_frame};
pub use self::utils::*;

use kspin::SpinNoIrq;
use lazyinit::LazyInit;
use memory_addr::{PhysAddr, va};
use memory_set::MappingError;
use xerrno::{XError, XResult};
use xhal::mem::phys_to_virt;

static KERNEL_ASPACE: LazyInit<SpinNoIrq<AddrSpace>> = LazyInit::new();

fn mapping_err_to_x_err(err: MappingError) -> XError {
    warn!("Mapping error: {:?}", err);
    match err {
        MappingError::InvalidParam => XError::InvalidInput,
        MappingError::AlreadyExists => XError::AlreadyExists,
        MappingError::BadState => XError::BadState,
    }
}

/// Creates a new address space for kernel itself.
pub fn new_kernel_aspace() -> XResult<AddrSpace> {
    let mut aspace = AddrSpace::new_empty(
        va!(xconfig::plat::KERNEL_ASPACE_BASE),
        xconfig::plat::KERNEL_ASPACE_SIZE,
    )?;
    for r in xhal::mem::memory_regions() {
        aspace.map_linear(
            phys_to_virt(r.paddr),
            r.paddr,
            r.size,
            r.flags.into(),
            PageSize::Size4K,
        )?;
    }
    Ok(aspace)
}

/// Returns the globally unique kernel address space.
pub fn kernel_aspace() -> &'static SpinNoIrq<AddrSpace> {
    &KERNEL_ASPACE
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
