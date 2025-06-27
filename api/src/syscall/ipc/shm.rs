use alloc::sync::Arc;
use axerrno::{LinuxError, LinuxResult};
use axprocess::Pid;
use axsync::Mutex;
use axtask::current;
use memory_addr::{PAGE_SIZE_4K, VirtAddr, VirtAddrRange};
use page_table_entry::MappingFlags;
use starry_core::task::TaskExt;

use crate::{
    ctypes::__kernel_time_t,
    ipc::{
        IPC_MANAGER, IPC_PRIVATE, IPC_RMID, IPC_SET, IPC_STAT, ShmAtFlags, ShmGetFlags, ShmInfo,
        ShmSegment,
    },
    ptr::{UserPtr, nullable},
    time::monotonic_time_nanos,
};

// called when a process exit, detach all the shmem related
pub fn clear_proc_shm(pid: Pid) {
    let ipc_manager = IPC_MANAGER.lock();
    let mut shm_manager = ipc_manager.get_shm().lock();
    if let Some(shmids) = shm_manager.get_shmids_by_pid(pid) {
        for shmid in shmids {
            let shm_inner = shm_manager.get_inner_by_shmid(shmid).unwrap();
            let mut shm_inner = shm_inner.lock();
            shm_inner.detach_process(pid);

            if shm_inner.rmid && shm_inner.attach_count() == 0 {
                shm_manager.remove_shmid(shmid);
            }
        }
    }
    shm_manager.remove_pid(pid);
}

pub fn sys_shmget(key: i32, size: usize, shmflg: usize) -> LinuxResult<isize> {
    let page_num = memory_addr::align_up_4k(size) / PAGE_SIZE_4K;
    if page_num == 0 {
        return Err(LinuxError::EINVAL);
    }

    let mut mapping_flags = MappingFlags::from_name("USER").unwrap();
    if (shmflg as u32) & ShmGetFlags::SHM_R.bits() != 0 {
        mapping_flags.insert(MappingFlags::READ);
    }
    if (shmflg as u32) & ShmGetFlags::SHM_W.bits() != 0 {
        mapping_flags.insert(MappingFlags::WRITE);
    }

    let cur_pid = TaskExt::from_task(&current()).thread.process().pid();
    let ipc_manager = IPC_MANAGER.lock();
    let mut shm_manager = ipc_manager.get_shm().lock();

    if key != IPC_PRIVATE {
        // This process has already created a shared memory segment with the same key
        if let Some(shmid) = shm_manager.get_shmid_by_key(key) {
            let shm_inner = shm_manager
                .get_inner_by_shmid(shmid)
                .ok_or(LinuxError::EINVAL)?;
            let mut shm_inner = shm_inner.lock();
            return shm_inner.try_update(size, mapping_flags, cur_pid);
        }
    }

    // Create a new shm_inner
    let shmid = shm_manager.allocate_shmid();
    let shm_inner = Arc::new(Mutex::new(ShmSegment::new(
        key,
        shmid,
        size,
        mapping_flags,
        cur_pid,
    )));
    shm_manager.insert_key_shmid(key, shmid);
    shm_manager.insert_shmid_inner(shmid, shm_inner);

    Ok(shmid as isize)
}

