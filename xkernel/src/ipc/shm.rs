//! Shared memory management implementation.
use alloc::{sync::Arc, vec::Vec};

use memory_addr::{PAGE_SIZE_4K, VirtAddr, VirtAddrRange, align_up_4k};
use page_table_entry::MappingFlags;
use xerrno::{LinuxError, LinuxResult};
use xsync::Mutex;
use xvma::SharedObject;

use xprocess::Pid;
use xutils::{
    collections::btreemap::{BTreeMap, BiBTreeMap},
    ctypes::{
        __kernel_mode_t, __kernel_pid_t, __kernel_size_t, __kernel_time_t, SHMALL, SHMMAX, SHMMIN,
        SHMMNI, c_ushort,
    },
    time::monotonic_time_nanos,
};

use super::{IPC_MANAGER, IpcManager, IpcPerm, IpcidGenerator};

/// Shared memory segment information structure
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ShmInfo {
    /// IPC permissions
    pub shm_perm: IpcPerm,
    /// Size of segment in bytes
    pub shm_segsz: __kernel_size_t,
    /// Last attach time
    pub shm_atime: __kernel_time_t,
    /// Last detach time
    pub shm_dtime: __kernel_time_t,
    /// Last change time
    pub shm_ctime: __kernel_time_t,
    /// PID of creator
    pub shm_cpid: __kernel_pid_t,
    /// PID of last shmat/shmdt
    pub shm_lpid: __kernel_pid_t,
    /// Number of current attaches
    pub shm_nattch: c_ushort,
}

impl ShmInfo {
    /// Create a new shared memory info structure
    pub fn new(key: i32, size: usize, mode: __kernel_mode_t, pid: __kernel_pid_t) -> Self {
        let current_time = monotonic_time_nanos() as __kernel_time_t;

        Self {
            shm_perm: IpcPerm::new(key, mode),
            shm_segsz: size as __kernel_size_t,
            shm_atime: 0,
            shm_dtime: 0,
            shm_ctime: current_time,
            shm_cpid: pid,
            shm_lpid: pid,
            shm_nattch: 0,
        }
    }

    /// Update timestamps for attach operation
    pub fn update_attach_time(&mut self, pid: __kernel_pid_t) {
        self.shm_atime = monotonic_time_nanos() as __kernel_time_t;
        self.shm_lpid = pid;
        self.shm_nattch += 1;
    }

    /// Update timestamps for detach operation
    pub fn update_detach_time(&mut self, pid: __kernel_pid_t) {
        self.shm_dtime = monotonic_time_nanos() as __kernel_time_t;
        self.shm_lpid = pid;
        self.shm_nattch = self.shm_nattch.saturating_sub(1);
    }

    /// Update timestamps for control operation
    pub fn update_change_time(&mut self) {
        self.shm_ctime = monotonic_time_nanos() as __kernel_time_t;
    }
}

/// Shared memory segment implementation
#[derive(Clone)]
pub struct ShmSegment {
    /// Shared memory identifier
    pub shmid: i32,
    /// Number of pages in this segment
    pub page_num: usize,
    /// Virtual address ranges mapped by each process
    pub va_range: BTreeMap<Pid, VirtAddrRange>,
    /// Shared VM object backing this segment.
    pub object: Option<Arc<SharedObject>>,
    /// Whether segment is marked for removal
    pub rmid: bool,
    /// Memory mapping flags
    pub mapping_flags: MappingFlags,
    /// Segment metadata
    pub shmid_ds: ShmInfo,
}

