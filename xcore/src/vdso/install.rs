//! Map the vDSO image + data page into a fresh user address space.

use axerrno::AxResult;
use axhal::{
    mem::virt_to_phys,
    paging::{MappingFlags, PageSize},
};
use axmm::AddrSpace;
use memory_addr::{PAGE_SIZE_4K, VirtAddr};

use super::{data, image};
use crate::config;

/// Per-process vDSO mapping handle, stored on `XProcess` after `execve`.
#[derive(Clone, Copy, Debug)]
pub struct VdsoBinding {
    pub base: VirtAddr,
    pub rt_sigreturn: VirtAddr,
}

/// Install the vDSO into a fresh `AddrSpace`.
///
/// - Shared data page (R|U) by-phys-addr at `USER_VDSO_DATA`.
/// - Per-process code page(s) (R-X|U) at `USER_VDSO_BASE`, pre-loaded
///   with the embedded ELF blob and patched so vDSO code can locate the
///   data page position-independently.
pub fn install(uspace: &mut AddrSpace) -> AxResult<VdsoBinding> {
    let blob = image::bytes();
    let code_size = blob.len().div_ceil(PAGE_SIZE_4K) * PAGE_SIZE_4K;
    let code_base = VirtAddr::from_usize(config::USER_VDSO_BASE);
    let offsets = image::offsets();

    let data_pa = virt_to_phys(VirtAddr::from_usize(data::kernel_addr()).into());
    uspace.map_linear(
        VirtAddr::from_usize(config::USER_VDSO_DATA),
        data_pa,
        PAGE_SIZE_4K,
        MappingFlags::READ | MappingFlags::USER,
        PageSize::Size4K,
    )?;

    uspace.map_alloc(
        code_base,
        code_size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        true,
        PageSize::Size4K,
    )?;
    uspace.write(code_base, blob, PageSize::Size4K)?;
    uspace.write(
        code_base + offsets.data_addr_slot,
        &(config::USER_VDSO_DATA as u64).to_ne_bytes(),
        PageSize::Size4K,
    )?;
    uspace.protect(
        code_base,
        code_size,
        MappingFlags::READ | MappingFlags::EXECUTE | MappingFlags::USER,
    )?;

    Ok(VdsoBinding {
        base: code_base,
        rt_sigreturn: code_base + offsets.rt_sigreturn,
    })
}
