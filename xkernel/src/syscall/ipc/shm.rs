//! Shared memory system calls implementation.
use alloc::sync::Arc;

use memory_addr::{PAGE_SIZE_4K, VirtAddr, VirtAddrRange};
use page_table_entry::MappingFlags;
use xerrno::{LinuxError, LinuxResult};
use xhal::paging::PageSize;
use xsync::Mutex;
use xvma::{Backend, SharedObject, VmSpace};

use crate::{
    ipc::{IPC_MANAGER, ShmInfo, ShmSegment},
    task::{with_process, with_uspace},
    with_ipc_manager,
};
use xuspace::{UserPtr, nullable};
use xutils::ctypes::{
    IPC_PRIVATE, IPC_RMID, IPC_SET, IPC_STAT,
    ipc::{ShmAtFlags, ShmGetFlags},
};

/// Convert shared memory flags to mapping flags
fn convert_shm_flags_to_mapping(shmflg: usize) -> LinuxResult<MappingFlags> {
    let mut mapping_flags = MappingFlags::from_name("USER").ok_or(LinuxError::EINVAL)?;

    if (shmflg as u32) & ShmGetFlags::SHM_R.bits() != 0 {
        mapping_flags.insert(MappingFlags::READ);
    }
    if (shmflg as u32) & ShmGetFlags::SHM_W.bits() != 0 {
        mapping_flags.insert(MappingFlags::WRITE);
    }

    Ok(mapping_flags)
}

/// Get shared memory statistics (example using macro)
pub fn get_shm_stats() -> (usize, usize) {
    with_ipc_manager!(shm, manager, {
        (manager.segment_count(), manager.total_pages())
    })
}

/// Create or get a shared memory segment
///
/// # Arguments
/// * `key` - IPC key for the segment
/// * `size` - Size of the segment in bytes
/// * `shmflg` - Flags controlling creation and permissions
pub fn sys_shmget(key: i32, size: usize, shmflg: usize) -> LinuxResult<isize> {
    // Validate basic parameters
    let page_num = memory_addr::align_up_4k(size) / PAGE_SIZE_4K;
    if page_num == 0 {
        return Err(LinuxError::EINVAL);
    }

    let mapping_flags = convert_shm_flags_to_mapping(shmflg)?;
    let cur_pid = with_process(|process| process.pid());

    IPC_MANAGER.with_shm(|shm_manager| {
        // Validate system limits
        shm_manager.validate_segment_params(size, shmflg as u32)?;

        // Check if segment with this key already exists
        if key != IPC_PRIVATE
            && let Some(shmid) = shm_manager.get_shmid_by_key(key)
        {
            let segment = shm_manager
                .get_segment_by_shmid(shmid)
                .ok_or(LinuxError::EINVAL)?;
            let mut segment = segment.lock();
            return segment.try_update(size, mapping_flags, cur_pid);
        }

        // Create new shared memory segment
        let shmid = shm_manager.allocate_shmid();
        let segment = Arc::new(Mutex::new(ShmSegment::new(
            key,
            shmid,
            size,
            mapping_flags,
            cur_pid,
        )));

        shm_manager.insert_key_shmid(key, shmid);
        shm_manager.insert_shmid_segment(shmid, segment);

        Ok(shmid as isize)
    })
}

/// Find available virtual address range for shared memory mapping
fn find_mapping_address(
    aspace: &mut VmSpace,
    requested_addr: usize,
    length: usize,
) -> LinuxResult<VirtAddr> {
    let start_aligned = memory_addr::align_down_4k(requested_addr);
    let range = VirtAddrRange::new(aspace.base(), aspace.end());

    // Try requested address first
    let start_addr = if requested_addr != 0 {
        aspace
            .find_free_area(
                VirtAddr::from(start_aligned),
                length,
                range,
                PageSize::Size4K,
            )
            .or_else(|| aspace.find_free_area(aspace.base(), length, range, PageSize::Size4K))
    } else {
        aspace.find_free_area(aspace.base(), length, range, PageSize::Size4K)
    };

    start_addr.ok_or(LinuxError::ENOMEM)
}

fn map_segment(
    aspace: &mut VmSpace,
    segment: &mut ShmSegment,
    range: VirtAddrRange,
    mapping_flags: MappingFlags,
) -> LinuxResult<()> {
    let new_object = segment.object.is_none();
    let object = match &segment.object {
        Some(object) => object.clone(),
        None => SharedObject::new(range.size())?,
    };
    aspace.map(
        range.start,
        range.size(),
        mapping_flags,
        // Populate eagerly: a segment's frames are allocated up front, so a
        // later fault could only report a shortage that already happened.
        Backend::shared(object.clone(), 0, true),
    )?;
    if new_object {
        segment.set_object(object);
    }

    Ok(())
}

