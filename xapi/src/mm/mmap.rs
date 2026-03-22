use alloc::vec;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axhal::paging::PageSize;
use axtask::current;
use memory_addr::{MemoryAddr, VirtAddr, VirtAddrRange, align_up_4k};

use xcore::{
    fs::{
        fd::{FD_TABLE, File},
        file::FileLike,
    },
    mm::FileWrapper,
    task::{XTaskExt, with_xprocess},
};
use xutils::ctypes::mm::{MmapFlags, MmapProt};
use xvma::MmapRegion;

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
    let xprocess = XTaskExt::from_task(&current()).xprocess();
    let mut aspace = xprocess.uspace().aspace.lock();
    let mut permission_flags = MmapProt::from_bits_truncate(prot);
    let map_flags = MmapFlags::from_bits_truncate(flags);
    debug!(
        "sys_mmap: addr: {:x?}, length: {:x?}, prot: {:?}, flags: {:?}, fd: {:?}, offset: {:?}",
        addr, length, permission_flags, map_flags, fd, offset
    );
    if map_flags.contains(MmapFlags::PRIVATE) && map_flags.contains(MmapFlags::SHARED) {
        return Err(LinuxError::EINVAL);
    }
    if permission_flags.contains(MmapProt::WRITE) && !permission_flags.contains(MmapProt::READ) {
        permission_flags.insert(MmapProt::READ);
    }
    if !map_flags.contains(MmapFlags::ANONYMOUS) && !FD_TABLE.is_assigned(fd as _) {
        return Err(LinuxError::EBADF);
    }

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

    let start_addr = if map_flags.intersects(MmapFlags::FIXED | MmapFlags::FIXED_NOREPLACE) {
        if start == 0 {
            return Err(LinuxError::EINVAL);
        }
        let dst_addr = VirtAddr::from(start);

        if !map_flags.contains(MmapFlags::FIXED_NOREPLACE) {
            // Remove any existing VMA mappings in the range before unmapping
            let vaddr_range = VirtAddrRange::from_start_size(dst_addr, aligned_length);
            xprocess.remove_overlapping_regions(vaddr_range);
            aspace.unmap(dst_addr, aligned_length)?;
        }
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
    match map_flags & MmapFlags::TYPE {
        MmapFlags::SHARED | MmapFlags::SHARED_VALIDATE => {
            aspace.map_shared(
                start_addr,
                aligned_length,
                permission_flags.into(),
                None,
                page_size,
            )?;
        }
        MmapFlags::PRIVATE => {
            aspace.map_alloc(
                start_addr,
                aligned_length,
                permission_flags.into(),
                populate,
                page_size,
            )?;
        }
        _ => return Err(LinuxError::EINVAL),
    }

    if populate
        || (map_flags.contains(MmapFlags::SHARED) && !map_flags.contains(MmapFlags::ANONYMOUS))
    {
        let file = File::from_fd(fd, FileFlags::READ, FileFlags::empty())
            .map_err(|_| LinuxError::EACCES)?;
        let file_size = file.len()? as usize;
        if offset < 0 || offset as usize >= file_size {
            warn!(
                "offset out of range: {:?}, file_size: {:?}",
                offset, file_size
            );
            return Err(LinuxError::EINVAL);
        }
        let offset = offset as usize;
        let len = core::cmp::min(length, file_size - offset);
        let mut buf = vec![0u8; len];
        file.read_at(&mut buf, offset as u64)?;
        aspace.write(start_addr, &buf, page_size)?;
    } else if !map_flags.contains(MmapFlags::ANONYMOUS) {
        // Create and add VMA mapping region
        let file = File::from_fd(fd, FileFlags::READ, FileFlags::empty())
            .map_err(|_| LinuxError::EACCES)?;
        xprocess.add_region(MmapRegion::new(
            VirtAddrRange::from_start_size(start_addr, aligned_length),
            FileWrapper(file.clone_inner()),
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
    with_xprocess(|xprocess| {
        let mut aspace = xprocess.uspace().aspace.lock();
        let length = align_up_4k(length);
        let start_addr = VirtAddr::from(addr);

        // Remove VMA mapping regions before unmapping
        let vaddr_range = VirtAddrRange::from_start_size(start_addr, length);
        xprocess.remove_overlapping_regions(vaddr_range);

        // Re-acquire aspace lock for actual unmapping
        aspace.unmap(start_addr, length)?;
        axhal::arch::flush_tlb(None);
        Ok(0)
    })
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

    with_xprocess(|xprocess| {
        let mut aspace = xprocess.uspace().aspace.lock();
        let length = align_up_4k(length);
        let start_addr = VirtAddr::from(addr);
        aspace.protect(start_addr, length, permission_flags.into())?;
        Ok(0)
    })
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

pub fn sys_madvise(addr: usize, length: usize, advice: i32) -> LinuxResult<isize> {
    let madvise = xutils::ctypes::mm::Madv::from_repr(advice).ok_or(LinuxError::EINVAL)?;
    info!(
        "[sys_madvise]: addr: {:#x}, len: {:#x}, advice: {:?}",
        addr, length, madvise
    );
    if !addr.is_multiple_of(4096) {
        return Err(LinuxError::EINVAL);
    }
    Ok(0)
}
