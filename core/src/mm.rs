//! User address space management.

use core::{
    ffi::CStr,
};

use alloc::{
    borrow::ToOwned,
    collections::BTreeSet,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};
use axerrno::{AxError, AxResult, LinuxError, LinuxResult};
use axfs_ng::{FS_CONTEXT, FsFile};
use axhal::{
    mem::virt_to_phys,
    paging::{MappingFlags, PageSize},
};
use axmm::{AddrSpace, kernel_aspace};
use axsync::{Mutex, RawMutex};
use axtask::current;
use kernel_elf_parser::{AuxvEntry, ELFParser, app_stack_region};
use memory_addr::{MemoryAddr, PAGE_SIZE_4K, PageIter4K, VirtAddr, VirtAddrRange};
use xmas_elf::{ElfFile, program::SegmentData};

use crate::task::TaskExt;

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
        PageSize::Size4K,
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

    // Handle .sh files with busybox sh
    if path.ends_with(".sh") {
        debug!("Loading shell script: {}", path);
        let mut new_args = vec!["/musl/busybox".to_string(), "sh".to_string()];
        new_args.extend_from_slice(args);
        return load_user_app(uspace, None, &new_args, envs);
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
        PageSize::Size4K,
    )?;

    let user_sp = ustack_end - stack_data.len();
    uspace.write(user_sp, stack_data.as_slice(), PageSize::Size4K)?;

    let heap_start = VirtAddr::from_usize(axconfig::plat::USER_HEAP_BASE);
    let heap_size = axconfig::plat::USER_HEAP_SIZE;
    uspace.map_alloc(
        heap_start,
        heap_size,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        true,
        PageSize::Size4K,
    )?;

    Ok((entry, user_sp))
}

/// Memory mapping region information for mmap
pub struct MmapRegion {
    /// Virtual address range of the mapping
    pub vaddr_range: VirtAddrRange,
    /// Associated file descriptor (None for anonymous mappings)
    pub vm_file: Arc<Mutex<FsFile<RawMutex>>>,
    /// Offset in the file
    pub file_offset: isize,
    /// Track which pages have been populated from file
    pub populated_pages: Mutex<BTreeSet<VirtAddr>>,
    /// The page alignment
    pub page_align: PageSize,
}

impl MmapRegion {
    /// Create a new MmapRegion
    pub fn new(
        vaddr_range: VirtAddrRange,
        vm_file: Arc<Mutex<FsFile<RawMutex>>>,
        file_offset: isize,
        page_align: PageSize,
    ) -> Self {
        Self {
            vaddr_range,
            vm_file,
            file_offset,
            populated_pages: Mutex::new(BTreeSet::new()),
            page_align,
        }
    }

    /// Check if the region contains the given address
    pub fn contains(&self, vaddr: VirtAddr) -> bool {
        self.vaddr_range.contains(vaddr)
    }

    /// Check if the region overlaps with the given range
    pub fn overlaps(&self, range: &VirtAddrRange) -> bool {
        self.vaddr_range.overlaps(*range)
    }

    /// Split the region at the given range, returning the parts that don't overlap and the part that does.
    /// Returns (before_part, overlap_part, after_part)
    pub fn split_at_range(
        &self,
        range: &VirtAddrRange,
    ) -> (Option<Self>, Option<Self>, Option<Self>) {
        let self_start = self.vaddr_range.start;
        let self_end = self.vaddr_range.end;
        let split_start = range.start;
        let split_end = range.end;

        let before = if self_start < split_start {
            Some(Self {
                vaddr_range: VirtAddrRange::from_start_size(self_start, split_start - self_start),
                vm_file: self.vm_file.clone(),
                file_offset: self.file_offset,
                populated_pages: Mutex::new(BTreeSet::new()),
                page_align: self.page_align,
            })
        } else {
            None
        };

        let after = if split_end < self_end {
            Some(Self {
                vaddr_range: VirtAddrRange::from_start_size(split_end, self_end - split_end),
                vm_file: self.vm_file.clone(),
                file_offset: self.file_offset + (split_end - self_start) as isize,
                populated_pages: Mutex::new(BTreeSet::new()),
                page_align: self.page_align,
            })
        } else {
            None
        };

        let overlap_start = self_start.max(split_start);
        let overlap_end = self_end.min(split_end);
        let overlap = if overlap_start < overlap_end {
            Some(Self {
                vaddr_range: VirtAddrRange::from_start_size(
                    overlap_start,
                    overlap_end - overlap_start,
                ),
                vm_file: self.vm_file.clone(),
                file_offset: self.file_offset + (overlap_start - self_start) as isize,
                populated_pages: Mutex::new(BTreeSet::new()),
                page_align: self.page_align,
            })
        } else {
            None
        };

        (before, overlap, after)
    }

    /// Get the overlapping part with the given range
    pub fn get_overlap(&self, range: &VirtAddrRange) -> Option<Self> {
        if !self.overlaps(range) {
            return None;
        }
        let overlap_start = self.vaddr_range.start.max(range.start);
        let overlap_end = self.vaddr_range.end.min(range.end);

        Some(Self {
            vaddr_range: VirtAddrRange::from_start_size(overlap_start, overlap_end - overlap_start),
            vm_file: self.vm_file.clone(),
            file_offset: self.file_offset + (overlap_start - self.vaddr_range.start) as isize,
            populated_pages: Mutex::new(BTreeSet::new()),
            page_align: self.page_align,
        })
    }

