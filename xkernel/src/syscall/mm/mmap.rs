use memory_addr::{MemoryAddr, VirtAddr, VirtAddrRange, align_up_4k};
use xerrno::{LinuxError, LinuxResult};
use xfs::FileFlags;
use xhal::paging::PageSize;
use xtask::current;
use xvma::{Backend, SharedObject};

use crate::{
    fs::{
        fd::{FD_TABLE, File},
        file::FileLike,
    },
    mm::FileVmObject,
    task::{XTaskExt, with_xprocess},
};
use xutils::ctypes::mm::{MmapFlags, MmapProt};

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
    let uspace = xprocess.uspace();
    let mut aspace = uspace.aspace.lock();
    let mut permission_flags = MmapProt::from_bits_truncate(prot);
    let map_flags = MmapFlags::from_bits_truncate(flags);
    debug!(
        "sys_mmap: addr: {:x?}, length: {:x?}, prot: {:?}, flags: {:?}, fd: {:?}, offset: {:?}",
        addr, length, permission_flags, map_flags, fd, offset
    );
    if map_flags.contains(MmapFlags::PRIVATE) && map_flags.contains(MmapFlags::SHARED) {
        return Err(LinuxError::EINVAL);
    }
    if length == 0 {
        return Err(LinuxError::EINVAL);
    }
    if map_flags.intersects(MmapFlags::HUGE | MmapFlags::HUGE_1G) {
        // User-object mappings intentionally use 4 KiB pages until COW and
        // object identity carry an explicit multi-size contract.
        return Err(LinuxError::EINVAL);
    }
    if permission_flags.contains(MmapProt::WRITE) && !permission_flags.contains(MmapProt::READ) {
        permission_flags.insert(MmapProt::READ);
    }
    if !map_flags.contains(MmapFlags::ANONYMOUS) && !FD_TABLE.is_assigned(fd as _) {
        return Err(LinuxError::EBADF);
    }
    if !map_flags.contains(MmapFlags::ANONYMOUS)
        && (offset < 0 || !(offset as usize).is_multiple_of(memory_addr::PAGE_SIZE_4K))
    {
        return Err(LinuxError::EINVAL);
    }

    let page_size = PageSize::Size4K;
    let start = addr.align_down(page_size);
    let end = addr
        .checked_add(length)
        .ok_or(LinuxError::EOVERFLOW)?
        .align_up(page_size);
    let aligned_length = end - start;
    debug!(
        "start: {:x?}, end: {:x?}, aligned_length: {:x?}, page_size: {:?}",
        start, end, aligned_length, page_size
    );

    let private = match map_flags & MmapFlags::TYPE {
        MmapFlags::PRIVATE => true,
        MmapFlags::SHARED | MmapFlags::SHARED_VALIDATE => false,
        _ => return Err(LinuxError::EINVAL),
    };

    // Resolve and validate the file before a fixed mapping can replace an old
    // range. The final backing is then constructed directly by xvma.
    let file_mapping = if map_flags.contains(MmapFlags::ANONYMOUS) {
        None
    } else {
        let mut required = FileFlags::READ;
        if !private && permission_flags.contains(MmapProt::WRITE) {
            required |= FileFlags::WRITE;
        }
        let file =
            File::from_fd(fd, required, FileFlags::empty()).map_err(|_| LinuxError::EACCES)?;
        Some(file.mapping().ok_or(LinuxError::ENODEV)?)
    };

    let fixed = map_flags.intersects(MmapFlags::FIXED | MmapFlags::FIXED_NOREPLACE);
    let start_addr = if fixed {
        if start == 0 {
            return Err(LinuxError::EINVAL);
        }
        VirtAddr::from(start)
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
    let file_object = match file_mapping {
        Some(mapping) => {
            // Every address space that maps a cached file must hear about its
            // truncations, whether the mapping is private or shared.
            uspace.mapped_files.attach(&mapping)?;
            Some(FileVmObject::new(mapping))
        }
        None => None,
    };
    if fixed
        && !map_flags.contains(MmapFlags::FIXED_NOREPLACE)
        && let Err(error) = aspace.unmap(start_addr, aligned_length)
    {
        uspace.mapped_files.prune(&aspace);
        return Err(error.into());
    }
    let backend = match file_object {
        Some(object) if private => Backend::private(object, offset as usize, populate),
        Some(object) => Backend::shared(object, offset as usize, populate),
        None if private => Backend::anonymous(populate),
        // An anonymous shared region needs frames nothing else can reclaim.
        None => Backend::shared(SharedObject::new(aligned_length)?, 0, populate),
    };
    let result = aspace.map(start_addr, aligned_length, permission_flags.into(), backend);
    // MAP_FIXED may have removed the final VMA of another file; a failed map
    // may also leave the just-attached file unused. In both cases the live VMA
    // tree is the authority for which invalidation subscriptions remain.
    uspace.mapped_files.prune(&aspace);
    result?;

    Ok(start_addr.as_usize() as _)
}

/// Unmap files or devices from memory.
///
/// # Arguments
/// * `addr` - Starting address of the mapping to unmap
/// * `length` - Length of the mapping to unmap
pub fn sys_munmap(addr: usize, length: usize) -> LinuxResult<isize> {
    with_xprocess(|xprocess| {
        let uspace = xprocess.uspace();
        let mut aspace = uspace.aspace.lock();
        let length = align_up_4k(length);
        let start_addr = VirtAddr::from(addr);

        aspace.unmap(start_addr, length)?;
        uspace.mapped_files.prune(&aspace);
        xhal::arch::flush_tlb(None);
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
    let Some(mut permission_flags) = MmapProt::from_bits(prot) else {
        return Err(LinuxError::EINVAL);
    };
    debug!(
        "mprotect: addr:{:x?}, length:{:x?}, prot:{:?}",
        addr, length, permission_flags
    );
    if permission_flags.contains(MmapProt::GROWDOWN | MmapProt::GROWSUP) {
        return Err(LinuxError::EINVAL);
    }
    if permission_flags.contains(MmapProt::WRITE) && !permission_flags.contains(MmapProt::READ) {
        permission_flags.insert(MmapProt::READ);
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
pub fn sys_msync(addr: usize, length: usize, flags: u32) -> LinuxResult<isize> {
    const MS_ASYNC: u32 = 1;
    const MS_INVALIDATE: u32 = 2;
    const MS_SYNC: u32 = 4;

    if length == 0
        || !addr.is_multiple_of(memory_addr::PAGE_SIZE_4K)
        || flags & !(MS_ASYNC | MS_INVALIDATE | MS_SYNC) != 0
        || flags & MS_ASYNC != 0 && flags & MS_SYNC != 0
    {
        return Err(LinuxError::EINVAL);
    }
    let end = addr.checked_add(length).ok_or(LinuxError::ENOMEM)?;
    let range = VirtAddrRange::new(VirtAddr::from(addr), VirtAddr::from(end).align_up_4k());
    with_xprocess(|xprocess| {
        xprocess
            .uspace()
            .aspace
            .lock()
            .sync_object_range(range, flags & MS_SYNC != 0)?;
        Ok(0)
    })
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
