use axerrno::{LinuxError, LinuxResult};
use axhal::paging::MappingFlags;
use axtask::{TaskExtRef, current};
use memory_addr::{VirtAddr, VirtAddrRange};
use starry_core::mm::MmapRegion;

use crate::{
    ctypes::{
        MAP_ANONYMOUS, MAP_FIXED, MAP_NORESERVE, MAP_POPULATE, MAP_PRIVATE, MAP_SHARED, MAP_STACK,
        PROT_EXEC, PROT_GROWSDOWN, PROT_GROWSUP, PROT_READ, PROT_WRITE,
    },
    fs::{File, FileLike},
};

bitflags::bitflags! {
    /// `PROT_*` flags for use with [`sys_mmap`].
    ///
    /// For `PROT_NONE`, use `ProtFlags::empty()`.
    #[derive(Debug)]
    struct MmapProt: u32 {
        /// Page can be read.
        const READ = PROT_READ;
        /// Page can be written.
        const WRITE = PROT_WRITE;
        /// Page can be executed.
        const EXEC = PROT_EXEC;
        /// Extend change to start of growsdown vma (mprotect only).
        const GROWDOWN = PROT_GROWSDOWN;
        /// Extend change to start of growsup vma (mprotect only).
        const GROWSUP = PROT_GROWSUP;
    }
}

impl From<MmapProt> for MappingFlags {
    fn from(value: MmapProt) -> Self {
        let mut flags = MappingFlags::USER;
        if value.contains(MmapProt::READ) {
            flags |= MappingFlags::READ;
        }
        if value.contains(MmapProt::WRITE) {
            flags |= MappingFlags::WRITE;
        }
        if value.contains(MmapProt::EXEC) {
            flags |= MappingFlags::EXECUTE;
        }
        flags
    }
}

bitflags::bitflags! {
    /// flags for sys_mmap
    ///
    /// See <https://github.com/bminor/glibc/blob/master/bits/mman.h>
    #[derive(Debug)]
    struct MmapFlags: u32 {
        /// Share changes
        const SHARED = MAP_SHARED;
        /// Changes private; copy pages on write.
        const PRIVATE = MAP_PRIVATE;
        /// Map address must be exactly as requested, no matter whether it is available.
        const FIXED = MAP_FIXED;
        /// Don't use a file.
        const ANONYMOUS = MAP_ANONYMOUS;
        /// Don't check for reservations.
        const NORESERVE = MAP_NORESERVE;
        /// Allocation is for a stack.
        const STACK = MAP_STACK;
        /// Populate the mapping with the file contents.
        const POPULATE = MAP_POPULATE;
    }
}

pub fn sys_mmap(
    addr: usize,
    length: usize,
    prot: u32,
    flags: u32,
    fd: i32,
    offset: isize,
) -> LinuxResult<isize> {
    let curr = current();
    let process_data = curr.task_ext().process_data();
    let mut aspace = process_data.aspace.lock();
    let permission_flags = MmapProt::from_bits_truncate(prot);
    // TODO: check illegal flags for mmap
    // An example is the flags contained none of MAP_PRIVATE, MAP_SHARED, or MAP_SHARED_VALIDATE.
    let map_flags = MmapFlags::from_bits_truncate(flags);

    info!(
        "sys_mmap: addr: {:x?}, length: {:x?}, prot: {:?}, flags: {:?}, fd: {:?}, offset: {:?}",
        addr, length, permission_flags, map_flags, fd, offset
    );

    let start = memory_addr::align_down_4k(addr);
    let end = memory_addr::align_up_4k(addr + length);
    let aligned_length = end - start;
    debug!(
        "start: {:x?}, end: {:x?}, aligned_length: {:x?}",
        start, end, aligned_length
    );

    let start_addr = if map_flags.contains(MmapFlags::FIXED) {
        if start == 0 {
            return Err(LinuxError::EINVAL);
        }
        let dst_addr = VirtAddr::from(start);

        // Remove any existing VMA mappings in the range before unmapping
        let vaddr_range = VirtAddrRange::from_start_size(dst_addr, aligned_length);
        let removed_regions = process_data.remove_overlapping_mmap_regions(vaddr_range);
        debug!(
            "Removed {} overlapping VMA regions for MAP_FIXED",
            removed_regions.len()
        );

        aspace.unmap(dst_addr, aligned_length)?;
        dst_addr
    } else {
        aspace
            .find_free_area(
                VirtAddr::from(start),
                aligned_length,
                VirtAddrRange::new(aspace.base(), aspace.end()),
            )
            .or(aspace.find_free_area(
                aspace.base(),
                aligned_length,
                VirtAddrRange::new(aspace.base(), aspace.end()),
            ))
            .ok_or(LinuxError::ENOMEM)?
    };

    aspace.map_alloc(
        start_addr,
        aligned_length,
        permission_flags.into(),
        map_flags.contains(MmapFlags::POPULATE),
        map_flags.contains(MmapFlags::SHARED),
    )?;

    // Create and add VMA mapping region
    let vaddr_range = VirtAddrRange::from_start_size(start_addr, aligned_length);
    let mmap_region = MmapRegion {
        vaddr_range,
        // FIXME: anonymous || shared should use shm file
        vm_file: if fd == -1 || map_flags.contains(MmapFlags::ANONYMOUS) {
            None
        } else {
            Some(File::from_fd(fd)?.clone_inner())
        },
        file_offset: if offset < 0 { 0 } else { offset },
        prot_flags: prot,
        map_flags: flags,
    };

    let _ = process_data
        .add_mmap_region(mmap_region)
        .map_err(|e| warn!("Failed to add VMA region: {}", e));

    Ok(start_addr.as_usize() as _)
}

pub fn sys_munmap(addr: usize, length: usize) -> LinuxResult<isize> {
    let curr = current();
    let process_data = curr.task_ext().process_data();
    let mut aspace = process_data.aspace.lock();
    let length = memory_addr::align_up_4k(length);
    let start_addr = VirtAddr::from(addr);

    // Remove VMA mapping regions before unmapping
    let vaddr_range = VirtAddrRange::from_start_size(start_addr, length);
    let removed_regions = process_data.remove_overlapping_mmap_regions(vaddr_range);
    debug!(
        "Removed {} VMA regions during munmap",
        removed_regions.len()
    );

    // Re-acquire aspace lock for actual unmapping
    aspace.unmap(start_addr, length)?;
    axhal::arch::flush_tlb(None);
    Ok(0)
}

pub fn sys_mprotect(addr: usize, length: usize, prot: u32) -> LinuxResult<isize> {
    // TODO: implement PROT_GROWSUP & PROT_GROWSDOWN
    let Some(permission_flags) = MmapProt::from_bits(prot) else {
        return Err(LinuxError::EINVAL);
    };
    if permission_flags.contains(MmapProt::GROWDOWN | MmapProt::GROWSUP) {
        return Err(LinuxError::EINVAL);
    }

    let curr = current();
    let process_data = curr.task_ext().process_data();
    let mut aspace = process_data.aspace.lock();
    let length = memory_addr::align_up_4k(length);
    let start_addr = VirtAddr::from(addr);
    aspace.protect(start_addr, length, permission_flags.into())?;

    Ok(0)
}

pub fn sys_msync(_addr: usize, _length: usize, _flags: u32) -> LinuxResult<isize> {
    // let start = memory_addr::align_down_4k(addr);
    // let end = memory_addr::align_up_4k(addr + length);
    // let aligned_length = end - start;
    warn!("sys_msync: not implemented");
    Ok(0)
}