    /// Populate a page from the file
    pub fn get_buf(&self, vaddr: VirtAddr) -> LinuxResult<Vec<u8>> {
        let page_addr = vaddr.align_down(self.page_align);

        // Check if this page has already been populated
        if self.populated_pages.lock().contains(&page_addr) {
            return Err(LinuxError::EEXIST);
        }

        let page_offset = page_addr - self.vaddr_range.start;
        let file_offset = self.file_offset + page_offset as isize;
        if file_offset < 0 || file_offset >= self.vm_file.lock().len()? as isize {
            return Err(LinuxError::EINVAL);
        }

        let buf_size = core::cmp::min(self.page_align as usize, self.vaddr_range.end - page_addr);
        let mut buf = vec![0u8; buf_size];
        self.vm_file.lock().read_at(&mut buf, file_offset as u64)?;
        self.populated_pages.lock().insert(page_addr);

        Ok(buf)
    }
}

impl Clone for MmapRegion {
    fn clone(&self) -> Self {
        let populated_pages_clone = {
            let pages = self.populated_pages.lock();
            Mutex::new(pages.clone())
        };

        Self {
            vaddr_range: self.vaddr_range,
            vm_file: self.vm_file.clone(),
            file_offset: self.file_offset,
            populated_pages: populated_pages_clone,
            page_align: self.page_align,
        }
    }
}

/// Virtual Memory Area (VMA) mapping manager
/// Maintains a sorted list of non-overlapping memory regions for efficient lookup
#[derive(Default, Clone)]
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
    pub fn add_region(&mut self, region: MmapRegion) -> LinuxResult<()> {
        // Check for overlaps
        if self.regions.iter().any(|r| r.overlaps(&region.vaddr_range)) {
            return Err(LinuxError::EFAULT);
        }

        // Find insertion position to maintain sorted order
        let pos = self
            .regions
            .binary_search_by_key(&region.vaddr_range.start, |r| r.vaddr_range.start)
            .unwrap_or_else(|e| e);

        self.regions.insert(pos, region);
        Ok(())
    }

    /// Find the memory mapping region that contains the given virtual address
    /// Returns None if no mapping found
    pub fn find_region_by_addr(&self, vaddr: VirtAddr) -> Option<&MmapRegion> {
        // Binary search for efficiency
        let idx = self.regions.binary_search_by(|r| {
            if vaddr < r.vaddr_range.start {
                core::cmp::Ordering::Greater
            } else if vaddr >= r.vaddr_range.end {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Equal
            }
        });

        idx.ok().map(|i| &self.regions[i])
    }

    /// Remove all regions that overlap with the given address range
    /// Returns the removed regions
    pub fn remove_overlapping_regions(&mut self, vaddr_range: VirtAddrRange) -> Vec<MmapRegion> {
        let mut removed = Vec::new();
        let mut retained = Vec::new();

        for region in self.regions.drain(..) {
            if region.overlaps(&vaddr_range) {
                // Keep the non-overlapping parts, and save the overlapping part
                let (before, overlap, after) = region.split_at_range(&vaddr_range);
                if let Some(overlap) = overlap {
                    removed.push(overlap);
                }
                if let Some(before) = before {
                    retained.push(before);
                }
                if let Some(after) = after {
                    retained.push(after);
                }
            } else {
                retained.push(region);
            }
        }

        // Restore retained regions in sorted order
        self.regions = retained;
        self.regions.sort_by_key(|r| r.vaddr_range.start);

        removed
    }

    /// Populate a page from file for the given virtual address
    pub fn get_buf(&self, vaddr: VirtAddr) -> LinuxResult<Vec<u8>> {
        self.find_region_by_addr(vaddr)
            .ok_or(LinuxError::EFAULT)
            .and_then(|region| region.get_buf(vaddr))
    }

    /// Populate file-backed pages in the address space
    pub fn populate_file_pages(&self, vaddr: VirtAddr, len: usize) -> LinuxResult<()> {
        let start_addr = vaddr.align_down_4k();
        let end_addr = (vaddr + len).align_up_4k();
        let aspace = TaskExt::from_task(&current()).process_data().aspace.lock();

        for page_addr in PageIter4K::new(start_addr, end_addr).unwrap() {
            if let Some(region) = self.find_region_by_addr(page_addr) {
                // Skip if this page has already been populated in this region
                if region.populated_pages.lock().contains(&page_addr) {
                    continue;
                }

                // File-backed page, read from file and write to aspace
                match region.get_buf(page_addr) {
                    Ok(page_data) => {
                        debug!("Populating page: {:#x}", page_addr);
                        aspace.write(page_addr, &page_data, region.page_align)?;
                    }
                    Err(LinuxError::EEXIST) => {
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(())
    }

    /// Clear all mappings
    pub fn clear(&mut self) {
        self.regions.clear();
    }
}