impl ShmSegment {
    /// Create a new shared memory segment
    pub fn new(key: i32, shmid: i32, size: usize, mapping_flags: MappingFlags, pid: Pid) -> Self {
        Self {
            shmid,
            page_num: align_up_4k(size) / PAGE_SIZE_4K,
            va_range: BTreeMap::new(),
            object: None,
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

    /// Try to update an existing segment with new parameters
    pub fn try_update(
        &mut self,
        size: usize,
        mapping_flags: MappingFlags,
        pid: Pid,
    ) -> LinuxResult<isize> {
        // Validate that size and permissions match
        if size as __kernel_size_t != self.shmid_ds.shm_segsz
            || mapping_flags.bits() as __kernel_mode_t != self.shmid_ds.shm_perm.mode
        {
            return Err(LinuxError::EINVAL);
        }

        self.shmid_ds.shm_lpid = pid as i32;
        Ok(self.shmid as isize)
    }

    /// Publishes the shared VM object backing this segment.
    pub fn set_object(&mut self, object: Arc<SharedObject>) {
        self.object = Some(object);
    }

    /// Get the number of processes currently attached
    pub fn attach_count(&self) -> usize {
        self.va_range.len()
    }

    /// Get the virtual address range for a specific process
    pub fn get_addr_range(&self, pid: Pid) -> Option<VirtAddrRange> {
        self.va_range.get(&pid).cloned()
    }

    /// Check if a process is attached to this segment
    pub fn is_attached(&self, pid: Pid) -> bool {
        self.va_range.contains_key(&pid)
    }

    /// Attach a process to this segment
    pub fn attach_process(&mut self, pid: Pid, va_range: VirtAddrRange) -> LinuxResult<()> {
        if self.is_attached(pid) {
            return Err(LinuxError::EINVAL);
        }

        self.va_range.insert(pid, va_range);
        self.shmid_ds.update_attach_time(pid as __kernel_pid_t);

        Ok(())
    }

    /// Detach a process from this segment
    pub fn detach_process(&mut self, pid: Pid) -> LinuxResult<()> {
        if !self.is_attached(pid) {
            return Err(LinuxError::EINVAL);
        }

        self.va_range.remove(&pid);
        self.shmid_ds.update_detach_time(pid as __kernel_pid_t);

        Ok(())
    }

    /// Check if this segment should be removed
    pub fn should_remove(&self) -> bool {
        self.rmid && self.attach_count() == 0
    }

    /// Get total memory usage in pages
    pub fn memory_usage(&self) -> usize {
        self.page_num
    }
}

/// Shared memory manager
pub struct ShmManager {
    /// Map from key to shared memory ID
    index: BTreeMap<i32, i32>,
    /// Map from shared memory ID to segment
    segments: BTreeMap<i32, Arc<Mutex<ShmSegment>>>,
    /// Map from process ID to (shmid -> virtual address) mappings
    pid_shmid_vaddr: BTreeMap<Pid, BiBTreeMap<i32, VirtAddr>>,
    /// ID generator for new segments
    id_generator: Mutex<IpcidGenerator>,
}

struct InheritedAttachment {
    shmid: i32,
    vaddr: VirtAddr,
    range: VirtAddrRange,
    segment: Arc<Mutex<ShmSegment>>,
}

impl ShmManager {
    /// Create a new shared memory manager
    pub const fn new() -> Self {
        Self {
            segments: BTreeMap::new(),
            index: BTreeMap::new(),
            pid_shmid_vaddr: BTreeMap::new(),
            id_generator: Mutex::new(IpcidGenerator::new()),
        }
    }

    /// Find shared memory ID by key
    pub fn get_shmid_by_key(&self, key: i32) -> Option<i32> {
        self.index.get(&key).cloned()
    }

    /// Get shared memory segment by ID
    pub fn get_segment_by_shmid(&self, shmid: i32) -> Option<Arc<Mutex<ShmSegment>>> {
        self.segments.get(&shmid).cloned()
    }

    /// Find shared memory ID by process and virtual address
    pub fn get_shmid_by_vaddr(&self, pid: Pid, vaddr: VirtAddr) -> Option<i32> {
        self.pid_shmid_vaddr
            .get(&pid)?
            .get_by_value(&vaddr)
            .cloned()
    }

    /// Find virtual address by process and shared memory ID
    pub fn find_vaddr_by_shmid(&self, pid: Pid, shmid: i32) -> Option<VirtAddr> {
        self.pid_shmid_vaddr.get(&pid)?.get_by_key(&shmid).cloned()
    }

    /// Register a new key-to-shmid mapping
    pub fn insert_key_shmid(&mut self, key: i32, shmid: i32) {
        self.index.insert(key, shmid);
    }

    /// Register a new shared memory segment
    pub fn insert_shmid_segment(&mut self, shmid: i32, segment: Arc<Mutex<ShmSegment>>) {
        self.segments.insert(shmid, segment);
    }

    /// Register a virtual address mapping for a process
    pub fn insert_shmid_vaddr(&mut self, pid: Pid, shmid: i32, vaddr: VirtAddr) {
        self.pid_shmid_vaddr
            .entry(pid)
            .or_insert_with(BiBTreeMap::new)
            .insert(shmid, vaddr);
    }

    /// Register the System V shared-memory attachments inherited by a child.
    pub fn inherit_process(&mut self, parent_pid: Pid, child_pid: Pid) -> LinuxResult<()> {
        if self.pid_shmid_vaddr.contains_key(&child_pid) {
            return Err(LinuxError::EINVAL);
        }

        let Some(parent_mappings) = self.pid_shmid_vaddr.get(&parent_pid) else {
            return Ok(());
        };
        let mut attachments = Vec::new();
        attachments
            .try_reserve_exact(parent_mappings.forward.len())
            .map_err(|_| LinuxError::ENOMEM)?;
        for (&shmid, &vaddr) in &parent_mappings.forward {
            let segment = self.get_segment_by_shmid(shmid).ok_or(LinuxError::EINVAL)?;
            let range = {
                let segment = segment.lock();
                if segment.is_attached(child_pid) {
                    return Err(LinuxError::EINVAL);
                }
                segment
                    .get_addr_range(parent_pid)
                    .ok_or(LinuxError::EINVAL)?
            };
            attachments.push(InheritedAttachment {
                shmid,
                vaddr,
                range,
                segment,
            });
        }

        for (attached, attachment) in attachments.iter().enumerate() {
            if let Err(error) = attachment
                .segment
                .lock()
                .attach_process(child_pid, attachment.range)
            {
                for attachment in attachments[..attached].iter().rev() {
                    let result = attachment.segment.lock().detach_process(child_pid);
                    debug_assert!(result.is_ok());
                }
                return Err(error);
            }
        }

        let mut child_mappings = BiBTreeMap::new();
        for attachment in attachments {
            child_mappings.insert(attachment.shmid, attachment.vaddr);
        }
        self.pid_shmid_vaddr.insert(child_pid, child_mappings);
        Ok(())
    }

    /// Remove a virtual address mapping
    pub fn remove_shmaddr(&mut self, pid: Pid, shmaddr: VirtAddr) {
        let remove_pid = if let Some(map) = self.pid_shmid_vaddr.get_mut(&pid) {
            map.remove_by_value(&shmaddr);
            map.forward.is_empty()
        } else {
            false
        };

        if remove_pid {
            self.pid_shmid_vaddr.remove(&pid);
        }
    }

    /// Detach every segment inherited or attached by a process.
    fn detach_process(&mut self, pid: Pid) {
        if let Some(mappings) = self.pid_shmid_vaddr.remove(&pid) {
            for shmid in mappings.forward.keys() {
                if let Some(segment) = self.get_segment_by_shmid(*shmid) {
                    let result = segment.lock().detach_process(pid);
                    debug_assert!(result.is_ok());
                }
            }
        }
        self.cleanup_orphaned_segments();
    }

    /// Remove a shared memory segment completely
    pub fn remove_shmid(&mut self, shmid: i32) -> bool {
        let key_removed = if let Some(segment) = self.segments.get(&shmid) {
            let segment = segment.lock();
            let key = segment.shmid_ds.shm_perm.key;
            self.index.remove(&key);
            true
        } else {
            false
        };

        self.segments.remove(&shmid);
        key_removed
    }

    /// Allocate a new shared memory ID
    pub fn allocate_shmid(&self) -> i32 {
        self.id_generator.lock().alloc()
    }

    /// Get the number of segments
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Calculate total pages used by all segments
    pub fn total_pages(&self) -> usize {
        self.segments
            .values()
            .map(|segment| segment.lock().memory_usage())
            .sum()
    }

    /// Cleanup orphaned segments marked for removal
    pub fn cleanup_orphaned_segments(&mut self) -> usize {
        let mut removed_count = 0;
        let shmids_to_remove: Vec<i32> = self
            .segments
            .iter()
            .filter_map(|(shmid, segment)| {
                if segment.lock().should_remove() {
                    Some(*shmid)
                } else {
                    None
                }
            })
            .collect();

        for shmid in shmids_to_remove {
            if self.remove_shmid(shmid) {
                removed_count += 1;
            }
        }

        removed_count
    }

    /// Validate segment parameters
    pub fn validate_segment_params(&self, size: usize, _flags: u32) -> LinuxResult<()> {
        if !(SHMMIN..=SHMMAX).contains(&size) {
            return Err(LinuxError::EINVAL);
        }

        let page_count = align_up_4k(size) / PAGE_SIZE_4K;
        if self.total_pages() + page_count > SHMALL {
            return Err(LinuxError::ENOSPC);
        }

        if self.segment_count() >= SHMMNI {
            return Err(LinuxError::ENOSPC);
        }

        Ok(())
    }

    /// Clear all shared memory resources
    pub fn clear(&mut self) {
        // Get all segment IDs to remove
        let all_shmids: Vec<i32> = self.segments.keys().cloned().collect();

        // Mark all segments for removal and remove them
        for shmid in all_shmids {
            if let Some(segment_arc) = self.segments.get(&shmid) {
                let mut segment = segment_arc.lock();
                segment.rmid = true;
                // Force detach all processes
                segment.va_range.clear();
                segment.shmid_ds.shm_nattch = 0;
            }
            self.remove_shmid(shmid);
        }

        // Clear all mappings
        self.index.clear();
        self.segments.clear();
        self.pid_shmid_vaddr.clear();
    }
}

/// Register shared-memory attachments inherited across process creation.
pub fn inherit_proc_shm(
    ipc_manager: &Mutex<IpcManager>,
    parent_pid: Pid,
    child_pid: Pid,
) -> LinuxResult<()> {
    ipc_manager
        .lock()
        .get_shm()
        .lock()
        .inherit_process(parent_pid, child_pid)
}

/// Detach all System V shared-memory segments owned by a process.
pub fn clear_proc_shm(pid: Pid) {
    IPC_MANAGER.with_shm(|manager| manager.detach_process(pid));
}