pub fn sys_shmat(shmid: i32, addr: usize, shmflg: u32) -> LinuxResult<isize> {
    let shm_inner = {
        let ipc_manager = IPC_MANAGER.lock();
        let shm_manager = ipc_manager.get_shm().lock();
        shm_manager.get_inner_by_shmid(shmid).unwrap()
    };
    let mut shm_inner = shm_inner.lock();
    let mut mapping_flags = shm_inner.mapping_flags;
    let shm_flg = ShmAtFlags::from_bits_truncate(shmflg);

    if shm_flg.contains(ShmAtFlags::SHM_RDONLY) {
        mapping_flags.remove(MappingFlags::WRITE);
    }

    // TODO: solve shmflg: SHM_RND and SHM_REMAP

    let cur_pid = TaskExt::from_task(&current()).thread.process().pid();
    let process_data = TaskExt::from_task(&current()).process_data();
    let mut aspace = process_data.aspace.lock();

    let start_aligned = memory_addr::align_down_4k(addr);
    let length = shm_inner.page_num * PAGE_SIZE_4K;

    // alloc the virtual address range
    assert!(shm_inner.get_addr_range(cur_pid).is_none());
    let start_addr = aspace
        .find_free_area(
            VirtAddr::from(start_aligned),
            length,
            VirtAddrRange::new(aspace.base(), aspace.end()),
        )
        .or_else(|| {
            aspace.find_free_area(
                aspace.base(),
                length,
                VirtAddrRange::new(aspace.base(), aspace.end()),
            )
        })
        .ok_or(LinuxError::ENOMEM)?;
    let end_addr = VirtAddr::from(start_addr.as_usize() + length);
    let va_range = VirtAddrRange::new(start_addr, end_addr);

    let ipc_manager = IPC_MANAGER.lock();
    let mut shm_manager = ipc_manager.get_shm().lock();
    shm_manager.insert_shmid_vaddr(cur_pid, shm_inner.shmid, start_addr);
    info!(
        "Process {} alloc shm virt addr start: {:#x}, size: {}, mapping_flags: {:#x?}",
        cur_pid,
        start_addr.as_usize(),
        length,
        mapping_flags
    );

    // map the virtual address range to the physical address
    if let Some(phys_pages) = shm_inner.phys_pages.clone() {
        // Another proccess has attached the shared memory
        aspace.map_shared(start_addr, length, mapping_flags, Some(phys_pages))?;
    } else {
        // This is the first process to attach the shared memory
        let result = aspace.map_shared(start_addr, length, mapping_flags, None);

        match result {
            Ok(pages) => {
                info!(
                    "proc {} map shm addr: {:#x}, size: {}",
                    cur_pid,
                    start_addr.as_usize(),
                    length
                );
                shm_inner.map_to_phys(pages);
            }
            Err(e) => {
                error!(
                    "proc {} map shm addr: {:#x}, size: {}, error: {:?}",
                    cur_pid,
                    start_addr.as_usize(),
                    length,
                    e
                );
                return Err(LinuxError::ENOMEM);
            }
        }
    }

    shm_inner.attach_process(cur_pid, va_range);
    Ok(start_addr.as_usize() as isize)
}

pub fn sys_shmctl(shmid: i32, cmd: u32, buf: UserPtr<ShmInfo>) -> LinuxResult<isize> {
    let shm_inner = {
        let ipc_manager = IPC_MANAGER.lock();
        let shm_manager = ipc_manager.get_shm().lock();
        shm_manager
            .get_inner_by_shmid(shmid)
            .ok_or(LinuxError::EINVAL)?
    };
    let mut shm_inner = shm_inner.lock();

    if cmd == IPC_SET {
        shm_inner.shmid_ds = *buf.get_as_mut()?;
    } else if cmd == IPC_STAT {
        if let Some(shmid_ds) = nullable!(buf.get_as_mut())? {
            *shmid_ds = shm_inner.shmid_ds;
        }
    } else if cmd == IPC_RMID {
        shm_inner.rmid = true;
    } else {
        return Err(LinuxError::EINVAL);
    }

    shm_inner.shmid_ds.shm_ctime = monotonic_time_nanos() as __kernel_time_t;
    Ok(0)
}

pub fn sys_shmdt(shmaddr: usize) -> LinuxResult<isize> {
    let shmaddr = VirtAddr::from(shmaddr);
    let pid = TaskExt::from_task(&current()).thread.process().pid();
    let shmid = {
        let ipc_manager = IPC_MANAGER.lock();
        let shm_manager = ipc_manager.get_shm().lock();
        shm_manager
            .get_shmid_by_vaddr(pid, shmaddr)
            .ok_or(LinuxError::EINVAL)?
    };

    let shm_inner = {
        let ipc_manager = IPC_MANAGER.lock();
        let shm_manager = ipc_manager.get_shm().lock();
        shm_manager
            .get_inner_by_shmid(shmid)
            .ok_or(LinuxError::EINVAL)?
    };
    let mut shm_inner = shm_inner.lock();
    let va_range = shm_inner.get_addr_range(pid).ok_or(LinuxError::EINVAL)?;

    let mut aspace = TaskExt::from_task(&current()).process_data().aspace.lock();
    aspace.unmap(va_range.start, va_range.size())?;
    axhal::arch::flush_tlb(None);

    let ipc_manager = IPC_MANAGER.lock();
    let mut shm_manager = ipc_manager.get_shm().lock();
    shm_manager.remove_shmaddr(pid, shmaddr);
    shm_inner.detach_process(pid);

    if shm_inner.rmid && shm_inner.attach_count() == 0 {
        shm_manager.remove_shmid(shmid);
    }

    Ok(0)
}
