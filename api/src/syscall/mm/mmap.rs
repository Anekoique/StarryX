use alloc::vec;
use axerrno::{LinuxError, LinuxResult};
use axhal::paging::PageSize;
use axtask::current;
use memory_addr::{MemoryAddr, VirtAddr, VirtAddrRange, align_up_4k};
use starry_core::{mm::MmapRegion, task::TaskExt};

use crate::{
    ctypes::mm::{MmapFlags, MmapProt},
    fs::{File, FileLike},
};

/// Map files or devices into memory.
///
/// # Arguments
/// * `addr` - Hint for the starting address of the mapping
/// * `length` - Length of the mapping
/// * `prot` - Memory protection flags (PROT_READ, PROT_WRITE, PROT_EXEC)
/// * `flags` - Mapping flags (MAP_PRIVATE, MAP_SHARED, MAP_ANONYMOUS, etc.)
/// * `fd` - File descriptor (-1 for anonymous mapping)
/// * `offset` - Offset in the file
pub fn sys_mmap(
    addr: usize,
    length: usize,
    prot: u32,
    flags: u32,
    fd: i32,
    offset: isize,
) -> LinuxResult<isize> {
    let process_data = TaskExt::from_task(&current()).process_data();
    let mut aspace = process_data.aspace.lock();
    let permission_flags = MmapProt::from_bits_truncate(prot);
    let map_flags = MmapFlags::from_bits_truncate(flags);

    info!(
        "sys_mmap: addr: {:x?}, length: {:x?}, prot: {:?}, flags: {:?}, fd: {:?}, offset: {:?}",
        addr, length, permission_flags, map_flags, fd, offset
    );

    let page_size = if map_flags.contains(MmapFlags::HUGE_1G) {
        PageSize::Size1G
    } else if map_flags.contains(MmapFlags::HUGE) {
        PageSize::Size2M
    } else {
        PageSize::Size4K
    };
    let start = addr.align_down(page_size);
    let end = (addr + length).align_up(page_size);
    let aligned_length = end - start;
    debug!(
        "start: {:x?}, end: {:x?}, aligned_length: {:x?}, page_size: {:?}",
        start, end, aligned_length, page_size
    );

    let start_addr = if map_flags.contains(MmapFlags::FIXED) {
        if start == 0 {
            return Err(LinuxError::EINVAL);
        }
        let dst_addr = VirtAddr::from(start);

        // Remove any existing VMA mappings in the range before unmapping
        let vaddr_range = VirtAddrRange::from_start_size(dst_addr, aligned_length);
        process_data.remove_overlapping_regions(vaddr_range);
        aspace.unmap(dst_addr, aligned_length)?;
        dst_addr
    } else {
        aspace
            .find_free_area(
                VirtAddr::from(start),
                aligned_length,
                VirtAddrRange::new(aspace.base(), aspace.end()),
                page_size,
            )
            .or(aspace.find_free_area(
                aspace.base(),
                aligned_length,
                VirtAddrRange::new(aspace.base(), aspace.end()),
                page_size,
            ))
            .ok_or(LinuxError::ENOMEM)?
    };

    let populate = map_flags.contains(MmapFlags::POPULATE);
    aspace.map_alloc(
        start_addr,
        aligned_length,
        permission_flags.into(),
        populate,
        page_size,
    )?;

    if populate {
        let file = File::from_fd(fd)?;
        let mut file = file.inner();
        let file_size = file.inner().len()? as usize;
        if offset < 0 || offset as usize >= file_size {
            return Err(LinuxError::EINVAL);
        }
        let offset = offset as usize;
        let len = core::cmp::min(length, file_size - offset);
        let mut buf = vec![0u8; len];
        file.read_at(&mut buf, offset as u64)?;
        aspace.write(start_addr, &buf, page_size)?;
    } else if !map_flags.contains(MmapFlags::ANONYMOUS) {
        // Create and add VMA mapping region
        process_data.add_region(MmapRegion::new(
            VirtAddrRange::from_start_size(start_addr, aligned_length),
            File::from_fd(fd)?.clone_inner(),
            if offset < 0 { 0 } else { offset },
            page_size,
        ))?;
    }

    Ok(start_addr.as_usize() as _)
}

/// Unmap files or devices from memory.
///
/// # Arguments
/// * `addr` - Starting address of the mapping to unmap
/// * `length` - Length of the mapping to unmap
pub fn sys_munmap(addr: usize, length: usize) -> LinuxResult<isize> {
    let process_data = TaskExt::from_task(&current()).process_data();
    let mut aspace = process_data.aspace.lock();
    let length = align_up_4k(length);
    let start_addr = VirtAddr::from(addr);

    // Remove VMA mapping regions before unmapping
    let vaddr_range = VirtAddrRange::from_start_size(start_addr, length);
    process_data.remove_overlapping_regions(vaddr_range);

    // Re-acquire aspace lock for actual unmapping
    aspace.unmap(start_addr, length)?;
    axhal::arch::flush_tlb(None);
    Ok(0)
}

/// Change memory protection on a mapping.
///
/// # Arguments
/// * `addr` - Starting address of the memory region
/// * `length` - Length of the memory region
/// * `prot` - New protection flags (PROT_READ, PROT_WRITE, PROT_EXEC)
pub fn sys_mprotect(addr: usize, length: usize, prot: u32) -> LinuxResult<isize> {
    // TODO: implement PROT_GROWSUP & PROT_GROWSDOWN
    let Some(permission_flags) = MmapProt::from_bits(prot) else {
        return Err(LinuxError::EINVAL);
    };
    debug!(
        "mprotect: addr:{:x?}, length:{:x?}, prot:{:?}",
        addr, length, permission_flags
    );
    if permission_flags.contains(MmapProt::GROWDOWN | MmapProt::GROWSUP) {
        return Err(LinuxError::EINVAL);
    }

    let process_data = TaskExt::from_task(&current()).process_data();
    let mut aspace = process_data.aspace.lock();
    let length = align_up_4k(length);
    let start_addr = VirtAddr::from(addr);
    aspace.protect(start_addr, length, permission_flags.into())?;
    drop(aspace);
    process_data.populate_file_pages(start_addr, length)?;
    Ok(0)
}

/// Synchronize a file with a memory map.
///
/// # Arguments
/// * `_addr` - Starting address of the memory region (currently unused)
/// * `_length` - Length of the memory region (currently unused)
/// * `_flags` - Synchronization flags (currently unused)
pub fn sys_msync(_addr: usize, _length: usize, _flags: u32) -> LinuxResult<isize> {
    // let start = memory_addr::align_down_4k(addr);
    // let end = memory_addr::align_up_4k(addr + length);
    // let aligned_length = end - start;
    warn!("sys_msync: not implemented");
    Ok(0)
}
