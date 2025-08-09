use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use core::cmp::Ordering;

use axerrno::{LinuxError, LinuxResult};
use axsync::Mutex;
use axtask::WaitQueue;

use xprocess::Pid;
use xutils::{
    collections::btreemap::BTreeMap,
    ctypes::{
        __kernel_mode_t, __kernel_pid_t, __kernel_time_t, SEMMNI, SEMMNS, SEMVMX, c_long, c_ushort,
    },
    time::monotonic_time_nanos,
};

use super::{IpcPerm, IpcidGenerator};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SemBuf {
    pub sem_num: c_ushort, // semaphore index in array
    pub sem_op: i16,       // semaphore operation
    pub sem_flg: c_ushort, // operation flags
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SemInfo {
    pub sem_perm: IpcPerm,
    pub sem_otime: __kernel_time_t, // last semop time
    pub sem_ctime: __kernel_time_t, // last change time
    pub sem_nsems: c_ushort,        // number of semaphores in set
    pub pad: c_ushort,
    pub unused0: c_long,
    pub unused1: c_long,
}

impl SemInfo {
    pub fn new(key: i32, nsems: usize, mode: __kernel_mode_t, _pid: __kernel_pid_t) -> Self {
        Self {
            sem_perm: IpcPerm {
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
            sem_otime: 0,
            sem_ctime: monotonic_time_nanos() as __kernel_time_t,
            sem_nsems: nsems as c_ushort,
            pad: 0,
            unused0: 0,
            unused1: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Semaphore {
    pub semval: i16,  // current value
    pub sempid: Pid,  // pid of last operation
    pub semncnt: u16, // number of processes waiting for semval to increase
    pub semzcnt: u16, // number of processes waiting for semval to become zero
}

impl Semaphore {
    pub fn new() -> Self {
        Self {
            semval: 0,
            sempid: 0,
            semncnt: 0,
            semzcnt: 0,
        }
    }
}

#[derive(Clone)]
pub struct SemUndo {
    pub semid: i32,
    pub sem_num: usize,
    pub sem_op: i16,
}

#[derive(Clone)]
pub struct WaitingProcess {
    pub pid: Pid,
    pub operations: Vec<SemBuf>,
    pub error: Option<LinuxError>,
}

#[derive(Clone)]
pub struct SemSet {
    pub semid: i32,
    pub semaphores: Vec<Semaphore>,
    pub sem_info: SemInfo,
    pub rmid: bool,
    pub waiting_queue: Arc<Mutex<VecDeque<WaitingProcess>>>,
    pub wait_queue: Arc<WaitQueue>,
}

impl SemSet {
    pub fn new(key: i32, semid: i32, nsems: usize, mode: __kernel_mode_t, pid: Pid) -> Self {
        let mut semaphores = Vec::with_capacity(nsems);
        for _ in 0..nsems {
            semaphores.push(Semaphore::new());
        }

        Self {
            semid,
            semaphores,
            sem_info: SemInfo::new(key, nsems, mode, pid as __kernel_pid_t),
            rmid: false,
            waiting_queue: Arc::new(Mutex::new(VecDeque::new())),
            wait_queue: Arc::new(WaitQueue::new()),
        }
    }

    pub fn can_perform_operations(&self, operations: &[SemBuf]) -> bool {
        let mut temp_values = Vec::new();
        for sem in &self.semaphores {
            temp_values.push(sem.semval);
        }

        for op in operations {
            let sem_num = op.sem_num as usize;
            if sem_num >= temp_values.len() {
                return false;
            }

            let new_val = temp_values[sem_num] as i32 + op.sem_op as i32;
            if new_val < 0 || new_val > SEMVMX as i32 {
                return false;
            }
            temp_values[sem_num] = new_val as i16;
        }

        // Check for zero wait conditions
        for op in operations {
            if op.sem_op == 0 && temp_values[op.sem_num as usize] != 0 {
                return false;
            }
        }

        true
    }

    pub fn perform_operations(&mut self, operations: &[SemBuf], pid: Pid) -> LinuxResult<()> {
        if !self.can_perform_operations(operations) {
            return Err(LinuxError::EAGAIN);
        }

        // Update semaphore counts before performing operations
        for op in operations {
            let sem_num = op.sem_num as usize;
            match op.sem_op.cmp(&0) {
                Ordering::Less => self.semaphores[sem_num].semncnt -= 1,
                Ordering::Equal => self.semaphores[sem_num].semzcnt -= 1,
                Ordering::Greater => (),
            }
        }

        // Perform operations
        for op in operations {
            let sem_num = op.sem_num as usize;
            self.semaphores[sem_num].semval += op.sem_op;
            self.semaphores[sem_num].sempid = pid;
        }

        self.sem_info.sem_otime = monotonic_time_nanos() as __kernel_time_t;
        Ok(())
    }

    pub fn add_waiting_process(&mut self, pid: Pid, operations: Vec<SemBuf>) {
        let waiting_proc = WaitingProcess {
            pid,
            operations,
            error: None,
        };
        self.waiting_queue.lock().push_back(waiting_proc.clone());

        // Update waiting counts
        for op in &waiting_proc.operations {
            let sem_num = op.sem_num as usize;
            if sem_num < self.semaphores.len() {
                match op.sem_op.cmp(&0) {
                    Ordering::Less => self.semaphores[sem_num].semncnt += 1,
                    Ordering::Equal => self.semaphores[sem_num].semzcnt += 1,
                    Ordering::Greater => (),
                }
            }
        }
    }

    pub fn wake_up_processes(&mut self) {
        let mut remaining = VecDeque::new();

        while let Some(waiting_proc) = {
            let mut queue = self.waiting_queue.lock();
            queue.pop_front()
        } {
            if self.can_perform_operations(&waiting_proc.operations) {
                if self
                    .perform_operations(&waiting_proc.operations, waiting_proc.pid)
                    .is_err()
                {
                    remaining.push_back(waiting_proc);
                } else {
                    self.wait_queue.notify_one(true);
                }
            } else {
                remaining.push_back(waiting_proc);
            }
        }

        let mut queue = self.waiting_queue.lock();
        *queue = remaining;
    }
}

pub struct SemManager {
    index: BTreeMap<i32, i32>,                  // key -> semid
    semsets: BTreeMap<i32, Arc<Mutex<SemSet>>>, // semid -> SemSet
    pid_undo: BTreeMap<Pid, Vec<SemUndo>>,      // process undo operations
    id_generator: Mutex<IpcidGenerator>,
    total_semaphores: usize,
}

impl SemManager {
    pub const fn new() -> Self {
        SemManager {
            index: BTreeMap::new(),
            semsets: BTreeMap::new(),
            pid_undo: BTreeMap::new(),
            id_generator: Mutex::new(IpcidGenerator::new()),
            total_semaphores: 0,
        }
    }

    pub fn get_semid_by_key(&self, key: i32) -> Option<i32> {
        self.index.get(&key).cloned()
    }

    pub fn get_semset_by_id(&self, semid: i32) -> Option<Arc<Mutex<SemSet>>> {
        self.semsets.get(&semid).cloned()
    }

    pub fn create_semset(
        &mut self,
        key: i32,
        nsems: usize,
        mode: __kernel_mode_t,
        pid: Pid,
    ) -> LinuxResult<i32> {
        if self.total_semaphores + nsems > SEMMNS {
            return Err(LinuxError::ENOSPC);
        }

        if self.semsets.len() >= SEMMNI {
            return Err(LinuxError::ENOSPC);
        }

        let semid = self.id_generator.lock().alloc();
        let semset = Arc::new(Mutex::new(SemSet::new(key, semid, nsems, mode, pid)));

        self.index.insert(key, semid);
        self.semsets.insert(semid, semset);
        self.total_semaphores += nsems;

        Ok(semid)
    }

    pub fn remove_semset(&mut self, semid: i32) {
        if let Some(semset_arc) = self.semsets.remove(&semid) {
            let semset = semset_arc.lock();
            self.total_semaphores -= semset.semaphores.len();
            let key = semset.sem_info.sem_perm.key;
            drop(semset);
            self.index.remove(&key);
        }
    }

    pub fn add_undo_operation(&mut self, pid: Pid, semid: i32, sem_num: usize, sem_op: i16) {
        let undo_op = SemUndo {
            semid,
            sem_num,
            sem_op: -sem_op, // Reverse operation for undo
        };

        self.pid_undo.entry(pid).or_default().push(undo_op);
    }

    pub fn perform_undo_operations(&mut self, pid: Pid) {
        if let Some(undo_ops) = self.pid_undo.remove(&pid) {
            for undo_op in undo_ops {
                if let Some(semset_arc) = self.get_semset_by_id(undo_op.semid) {
                    let mut semset = semset_arc.lock();
                    if undo_op.sem_num < semset.semaphores.len() {
                        let new_val = semset.semaphores[undo_op.sem_num].semval as i32
                            + undo_op.sem_op as i32;
                        if new_val >= 0 && new_val <= SEMVMX as i32 {
                            semset.semaphores[undo_op.sem_num].semval = new_val as i16;
                            semset.semaphores[undo_op.sem_num].sempid = pid;
                            semset.wake_up_processes();
                        }
                    }
                }
            }
        }
    }

    pub fn allocate_semid(&self) -> i32 {
        self.id_generator.lock().alloc()
    }

    pub fn semset_count(&self) -> usize {
        self.semsets.len()
    }

    pub fn array_count(&self) -> usize {
        self.semsets.len()
    }

    pub fn total_semaphores(&self) -> usize {
        self.total_semaphores
    }

    /// Clear all semaphore resources
    pub fn clear(&mut self) {
        // Get all semaphore set IDs to remove
        let all_semids: Vec<i32> = self.semsets.keys().cloned().collect();

        // Mark all semaphore sets for removal and clear them
        for semid in all_semids {
            if let Some(semset_arc) = self.semsets.get(&semid) {
                let mut semset = semset_arc.lock();
                semset.rmid = true;

                // Clear all waiting processes with error
                let mut waiting_queue = semset.waiting_queue.lock();
                while let Some(mut waiting) = waiting_queue.pop_front() {
                    waiting.error = Some(LinuxError::EIDRM);
                }
                drop(waiting_queue);

                // Wake up all waiting processes
                semset.wait_queue.notify_all(true);

                // Reset semaphore values
                for sem in &mut semset.semaphores {
                    sem.semval = 0;
                    sem.sempid = 0;
                    sem.semncnt = 0;
                    sem.semzcnt = 0;
                }
            }
            self.remove_semset(semid);
        }

        // Clear all mappings and undo operations
        self.index.clear();
        self.semsets.clear();
        self.pid_undo.clear();
        self.total_semaphores = 0;
    }
}

impl Clone for SemManager {
    fn clone(&self) -> Self {
        SemManager {
            index: self.index.clone(),
            semsets: self.semsets.clone(),
            pid_undo: self.pid_undo.clone(),
            id_generator: Mutex::new(self.id_generator.lock().clone()),
            total_semaphores: self.total_semaphores,
        }
    }
}
