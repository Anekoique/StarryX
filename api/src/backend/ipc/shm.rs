use alloc::{sync::Arc, vec::Vec};
use axerrno::{LinuxError, LinuxResult};
use axmm::SharedPages;
use axprocess::Pid;
use axsync::Mutex;
use bitflags::bitflags;
use memory_addr::{PAGE_SIZE_4K, VirtAddr, VirtAddrRange};
use page_table_entry::MappingFlags;

use super::{IpcPerm, IpcidGenerator};
use crate::{
    collections::{BTreeMap, BiBTreeMap},
    ctypes::{__kernel_mode_t, __kernel_pid_t, __kernel_size_t, __kernel_time_t, c_ushort},
    time::monotonic_time_nanos,
};

pub const SHMMIN: usize = 1;
pub const SHMMNI: usize = 4096;
pub const SHMMAX: usize = usize::MAX - (1 << 24);
pub const SHMALL: usize = usize::MAX - (1 << 24);
pub const SHMSEG: usize = SHMMNI;

bitflags! {
    pub struct ShmGetFlags: u32 {
        const SHM_R = 0o400;
        const SHM_W = 0o200;
    }
}

bitflags! {
    pub struct ShmAtFlags: u32 {
        const SHM_RDONLY = 0o10000;
        const SHM_RND = 0o20000;
        const SHM_REMAP = 0o40000;
        const SHM_EXEC = 0o100000;
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ShmInfo {
    pub shm_perm: IpcPerm,
    pub shm_segsz: __kernel_size_t,
    pub shm_atime: __kernel_time_t,
    pub shm_dtime: __kernel_time_t,
    pub shm_ctime: __kernel_time_t,
    pub shm_cpid: __kernel_pid_t,
    pub shm_lpid: __kernel_pid_t,
    pub shm_nattch: c_ushort,
}

impl ShmInfo {
    pub fn new(key: i32, size: usize, mode: __kernel_mode_t, pid: __kernel_pid_t) -> Self {
        Self {
            shm_perm: IpcPerm {
                key,
                uid: 0,
                gid: 0,
                cuid: 0,
                cgid: 0,
                mode,
                seq: 0,
                pad: 0,
                unused0: 0,
                unused1: 0,
            },
            shm_segsz: size as __kernel_size_t,
            shm_atime: 0,
            shm_dtime: 0,
            shm_ctime: 0,
            shm_cpid: pid,
            shm_lpid: pid,
            shm_nattch: 0,
        }
    }
}

#[derive(Clone)]
pub struct ShmSegment {
    pub shmid: i32,
    pub page_num: usize,
    pub va_range: BTreeMap<Pid, VirtAddrRange>,
    pub phys_pages: Option<Arc<SharedPages>>,
    pub rmid: bool,
    pub mapping_flags: MappingFlags,
    pub shmid_ds: ShmInfo,
}

impl ShmSegment {
    pub fn new(key: i32, shmid: i32, size: usize, mapping_flags: MappingFlags, pid: Pid) -> Self {
        ShmSegment {
            shmid,
            page_num: memory_addr::align_up_4k(size) / PAGE_SIZE_4K,
            va_range: BTreeMap::new(),
            phys_pages: None,
            rmid: false,
            mapping_flags,
            shmid_ds: ShmInfo::new(
                key,
                size,
                mapping_flags.bits() as __kernel_mode_t,
                pid as __kernel_pid_t,
            ),
        }
    }

    pub fn try_update(
        &mut self,
        size: usize,
        mapping_flags: MappingFlags,
        pid: Pid,
    ) -> LinuxResult<isize> {
        if size as __kernel_size_t != self.shmid_ds.shm_segsz
            || mapping_flags.bits() as __kernel_mode_t != self.shmid_ds.shm_perm.mode
        {
            return Err(LinuxError::EINVAL);
        }
        self.shmid_ds.shm_lpid = pid as i32;
        Ok(self.shmid as isize)
    }

    pub fn map_to_phys(&mut self, phys_pages: Arc<SharedPages>) {
        self.phys_pages = Some(phys_pages);
    }

    pub fn attach_count(&self) -> usize {
        self.va_range.len()
    }

    pub fn get_addr_range(&self, pid: Pid) -> Option<VirtAddrRange> {
        self.va_range.get(&pid).cloned()
    }

    // called by sys_shmat
    pub fn attach_process(&mut self, pid: Pid, va_range: VirtAddrRange) {
        assert!(self.get_addr_range(pid).is_none());
        self.va_range.insert(pid, va_range);
        self.shmid_ds.shm_nattch += 1;
        self.shmid_ds.shm_lpid = pid as __kernel_pid_t;
        self.shmid_ds.shm_atime = monotonic_time_nanos() as __kernel_time_t;
    }

    // called by sys_shmdt
    pub fn detach_process(&mut self, pid: Pid) {
        assert!(self.get_addr_range(pid).is_some());
        self.va_range.remove(&pid);
        self.shmid_ds.shm_nattch -= 1;
        self.shmid_ds.shm_lpid = pid as __kernel_pid_t;
        self.shmid_ds.shm_dtime = monotonic_time_nanos() as __kernel_time_t;
    }
}

pub struct ShmManager {
    index: BTreeMap<i32, i32>,
    segments: BTreeMap<i32, Arc<Mutex<ShmSegment>>>,
    pid_shmid_vaddr: BTreeMap<Pid, BiBTreeMap<i32, VirtAddr>>,
    id_generator: Mutex<IpcidGenerator>,
}

impl ShmManager {
    pub const fn new() -> Self {
        ShmManager {
            segments: BTreeMap::new(),
            index: BTreeMap::new(),
            pid_shmid_vaddr: BTreeMap::new(),
            id_generator: Mutex::new(IpcidGenerator::new()),
        }
    }

    // used by sys_shmget
    pub fn get_shmid_by_key(&self, key: i32) -> Option<i32> {
        self.index.get(&key).cloned()
    }

    // the only way to find shm_inner -- the data structure to maintain shm
    pub fn get_inner_by_shmid(&self, shmid: i32) -> Option<Arc<Mutex<ShmSegment>>> {
        self.segments.get(&shmid).cloned()
    }

    // used by sys_shmdt
    pub fn get_shmid_by_vaddr(&self, pid: Pid, vaddr: VirtAddr) -> Option<i32> {
        self.pid_shmid_vaddr
            .get(&pid)
            .and_then(|map| map.get_by_value(&vaddr))
            .cloned()
    }

    pub fn get_shmids_by_pid(&self, pid: Pid) -> Option<Vec<i32>> {
        let map = self.pid_shmid_vaddr.get(&pid)?;
        let mut res = Vec::new();
        for key in map.forward.keys() {
            res.push(*key);
        }
        Some(res)
    }

    // used by garbage collection
    #[allow(dead_code)]
    pub fn find_vaddr_by_shmid(&self, pid: Pid, shmid: i32) -> Option<VirtAddr> {
        self.pid_shmid_vaddr
            .get(&pid)
            .and_then(|map| map.get_by_key(&shmid))
            .cloned()
    }

    // used by sys_shmget
    pub fn insert_key_shmid(&mut self, key: i32, shmid: i32) {
        self.index.insert(key, shmid);
    }

    // used by sys_shmat
    pub fn insert_shmid_inner(&mut self, shmid: i32, segment: Arc<Mutex<ShmSegment>>) {
        self.segments.insert(shmid, segment);
    }

    // used by sys_shmat, aiming at garbage collection when called sys_shmdt
    pub fn insert_shmid_vaddr(&mut self, pid: Pid, shmid: i32, vaddr: VirtAddr) {
        // maintain the map 'shmid_vaddr'
        self.pid_shmid_vaddr
            .entry(pid)
            .or_insert_with(BiBTreeMap::new)
            .insert(shmid, vaddr);
    }

    /*
     * Garbage collection for shared memory:
     * 1. when the process call sys_shmdt, delete everything related to shmaddr,
     *   including map 'shmid_vaddr';
     * 2. when the last process detach the shared memory and this shared memory
     *   was specified with IPC_RMID, delete everything related to this shared memory,
     *   including all the 3 maps;
     * 3. when a process exit, delete everything related to this process, including 2
     *   maps: 'shmid_vaddr' and 'shmid_inner';
     *
     *
     * The attach between the process and the shared memory occurs in sys_shmat,
     *  and the detach occurs in sys_shmdt, or when the process exits.
     */

    /*
     * Note: all the below delete functions only delete the mapping between the shm_id and the shm_inner,
     *   but the shm_inner is not deleted or modifyed!
     */

    // called by shmdt
    pub fn remove_shmaddr(&mut self, pid: Pid, shmaddr: VirtAddr) {
        let mut empty: bool = false;
        if let Some(map) = self.pid_shmid_vaddr.get_mut(&pid) {
            map.remove_by_value(&shmaddr);
            empty = map.forward.is_empty();
        }
        if empty {
            self.pid_shmid_vaddr.remove(&pid);
        }
    }

    // called when a process exit
    pub fn remove_pid(&mut self, pid: Pid) {
        self.pid_shmid_vaddr.remove(&pid);
    }

    pub fn remove_shmid(&mut self, shmid: i32) {
        self.index.remove(&shmid);
        self.segments.remove(&shmid);
        // for map in self.pid_shmid_vaddr.values() {
        // assert!(map.get_by_key(&shmid).is_none());
        // }
    }

    pub fn allocate_shmid(&self) -> i32 {
        self.id_generator.lock().alloc()
    }
}

impl Clone for ShmManager {
    fn clone(&self) -> Self {
        ShmManager {
            segments: self.segments.clone(),
            index: self.index.clone(),
            pid_shmid_vaddr: self.pid_shmid_vaddr.clone(),
            id_generator: Mutex::new(self.id_generator.lock().clone()),
        }
    }
}