/// Attach a shared memory segment to the calling process.
pub fn sys_shmat(shmid: i32, addr: usize, shmflg: u32) -> LinuxResult<isize> {
    let shm_flg = ShmAtFlags::from_bits_truncate(shmflg);
    let cur_pid = with_process(|process| process.pid());

    IPC_MANAGER.with_shm(|shm_manager| {
        let segment = shm_manager
            .get_segment_by_shmid(shmid)
            .ok_or(LinuxError::EINVAL)?;
        let mut segment = segment.lock();
        let mut mapping_flags = segment.mapping_flags;
        if shm_flg.contains(ShmAtFlags::SHM_RDONLY) {
            mapping_flags.remove(MappingFlags::WRITE);
        }

        with_uspace(|uspace| {
            let mut aspace = uspace.aspace.lock();
            let length = segment.page_num * PAGE_SIZE_4K;
            if segment.is_attached(cur_pid) {
                return Err(LinuxError::EINVAL);
            }

            let start_addr = find_mapping_address(&mut aspace, addr, length)?;
            let range = VirtAddrRange::from_start_size(start_addr, length);
            info!(
                "Process {} attaching shared memory: shmid={}, addr={:#x}, size={}, flags={:#x?}",
                cur_pid,
                shmid,
                start_addr.as_usize(),
                length,
                mapping_flags
            );

            map_segment(&mut aspace, &mut segment, range, mapping_flags)?;
            if let Err(error) = segment.attach_process(cur_pid, range) {
                aspace.unmap(start_addr, length)?;
                return Err(error);
            }
            shm_manager.insert_shmid_vaddr(cur_pid, segment.shmid, start_addr);

            Ok(start_addr.as_usize() as isize)
        })
    })
}

/// Control operations on shared memory segments
///
/// # Arguments
/// * `shmid` - Shared memory identifier
/// * `cmd` - Control command (IPC_STAT, IPC_SET, IPC_RMID)
/// * `buf` - Buffer for shared memory information
pub fn sys_shmctl(shmid: i32, cmd: u32, buf: UserPtr<ShmInfo>) -> LinuxResult<isize> {
    let segment = IPC_MANAGER.with_shm(|shm_manager| {
        shm_manager
            .get_segment_by_shmid(shmid)
            .ok_or(LinuxError::EINVAL)
    })?;

    let mut segment = segment.lock();
    with_uspace(|uspace| {
        match cmd {
            IPC_SET => {
                // Update segment information
                if !buf.is_null() {
                    segment.shmid_ds = uspace.read(buf)?;
                    segment.shmid_ds.update_change_time();
                }
            }
            IPC_STAT => {
                // Get segment information
                nullable!(uspace.write(buf, segment.shmid_ds))?;
            }
            IPC_RMID => {
                // Mark segment for removal
                segment.rmid = true;
                segment.shmid_ds.update_change_time();
            }
            _ => {
                return Err(LinuxError::EINVAL);
            }
        }

        Ok(0)
    })
}

/// Detach shared memory segment from the calling process
///
/// # Arguments
/// * `shmaddr` - Address of the shared memory segment to detach
pub fn sys_shmdt(shmaddr: usize) -> LinuxResult<isize> {
    let shmaddr = VirtAddr::from(shmaddr);
    let pid = with_process(|process| process.pid());

    // Find the shared memory ID for this address and perform detach operations
    let (shmid, should_remove) =
        IPC_MANAGER.with_shm(|shm_manager| -> LinuxResult<(i32, bool)> {
            let shmid = shm_manager
                .get_shmid_by_vaddr(pid, shmaddr)
                .ok_or(LinuxError::EINVAL)?;

            let segment = shm_manager
                .get_segment_by_shmid(shmid)
                .ok_or(LinuxError::EINVAL)?;

            let mut segment = segment.lock();

            // Get the virtual address range for validation
            let va_range = segment.get_addr_range(pid).ok_or(LinuxError::EINVAL)?;

            // Unmap from virtual address space
            with_uspace(|uspace| uspace.aspace.lock().unmap(va_range.start, va_range.size()))?;

            // Update bookkeeping
            shm_manager.remove_shmaddr(pid, shmaddr);
            segment
                .detach_process(pid)
                .map_err(|_| LinuxError::EINVAL)?;

            // Check if segment should be removed
            let should_remove = segment.should_remove();
            Ok((shmid, should_remove))
        })?;

    // Remove segment if needed (done outside the closure to avoid deadlock)
    if should_remove {
        IPC_MANAGER.with_shm(|shm_manager| {
            shm_manager.remove_shmid(shmid);
        });
    }

    Ok(0)
}
