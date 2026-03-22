//! Shared memory system calls implementation.
use alloc::sync::Arc;

use axerrno::{LinuxError, LinuxResult};
use axhal::paging::PageSize;
use axsync::Mutex;
use memory_addr::{PAGE_SIZE_4K, VirtAddr, VirtAddrRange};
use page_table_entry::MappingFlags;

use xcore::{
    ipc::{IPC_MANAGER, ShmInfo, ShmSegment},
    task::{with_process, with_uspace},
    with_ipc_manager,
};
use xprocess::Pid;
use xuspace::{UserPtr, UserSpaceAccess, nullable};
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

/// Cleanup shared memory for a specific process on exit
pub fn clear_proc_shm(pid: Pid) {
    IPC_MANAGER.with_shm(|shm_manager| {
        if let Some(shmids) = shm_manager.get_shmids_by_pid(pid) {
            for shmid in shmids {
                if let Some(segment) = shm_manager.get_segment_by_shmid(shmid) {
                    let mut segment = segment.lock();
                    if segment.detach_process(pid).is_ok() && segment.should_remove() {
                        // Segment will be cleaned up by cleanup_orphaned_segments
                    }
                }
            }
        }

        shm_manager.remove_pid(pid);
        shm_manager.cleanup_orphaned_segments();
    });
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
    aspace: &mut axmm::AddrSpace,
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

/// Map shared memory segment to virtual address space
fn map_shared_memory(
    aspace: &mut axmm::AddrSpace,
    segment: &mut ShmSegment,
    start_addr: VirtAddr,
    length: usize,
    mapping_flags: MappingFlags,
    cur_pid: Pid,
) -> LinuxResult<()> {
    if let Some(phys_pages) = segment.phys_pages.clone() {
        // Segment already has physical pages from another process
        aspace.map_shared(
            start_addr,
            length,
            mapping_flags,
            Some(phys_pages),
            PageSize::Size4K,
        )?;
    } else {
        // First process to attach - allocate physical pages
        match aspace.map_shared(start_addr, length, mapping_flags, None, PageSize::Size4K) {
            Ok(pages) => {
                info!(
                    "Process {} allocated shared memory: addr={:#x}, size={}",
                    cur_pid,
                    start_addr.as_usize(),
                    length
                );
                segment.map_to_phys(pages);
            }
            Err(e) => {
                error!(
                    "Failed to map shared memory for process {}: addr={:#x}, size={}, error={:?}",
                    cur_pid,
                    start_addr.as_usize(),
                    length,
                    e
                );
                return Err(LinuxError::ENOMEM);
            }
        }
    }

    Ok(())
}

/// Attach a shared memory segment to the calling process
///
/// # Arguments
/// * `shmid` - Shared memory identifier
/// * `addr` - Requested address (0 for automatic allocation)
/// * `shmflg` - Flags controlling attachment behavior
pub fn sys_shmat(shmid: i32, addr: usize, shmflg: u32) -> LinuxResult<isize> {
    let segment = IPC_MANAGER.with_shm(|shm_manager| {
        shm_manager
            .get_segment_by_shmid(shmid)
            .ok_or(LinuxError::EINVAL)
    })?;

    let mut segment = segment.lock();
    let mut mapping_flags = segment.mapping_flags;
    let shm_flg = ShmAtFlags::from_bits_truncate(shmflg);

    // Apply read-only flag
    if shm_flg.contains(ShmAtFlags::SHM_RDONLY) {
        mapping_flags.remove(MappingFlags::WRITE);
    }

    let cur_pid = with_process(|process| process.pid());

    with_uspace(|uspace| {
        let mut aspace = uspace.aspace.lock();
        let length = segment.page_num * PAGE_SIZE_4K;

        // Check if process is already attached
        if segment.is_attached(cur_pid) {
            return Err(LinuxError::EINVAL);
        }

        // Find virtual address for mapping
        let start_addr = find_mapping_address(&mut aspace, addr, length)?;
        let end_addr = VirtAddr::from(start_addr.as_usize() + length);
        let va_range = VirtAddrRange::new(start_addr, end_addr);

        // Register mapping before actual memory mapping
        IPC_MANAGER.with_shm(|shm_manager| {
            shm_manager.insert_shmid_vaddr(cur_pid, segment.shmid, start_addr);
        });

        info!(
            "Process {} attaching shared memory: shmid={}, addr={:#x}, size={}, flags={:#x?}",
            cur_pid,
            shmid,
            start_addr.as_usize(),
            length,
            mapping_flags
        );

        // Perform the actual memory mapping
        map_shared_memory(
            &mut aspace,
            &mut segment,
            start_addr,
            length,
            mapping_flags,
            cur_pid,
        )?;

        // Update segment metadata
        segment
            .attach_process(cur_pid, va_range)
            .map_err(|_| LinuxError::EINVAL)?;

        Ok(start_addr.as_usize() as isize)
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
    let should_remove_segment =
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
            axhal::arch::flush_tlb(None);

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
    if should_remove_segment.1 {
        IPC_MANAGER.with_shm(|shm_manager| {
            shm_manager.remove_shmid(should_remove_segment.0);
        });
    }

    Ok(0)
}
