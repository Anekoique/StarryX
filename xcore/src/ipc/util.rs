//! IPC utilities and common structures.

use core::sync::atomic::{AtomicI32, Ordering};

use axns::{ResArc, def_resource};
use axsync::Mutex;

use xutils::ctypes::{
    __kernel_gid_t, __kernel_key_t, __kernel_mode_t, __kernel_uid_t, MSGMAX, MSGMNB, MSGMNI,
    SEMMNI, SEMMNS, SEMMSL, SEMOPM, SEMVMX, SHMALL, SHMMAX, SHMMNI, c_long, c_ushort,
};

use super::{msg::MsgManager, sem::SemManager, shm::ShmManager};

/// IPC permission structure as defined by POSIX
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IpcPerm {
    /// Key supplied to shmget(2)
    pub key: __kernel_key_t,
    /// Effective UID of owner
    pub uid: __kernel_uid_t,
    /// Effective GID of owner
    pub gid: __kernel_gid_t,
    /// Effective UID of creator
    pub cuid: __kernel_uid_t,
    /// Effective GID of creator
    pub cgid: __kernel_gid_t,
    /// Permissions + SHM_DEST and SHM_LOCKED flags
    pub mode: __kernel_mode_t,
    /// Sequence number
    pub seq: c_ushort,
    /// Padding for memory alignment
    pub pad: c_ushort,
    /// Reserved for future use
    pub unused0: c_long,
    /// Reserved for future use
    pub unused1: c_long,
}

impl IpcPerm {
    /// Create a new IPC permission structure
    pub fn new(key: i32, mode: __kernel_mode_t) -> Self {
        Self {
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
        }
    }
}

/// IPC ID generator for unique resource identification
#[derive(Debug)]
pub struct IpcidGenerator {
    next_ipcid: AtomicI32,
}

impl Clone for IpcidGenerator {
    fn clone(&self) -> Self {
        Self {
            next_ipcid: AtomicI32::new(self.next_ipcid.load(Ordering::SeqCst)),
        }
    }
}

impl IpcidGenerator {
    /// Create a new IPC ID generator
    pub const fn new() -> Self {
        Self {
            next_ipcid: AtomicI32::new(0),
        }
    }

    /// Allocate a new IPC ID
    pub fn alloc(&self) -> i32 {
        self.next_ipcid.fetch_add(1, Ordering::SeqCst)
    }

    /// Get the current ID without incrementing
    pub fn current(&self) -> i32 {
        self.next_ipcid.load(Ordering::SeqCst)
    }
}

/// System-wide IPC resource limits
#[derive(Clone, Copy, Debug)]
pub struct IpcLimits {
    // Shared Memory limits
    /// Maximum size in bytes for a shared memory segment
    pub shmmax: usize,
    /// Maximum number of shared memory identifiers
    pub shmmni: usize,
    /// Maximum number of shared memory pages system-wide
    pub shmall: usize,

    // Message Queue limits
    /// Maximum size in bytes for a message
    pub msgmax: usize,
    /// Default maximum size in bytes for a message queue
    pub msgmnb: usize,
    /// Maximum number of message queue identifiers
    pub msgmni: usize,

    // Semaphore limits
    /// Maximum number of semaphores per semaphore set
    pub semmsl: usize,
    /// Maximum number of semaphores system-wide
    pub semmns: usize,
    /// Maximum number of operations per semop call
    pub semopm: usize,
    /// Maximum number of semaphore identifiers
    pub semmni: usize,
    /// Maximum value for a semaphore
    pub semvmx: usize,
}

impl Default for IpcLimits {
    fn default() -> Self {
        Self {
            // Shared Memory
            shmmax: SHMMAX,
            shmmni: SHMMNI,
            shmall: SHMALL,

            // Message Queue
            msgmax: MSGMAX,
            msgmnb: MSGMNB,
            msgmni: MSGMNI,

            // Semaphore
            semmsl: SEMMSL,
            semmns: SEMMNS,
            semopm: SEMOPM,
            semmni: SEMMNI,
            semvmx: SEMVMX,
        }
    }
}

/// Central IPC manager coordinating all IPC resources
pub struct IpcManager {
    shm: Mutex<ShmManager>,
    msg: Mutex<MsgManager>,
    sem: Mutex<SemManager>,
    limits: IpcLimits,
}

impl Clone for IpcManager {
    fn clone(&self) -> Self {
        Self {
            shm: Mutex::new(self.shm.lock().clone()),
            msg: Mutex::new(self.msg.lock().clone()),
            sem: Mutex::new(self.sem.lock().clone()),
            limits: self.limits,
        }
    }
}

