//! User address space management.

use core::ffi::CStr;

use alloc::{borrow::ToOwned, string::String, sync::Arc, vec, vec::Vec};
use axerrno::{AxError, AxResult, LinuxError, LinuxResult};
use axfs_ng::{FS_CONTEXT, File};
use axhal::{mem::virt_to_phys, paging::MappingFlags};
use axmm::{AddrSpace, kernel_aspace};
use axsync::{Mutex, RawMutex};
use kernel_elf_parser::{AuxvEntry, ELFParser, app_stack_region};
use memory_addr::{MemoryAddr, PAGE_SIZE_4K, VirtAddr, VirtAddrRange};
use xmas_elf::{ElfFile, program::SegmentData};

/// Creates a new empty user address space.
pub fn new_user_aspace_empty() -> AxResult<AddrSpace> {
    AddrSpace::new_empty(
        VirtAddr::from_usize(axconfig::plat::USER_SPACE_BASE),
        axconfig::plat::USER_SPACE_SIZE,
    )
}

/// If the target architecture requires it, the kernel portion of the address
/// space will be copied to the user address space.
pub fn copy_from_kernel(aspace: &mut AddrSpace) -> AxResult {
    if !cfg!(target_arch = "aarch64") && !cfg!(target_arch = "loongarch64") {
        // ARMv8 (aarch64) and LoongArch64 use separate page tables for user space
        // (aarch64: TTBR0_EL1, LoongArch64: PGDL), so there is no need to copy the
        // kernel portion to the user page table.
        aspace.copy_mappings_from(&kernel_aspace().lock())?;
    }
    Ok(())
}

/// Map the signal trampoline to the user address space.
pub fn map_trampoline(aspace: &mut AddrSpace) -> AxResult {
    let signal_trampoline_paddr = virt_to_phys(axsignal::arch::signal_trampoline_address().into());
    aspace.map_linear(
        axconfig::plat::SIGNAL_TRAMPOLINE.into(),
        signal_trampoline_paddr,
        PAGE_SIZE_4K,
        MappingFlags::READ | MappingFlags::EXECUTE | MappingFlags::USER,
    )?;
    Ok(())
}

/// Map the elf file to the user address space.
///
/// # Arguments
/// - `uspace`: The address space of the user app.
/// - `elf`: The elf file.
///
/// # Returns
/// - The entry point of the user app.
fn map_elf(uspace: &mut AddrSpace, elf: &ElfFile) -> AxResult<(VirtAddr, [AuxvEntry; 17])> {
    let uspace_base = uspace.base().as_usize();
    let elf_parser = ELFParser::new(
        elf,
        axconfig::plat::USER_INTERP_BASE,
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
            false,
        )?;
        let seg_data = elf
            .input
            .get(segement.offset..segement.offset + segement.filesz as usize)
            .ok_or(AxError::InvalidData)?;
        uspace.write(segement.vaddr, seg_data)?;
        // TDOO: flush the I-cache
    }

    Ok((
        elf_parser.entry().into(),
        elf_parser.auxv_vector(PAGE_SIZE_4K),
    ))
}

/// Load the user app to the user address space.
///
/// # Arguments
/// - `uspace`: The address space of the user app.
/// - `args`: The arguments of the user app. The first argument is the path of the user app.
/// - `envs`: The environment variables of the user app.
///
/// # Returns
/// - The entry point of the user app.
/// - The stack pointer of the user app.
pub fn load_user_app(
    uspace: &mut AddrSpace,
    path: Option<&str>,
    args: &[String],
    envs: &[String],
) -> LinuxResult<(VirtAddr, VirtAddr)> {
    debug!("load_user_app: {:?}, {:?}, {:?}", path, args, envs);
    let path = path
        .or_else(|| args.first().map(String::as_str))
        .ok_or(AxError::InvalidInput)?;
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
        return load_user_app(uspace, None, &new_args, envs);
    }

    let elf = ElfFile::new(&file_data).map_err(|_| AxError::InvalidData)?;

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

        debug!("Loading interpreter: {}", interp_path);

        // Set the first argument to the path of the user app.
        let mut new_args = vec![interp_path.to_owned()];
        new_args.extend_from_slice(args);
        return load_user_app(uspace, None, &new_args, envs);
    }

    let (entry, mut auxv) = map_elf(uspace, &elf)?;
    // The user stack is divided into two parts:
    // `ustack_start` -> `ustack_pointer`: It is the stack space that users actually read and write.
    // `ustack_pointer` -> `ustack_end`: It is the space that contains the arguments, environment variables and auxv passed to the app.
    //  When the app starts running, the stack pointer points to `ustack_pointer`.
    let ustack_end = VirtAddr::from_usize(axconfig::plat::USER_STACK_TOP);
    let ustack_size = axconfig::plat::USER_STACK_SIZE;
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
        false,
    )?;

    let user_sp = ustack_end - stack_data.len();
    uspace.write(user_sp, stack_data.as_slice())?;

    let heap_start = VirtAddr::from_usize(axconfig::plat::USER_HEAP_BASE);
    let heap_size = axconfig::plat::USER_HEAP_SIZE;
    uspace.map_alloc(
        heap_start,
        heap_size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        true,
        false,
    )?;

    Ok((entry, user_sp))
}

