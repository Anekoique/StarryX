//! Kernel-profile adapter for the [`xvdso`] component.
//!
//! The component owns the external Linux image and shared vvar pages. This
//! module only installs those shared pages into a process address space.

use memory_addr::{PAGE_SIZE_4K, VirtAddr};
use xerrno::XResult;
use xhal::{
    mem::virt_to_phys,
    paging::{MappingFlags, PageSize},
};
use xmm::AddrSpace;

use crate::config;

/// Per-process vDSO mapping information.
#[derive(Clone, Copy, Debug)]
pub struct VdsoBinding {
    pub base: VirtAddr,
    pub rt_sigreturn: VirtAddr,
}

/// Maps the shared Linux vDSO image and its read-only data pages.
pub fn install(uspace: &mut AddrSpace) -> XResult<VdsoBinding> {
    xvdso::refresh_data();

    let code_base = VirtAddr::from_usize(config::USER_VDSO_BASE);
    let data_base = VirtAddr::from_usize(config::USER_VDSO_DATA);
    debug_assert_eq!(code_base - data_base, xvdso::VVAR_PAGES * PAGE_SIZE_4K);

    let data_kernel_addr = VirtAddr::from_usize(xvdso::data_page_kernel_address());
    uspace.map_linear(
        data_base,
        virt_to_phys(data_kernel_addr),
        PAGE_SIZE_4K,
        MappingFlags::READ | MappingFlags::USER,
        PageSize::Size4K,
    )?;

    #[cfg(target_arch = "loongarch64")]
    {
        let arch_data_base = data_base + 2 * PAGE_SIZE_4K;
        let arch_data_kernel_addr = VirtAddr::from_usize(xvdso::arch_data_page_kernel_address());
        uspace.map_linear(
            arch_data_base,
            virt_to_phys(arch_data_kernel_addr),
            PAGE_SIZE_4K,
            MappingFlags::READ | MappingFlags::USER,
            PageSize::Size4K,
        )?;
    }

    let image = xvdso::image();
    debug_assert!(image.len().is_multiple_of(PAGE_SIZE_4K));
    let image_kernel_addr = VirtAddr::from_usize(image.as_ptr() as usize);
    uspace.map_linear(
        code_base,
        virt_to_phys(image_kernel_addr),
        image.len(),
        MappingFlags::READ | MappingFlags::EXECUTE | MappingFlags::USER,
        PageSize::Size4K,
    )?;

    Ok(VdsoBinding {
        base: code_base,
        rt_sigreturn: code_base + xvdso::rt_sigreturn_offset(),
    })
}