impl IpcManager {
    /// Create a new IPC manager with default limits
    pub fn new() -> Self {
        Self {
            shm: Mutex::new(ShmManager::new()),
            msg: Mutex::new(MsgManager::new()),
            sem: Mutex::new(SemManager::new()),
            limits: IpcLimits::default(),
        }
    }

    /// Get reference to shared memory manager
    pub fn get_shm(&self) -> &Mutex<ShmManager> {
        &self.shm
    }

    /// Get reference to message queue manager
    pub fn get_msg(&self) -> &Mutex<MsgManager> {
        &self.msg
    }

    /// Get reference to semaphore manager
    pub fn get_sem(&self) -> &Mutex<SemManager> {
        &self.sem
    }

    /// Get current IPC limits
    pub fn get_limits(&self) -> &IpcLimits {
        &self.limits
    }

    /// Update IPC limits
    pub fn set_limits(&mut self, limits: IpcLimits) {
        self.limits = limits;
    }

    /// Get comprehensive statistics for all IPC resources
    pub fn get_ipc_stats(&self) -> IpcStats {
        let shm_manager = self.shm.lock();
        let msg_manager = self.msg.lock();
        let sem_manager = self.sem.lock();

        IpcStats {
            shm_segments: shm_manager.segment_count(),
            msg_queues: msg_manager.queue_count(),
            sem_arrays: sem_manager.array_count(),
            total_shm_pages: shm_manager.total_pages(),
            total_msg_bytes: msg_manager.total_queues_bytes(),
            total_messages: msg_manager.total_messages(),
            total_semaphores: sem_manager.total_semaphores(),
        }
    }

    /// Check if resource limits are exceeded
    pub fn check_limits(&self) -> bool {
        let stats = self.get_ipc_stats();

        stats.shm_segments <= self.limits.shmmni
            && stats.msg_queues <= self.limits.msgmni
            && stats.sem_arrays <= self.limits.semmni
            && stats.total_shm_pages <= self.limits.shmall
            && stats.total_semaphores <= self.limits.semmns
    }

    /// Clear all IPC resources (shared memory, message queues, semaphores)
    pub fn clear(&mut self) {
        // Clear shared memory resources
        self.shm.lock().clear();

        // Clear message queue resources
        self.msg.lock().clear();

        // Clear semaphore resources
        self.sem.lock().clear();
    }
}

impl Default for IpcManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive IPC resource statistics
#[derive(Debug, Clone, Copy)]
pub struct IpcStats {
    /// Number of shared memory segments
    pub shm_segments: usize,
    /// Number of message queues
    pub msg_queues: usize,
    /// Number of semaphore arrays
    pub sem_arrays: usize,
    /// Total pages used by shared memory
    pub total_shm_pages: usize,
    /// Total bytes used by message queues
    pub total_msg_bytes: usize,
    /// Total number of messages across all queues
    pub total_messages: usize,
    /// Total number of semaphores across all arrays
    pub total_semaphores: usize,
}

def_resource! {
    /// Global IPC manager instance
    pub static IPC_MANAGER: ResArc<Mutex<IpcManager>> = ResArc::new();
}

impl IPC_MANAGER {
    /// Create a copy of the inner IPC manager
    pub fn copy_inner(&self) -> Mutex<IpcManager> {
        Mutex::new(self.lock().clone())
    }

    /// Clear the inner IPC manager
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Execute a closure with access to the shared memory manager
    pub fn with_shm<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut ShmManager) -> R,
    {
        let manager = self.lock();
        let mut shm_manager = manager.get_shm().lock();
        f(&mut shm_manager)
    }

    /// Execute a closure with access to the message queue manager
    pub fn with_msg<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut MsgManager) -> R,
    {
        let manager = self.lock();
        let mut msg_manager = manager.get_msg().lock();
        f(&mut msg_manager)
    }

    /// Execute a closure with access to the semaphore manager
    pub fn with_sem<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut SemManager) -> R,
    {
        let manager = self.lock();
        let mut sem_manager = manager.get_sem().lock();
        f(&mut sem_manager)
    }
}

/// Macro to simplify IPC manager access
#[macro_export]
macro_rules! with_ipc_manager {
    (shm, $var:ident, $body:expr) => {
        $crate::ipc::IPC_MANAGER.with_shm(|$var| $body)
    };
    (msg, $var:ident, $body:expr) => {
        $crate::ipc::IPC_MANAGER.with_msg(|$var| $body)
    };
    (sem, $var:ident, $body:expr) => {
        $crate::ipc::IPC_MANAGER.with_sem(|$var| $body)
    };
}

/// Initialize the global IPC manager
#[ctor_bare::register_ctor]
fn init_ipc_manager() {
    IPC_MANAGER.init_new(Mutex::new(IpcManager::new()));
}
