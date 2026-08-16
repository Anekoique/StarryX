//! Kernel-profile adapter for the [`xvdso`] component.
//!
//! The component owns the external Linux image and shared vvar pages. This
//! module connects its explicit refresh operation to the runtime timer and
//! installs the shared pages into each process address space.

use memory_addr::{PAGE_SIZE_4K, VirtAddr};
use xerrno::XResult;
use xhal::paging::{MappingFlags, PageSize};
use xmm::StaticFrameRange;
use xvma::{Backend, VmSpace};

use crate::config;

/// Per-process vDSO mapping information.
#[derive(Clone, Copy, Debug)]
pub struct VdsoBinding {
    pub base: VirtAddr,
    pub rt_sigreturn: VirtAddr,
}

struct VdsoTimerTick;

#[crate_interface::impl_interface]
impl xruntime::RuntimeTimerIf for VdsoTimerTick {
    fn on_timer_tick() {
        xvdso::refresh_data();
    }
}

/// Maps the shared Linux vDSO image and its read-only data pages.
pub fn install(uspace: &mut VmSpace) -> XResult<VdsoBinding> {
    xvdso::refresh_data();

    let code_base = VirtAddr::from_usize(config::USER_VDSO_BASE);
    let data_base = VirtAddr::from_usize(config::USER_VDSO_DATA);
    debug_assert_eq!(code_base - data_base, xvdso::VVAR_PAGES * PAGE_SIZE_4K);

    let data_frames = StaticFrameRange::from_static_readonly(xvdso::data_page())
        .expect("vvar data page must be page aligned");
    uspace.map(
        data_base,
        data_frames.size(),
        MappingFlags::READ | MappingFlags::USER,
        Backend::static_frames(data_frames, PageSize::Size4K),
    )?;

    #[cfg(target_arch = "loongarch64")]
    {
        let arch_data_base = data_base + 2 * PAGE_SIZE_4K;
        let arch_data_frames = StaticFrameRange::from_static_readonly(xvdso::arch_data_page())
            .expect("architecture vvar page must be page aligned");
        uspace.map(
            arch_data_base,
            arch_data_frames.size(),
            MappingFlags::READ | MappingFlags::USER,
            Backend::static_frames(arch_data_frames, PageSize::Size4K),
        )?;
    }

    let image_frames = StaticFrameRange::from_static_code(xvdso::image())
        .expect("embedded vDSO image must be page aligned");
    uspace.map(
        code_base,
        image_frames.size(),
        MappingFlags::READ | MappingFlags::EXECUTE | MappingFlags::USER,
        Backend::static_frames(image_frames, PageSize::Size4K),
    )?;

    Ok(VdsoBinding {
        base: code_base,
        rt_sigreturn: code_base + xvdso::rt_sigreturn_offset(),
    })
}