#[percpu::def_percpu]
static mut ACCESSING_USER_MEM: bool = false;

/// Enables scoped access into user memory, allowing page faults to occur inside
/// kernel.
pub fn access_user_memory<R>(f: impl FnOnce() -> R) -> R {
    ACCESSING_USER_MEM.with_current(|v| {
        *v = true;
        let result = f();
        *v = false;
        result
    })
}

/// Check if the current thread is accessing user memory.
pub fn is_accessing_user_memory() -> bool {
    ACCESSING_USER_MEM.read_current()
}

/// Memory mapping region information for mmap
#[derive(Clone)]
pub struct MmapRegion {
    /// Virtual address range of the mapping
    pub vaddr_range: VirtAddrRange,
    /// Associated file descriptor (None for anonymous mappings)
    pub vm_file: Option<Arc<Mutex<File<RawMutex>>>>,
    /// Offset in the file
    pub file_offset: u64,
    /// Protection flags (PROT_READ, PROT_WRITE, PROT_EXEC)
    pub prot_flags: u32,
    /// Mapping flags (MAP_SHARED, MAP_PRIVATE, etc.)
    pub map_flags: u32,
}

/// Virtual Memory Area (VMA) mapping manager
/// Maintains a sorted list of non-overlapping memory regions for efficient lookup
#[derive(Default)]
pub struct VmaMapping {
    /// Sorted list of memory mapping regions (sorted by start address)
    regions: Vec<MmapRegion>,
}

impl VmaMapping {
    /// Create a new empty VMA mapping
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Add a new memory mapping region
    /// Returns error if the region overlaps with existing mappings
    pub fn add_region(&mut self, region: MmapRegion) -> Result<(), &'static str> {
        let start = region.vaddr_range.start.as_usize();
        let end = region.vaddr_range.end.as_usize();

        // Check for overlaps with existing regions
        for existing in &self.regions {
            let existing_start = existing.vaddr_range.start.as_usize();
            let existing_end = existing.vaddr_range.end.as_usize();

            if !(end <= existing_start || start >= existing_end) {
                return Err("Region overlaps with existing mapping");
            }
        }

        // Find insertion position to maintain sorted order
        let insert_pos = self
            .regions
            .binary_search_by_key(&start, |r| r.vaddr_range.start.as_usize())
            .unwrap_or_else(|e| e);

        self.regions.insert(insert_pos, region);
        Ok(())
    }

    /// Remove a memory mapping region by virtual address range
    pub fn remove_region(&mut self, vaddr_range: VirtAddrRange) -> Option<MmapRegion> {
        let start = vaddr_range.start.as_usize();

        if let Some(pos) = self.regions.iter().position(|r| {
            r.vaddr_range.start.as_usize() == start && r.vaddr_range.size() == vaddr_range.size()
        }) {
            Some(self.regions.remove(pos))
        } else {
            None
        }
    }

    /// Find the memory mapping region that contains the given virtual address
    /// Returns None if no mapping found
    pub fn find_region_by_addr(&self, vaddr: VirtAddr) -> Option<&MmapRegion> {
        let addr = vaddr.as_usize();

        // Linear search through sorted regions (similar complexity to find_area)
        for region in &self.regions {
            let start = region.vaddr_range.start.as_usize();
            let end = region.vaddr_range.end.as_usize();

            if addr >= start && addr < end {
                return Some(region);
            }

            // Since regions are sorted, we can break early if we've passed the target
            if start > addr {
                break;
            }
        }

        None
    }

    /// Get all regions that overlap with the given address range
    pub fn find_overlapping_regions(&self, vaddr_range: VirtAddrRange) -> Vec<&MmapRegion> {
        let start = vaddr_range.start.as_usize();
        let end = vaddr_range.end.as_usize();
        let mut overlapping = Vec::new();

        for region in &self.regions {
            let region_start = region.vaddr_range.start.as_usize();
            let region_end = region.vaddr_range.end.as_usize();

            // Check for overlap: !(end <= region_start || start >= region_end)
            if end > region_start && start < region_end {
                overlapping.push(region);
            }

            // Early termination since regions are sorted
            if region_start >= end {
                break;
            }
        }

        overlapping
    }

    /// Remove all regions that overlap with the given address range
    /// Returns the removed regions
    pub fn remove_overlapping_regions(&mut self, vaddr_range: VirtAddrRange) -> Vec<MmapRegion> {
        let start = vaddr_range.start.as_usize();
        let end = vaddr_range.end.as_usize();
        let mut removed = Vec::new();

        self.regions.retain(|region| {
            let region_start = region.vaddr_range.start.as_usize();
            let region_end = region.vaddr_range.end.as_usize();

            // Check for overlap
            if end > region_start && start < region_end {
                removed.push(region.clone());
                false // Remove this region
            } else {
                true // Keep this region
            }
        });

        removed
    }

    /// Get all mapping regions (for debugging/inspection)
    pub fn get_all_regions(&self) -> &[MmapRegion] {
        &self.regions
    }

    /// Clear all mappings
    pub fn clear(&mut self) {
        self.regions.clear();
    }
}
