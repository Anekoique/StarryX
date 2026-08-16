use alloc::{sync::Arc, vec};

use memory_addr::{MemoryAddr, VirtAddr, VirtAddrRange, align_up_4k};
use xerrno::{LinuxError, LinuxResult};
use xfs::FileFlags;
use xhal::paging::{MappingFlags, PageSize};
use xtask::current;
use xvma::{Backend, SharedObject};

use crate::{
    fs::{
        fd::{FD_TABLE, File},
        file::FileLike,
    },
    mm::FileWrapper,
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

    // Resolve and validate the file before a fixed mapping can replace an old
    // range. The final backing is then constructed directly by xvma.
    let file = if map_flags.contains(MmapFlags::ANONYMOUS) {
        None
    } else {
        Some(
            File::from_fd(fd, FileFlags::READ, FileFlags::empty())
                .map_err(|_| LinuxError::EACCES)?,
        )
    };
    let eager_file = file.is_some()
        && (map_flags.contains(MmapFlags::POPULATE)
            || map_flags.intersects(MmapFlags::SHARED | MmapFlags::SHARED_VALIDATE));
    let file_len = match (&file, eager_file) {
        (Some(file), true) => {
            Some(usize::try_from(file.len()?).map_err(|_| LinuxError::EOVERFLOW)?)
        }
        _ => None,
    };
    if eager_file && file_len.is_some_and(|len| offset as usize >= len) {
        return Err(LinuxError::EINVAL);
    }

    let start_addr = if map_flags.intersects(MmapFlags::FIXED | MmapFlags::FIXED_NOREPLACE) {
        if start == 0 {
            return Err(LinuxError::EINVAL);
        }
        let dst_addr = VirtAddr::from(start);

        if !map_flags.contains(MmapFlags::FIXED_NOREPLACE) {
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
    let mut installed = false;
    let map_result = (|| -> LinuxResult<()> {
        match (map_flags & MmapFlags::TYPE, file) {
            (MmapFlags::PRIVATE, None) => {
                aspace.map(
                    start_addr,
                    aligned_length,
                    permission_flags.into(),
                    Backend::anonymous(populate),
                )?;
                installed = true;
                Ok(())
            }
            (MmapFlags::PRIVATE, Some(file)) => {
                aspace.map(
                    start_addr,
                    aligned_length,
                    permission_flags.into(),
                    Backend::file(
                        Arc::new(FileWrapper(file.clone_inner())),
                        offset as usize,
                        populate,
                    ),
                )?;
                installed = true;
                Ok(())
            }
            (MmapFlags::SHARED | MmapFlags::SHARED_VALIDATE, file) => {
                let final_flags: MappingFlags = permission_flags.into();
                let load_flags = if file.is_some() {
                    final_flags | MappingFlags::READ | MappingFlags::WRITE
                } else {
                    final_flags
                };
                let object = SharedObject::new(aligned_length)?;
                aspace.map(
                    start_addr,
                    aligned_length,
                    load_flags,
                    Backend::shared(object, 0),
                )?;
                installed = true;
                if let Some(file) = file {
                    // Compatibility contract: one eager snapshot object per
                    // mmap, shared with forks but not coherent with another
                    // mmap and not written back.
                    let file_size = file_len.expect("file length was resolved");
                    let count = core::cmp::min(length, file_size - offset as usize);
                    let mut buffer = vec![0_u8; count];
                    let read = file.read_at(&mut buffer, offset as u64)?;
                    if read > count {
                        return Err(LinuxError::EIO);
                    }
                    aspace.write_bytes(start_addr, &buffer[..read])?;
                    if load_flags != final_flags {
                        aspace.protect(start_addr, aligned_length, final_flags)?;
                    }
                }
                Ok(())
            }
            _ => Err(LinuxError::EINVAL),
        }
    })();
    if let Err(error) = map_result {
        // The address range was reserved above; do not leak it when file
        // validation, reading, or policy attachment fails.
        if installed {
            let _ = aspace.unmap(start_addr, aligned_length);
        }
        return Err(error);
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

        aspace.unmap(start_addr, length)?;
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
