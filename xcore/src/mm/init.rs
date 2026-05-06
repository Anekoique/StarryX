use core::ffi::CStr;

use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use axerrno::{AxError, AxResult, LinuxError, LinuxResult};
use axfs_ng::FS_CONTEXT;
use axhal::paging::{MappingFlags, PageSize};
use axmm::{AddrSpace, kernel_aspace};
use kernel_elf_parser::{AUXV_LEN, AuxvEntry, AuxvType, ELFParser, app_stack_region};
use memory_addr::{MemoryAddr, PAGE_SIZE_4K, VirtAddr};
use xmas_elf::{ElfFile, program::SegmentData};

pub fn new_aspace() -> AxResult<AddrSpace> {
    AddrSpace::new_empty(
        VirtAddr::from_usize(crate::config::USER_SPACE_BASE),
        crate::config::USER_SPACE_SIZE,
    )
}

pub fn copy_from_kernel(aspace: &mut AddrSpace) -> AxResult {
    if !cfg!(target_arch = "aarch64") && !cfg!(target_arch = "loongarch64") {
        aspace.copy_mappings_from(&kernel_aspace().lock())?;
    }
    Ok(())
}

pub fn map_elf(
    uspace: &mut AddrSpace,
    elf: &ElfFile,
) -> AxResult<(VirtAddr, [AuxvEntry; AUXV_LEN])> {
    let uspace_base = uspace.base().as_usize();
    let elf_parser = ELFParser::new(
        elf,
        crate::config::USER_INTERP_BASE,
        Some(uspace_base as isize),
        uspace_base,
    )
    .map_err(|_| AxError::InvalidData)?;

    for segement in elf_parser.ph_load() {
        debug!(
            "Mapping ELF segment: [{:#x?}, {:#x?}) flags: {:#x?}",
            segement.vaddr,
            segement.vaddr + segement.memsz as usize,
            segement.flags
        );
        let seg_pad = segement.vaddr.align_offset_4k();
        assert_eq!(seg_pad, segement.offset % PAGE_SIZE_4K);

        let seg_align_size =
            (segement.memsz as usize + seg_pad + PAGE_SIZE_4K - 1) & !(PAGE_SIZE_4K - 1);
        uspace.map_alloc(
            segement.vaddr.align_down_4k(),
            seg_align_size,
            segement.flags,
            true,
            PageSize::Size4K,
        )?;
        let seg_data = elf
            .input
            .get(segement.offset..segement.offset + segement.filesz as usize)
            .ok_or(AxError::InvalidData)?;
        uspace.write(segement.vaddr, seg_data, PageSize::Size4K)?;
        // TDOO: flush the I-cache
    }
    Ok((
        elf_parser.entry().into(),
        elf_parser.auxv_vector(PAGE_SIZE_4K),
    ))
}

pub fn load_file(path: Option<&str>, args: &[String]) -> LinuxResult<(Vec<u8>, Vec<String>)> {
    let path = path
        .or_else(|| args.first().map(String::as_str))
        .ok_or(LinuxError::EINVAL)?;

    if path.ends_with(".sh") {
        let mut new_args = vec!["/musl/busybox".to_string(), "sh".to_string()];
        new_args.extend_from_slice(args);
        return load_file(None, &new_args);
    }

    let file_data = FS_CONTEXT.lock().read(path)?;
    if file_data.starts_with(b"#!") {
        let head = &file_data[2..file_data.len().min(256)];
        let pos = head.iter().position(|c| *c == b'\n').unwrap_or(head.len());
        let line = core::str::from_utf8(&head[..pos]).map_err(|_| AxError::InvalidData)?;

        let new_args: Vec<String> = line
            .trim()
            .splitn(2, |c: char| c.is_ascii_whitespace())
            .map(|s| s.trim_ascii().to_owned())
            .chain(args.iter().cloned())
            .collect();
        return load_file(None, &new_args);
    }

    Ok((file_data, args.to_vec()))
}

/// Result of loading a user application into an address space.
pub struct LoadedApp {
    /// User-space program counter to jump to.
    pub entry: VirtAddr,
    /// Initial user stack pointer.
    pub user_sp: VirtAddr,
    /// Per-process vDSO `rt_sigreturn` address; the caller must publish it
    /// to `XProcess.signal` via `set_default_restorer`.
    pub vdso_rt_sigreturn: VirtAddr,
}

pub fn load_app(
    uspace: &mut AddrSpace,
    file_data: Vec<u8>,
    args: &[String],
    envs: &[String],
    init: bool,
) -> LinuxResult<LoadedApp> {
    let elf = ElfFile::new(&file_data).map_err(|_| LinuxError::ENOEXEC)?;

    if let Some(interp) = elf
        .program_iter()
        .find(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Interp))
    {
        let interp = match interp.get_data(&elf) {
            Ok(SegmentData::Undefined(data)) => data,
            _ => panic!("Invalid data in Interp Elf Program Header"),
        };

        let interp_path = FS_CONTEXT
            .lock()
            .current_dir()
            .absolute_path()?
            .join(
                CStr::from_bytes_with_nul(interp)
                    .ok()
                    .and_then(|it| it.to_str().ok())
                    .ok_or(LinuxError::EINVAL)?,
            )
            .normalize()
            .ok_or(LinuxError::EINVAL)?;
        let interp_path = interp_path.as_str();

        let mut new_args = vec![interp_path.to_owned()];
        new_args.extend_from_slice(args);
        let (file_data, new_args) = load_file(None, &new_args)?;
        return load_app(uspace, file_data, &new_args, envs, init);
    }

    if !init {
        uspace.unmap_user_areas()?;
        axhal::arch::flush_tlb(None);
    }

    let vdso = crate::vdso::install(uspace)?;

    let (entry, mut auxv) = map_elf(uspace, &elf)?;
    if let Some(slot) = auxv
        .iter_mut()
        .find(|slot| slot.get_type() == AuxvType::SYSINFO_EHDR)
    {
        *slot.value_mut_ref() = vdso.base.as_usize();
    }
    let ustack_end = VirtAddr::from_usize(crate::config::USER_STACK_TOP);
    let ustack_size = crate::config::USER_STACK_SIZE;
    let ustack_start = ustack_end - ustack_size;
    debug!(
        "Mapping user stack: {:#x?} -> {:#x?}",
        ustack_start, ustack_end
    );

    let stack_data = app_stack_region(args, envs, &mut auxv, ustack_start, ustack_size);
    uspace.map_alloc(
        ustack_start,
        ustack_size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        true,
        PageSize::Size4K,
    )?;

    let user_sp = ustack_end - stack_data.len();
    uspace.write(user_sp, stack_data.as_slice(), PageSize::Size4K)?;

    let heap_start = VirtAddr::from_usize(crate::config::USER_HEAP_BASE);
    let heap_size = crate::config::USER_HEAP_SIZE;
    uspace.map_alloc(
        heap_start,
        heap_size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        true,
        PageSize::Size4K,
    )?;

    Ok(LoadedApp {
        entry,
        user_sp,
        vdso_rt_sigreturn: vdso.rt_sigreturn,
    })
}
