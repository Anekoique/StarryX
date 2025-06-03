use alloc::vec;
use axerrno::{LinuxError, LinuxResult};
use axtask::current;
use memory_addr::{VirtAddr, VirtAddrRange};
use starry_core::task::TaskExt;

use crate::{
    fs::{File, FileLike},
    mm::{MmapFlags, MmapProt},
};

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

    let populate = if fd == -1 {
        false
    } else {
        !map_flags.contains(MmapFlags::ANONYMOUS)
    };

    aspace.map_alloc(
        start_addr,
        aligned_length,
        permission_flags.into(),
        populate,
    )?;

    if populate {
        let file = File::from_fd(fd)?;
        let mut file = file.inner();
        let file_size = file.inner().len()? as usize;
        if offset < 0 || offset as usize >= file_size {
            return Err(LinuxError::EINVAL);
        }
        let offset = offset as usize;
        let length = core::cmp::min(length, file_size - offset);
        let mut buf = vec![0u8; length];
        file.read_at(&mut buf, offset as u64)?;
        aspace.write(start_addr, &buf)?;
    }

    // Create and add VMA mapping region
    let vaddr_range = VirtAddrRange::from_start_size(start_addr, aligned_length);
    let mmap_region = MmapRegion {
        vaddr_range,
        vm_file: if fd == -1 {
            None
        } else {
            Some(File::from_fd(fd)?.clone_inner())
        },
        file_offset: if offset < 0 { 0 } else { offset as u64 },
        prot_flags: prot,
        map_flags: flags,
    };

    if let Err(e) = process_data.add_mmap_region(mmap_region) {
        warn!("Failed to add VMA region: {}", e);
        // Continue execution as this is not critical for basic functionality
    }

    Ok(start_addr.as_usize() as _)
}

pub fn sys_munmap(addr: usize, length: usize) -> LinuxResult<isize> {
    let process_data = TaskExt::from_task(&current()).process_data();
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

    let process_data = TaskExt::from_task(&current()).process_data();
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
