use axns::{ResArc, def_resource};
use axsync::Mutex;
use core::sync::atomic::{AtomicI32, Ordering};

use super::msg::{MSGMAX, MSGMNB, MSGMNI, MsgManager};
use super::shm::{SHMALL, SHMMAX, SHMMNI, ShmManager};
use crate::utils::ctypes::{
    __kernel_gid_t, __kernel_key_t, __kernel_mode_t, __kernel_uid_t, c_long, c_ushort,
};

pub const IPC_PRIVATE: i32 = 0;

pub const IPC_CREAT: u32 = 0o1000;
pub const IPC_EXCL: u32 = 0o2000;
pub const IPC_NOWAIT: u32 = 0o4000;

pub const IPC_RMID: u32 = 0;
pub const IPC_SET: u32 = 1;
pub const IPC_STAT: u32 = 2;
pub const IPC_INFO: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IpcPerm {
    pub key: __kernel_key_t,
    pub uid: __kernel_uid_t,
    pub gid: __kernel_gid_t,
    pub cuid: __kernel_uid_t,
    pub cgid: __kernel_gid_t,
    pub mode: __kernel_mode_t,
    pub seq: c_ushort,
    pub pad: c_ushort,   // for memory align
    pub unused0: c_long, // for memory align
    pub unused1: c_long, // for memory align
}

pub struct IpcidGenerator {
    next_ipcid: AtomicI32,
}

impl Clone for IpcidGenerator {
    fn clone(&self) -> Self {
        IpcidGenerator {
            next_ipcid: AtomicI32::new(self.next_ipcid.load(Ordering::SeqCst)),
        }
    }
}

impl IpcidGenerator {
    pub const fn new() -> Self {
        IpcidGenerator {
            next_ipcid: AtomicI32::new(0),
        }
    }

    pub fn alloc(&self) -> i32 {
        self.next_ipcid.fetch_add(1, Ordering::SeqCst)
    }
}

#[derive(Clone, Copy)]
pub struct IpcLimits {
    // Shared Memory limits
    pub shmmax: usize,
    pub shmmni: usize,
    pub shmall: usize,

    // Message Queue limits
    pub msgmax: usize,
    pub msgmnb: usize,
    pub msgmni: usize,
    // TODO: Semaphore limits (for future implementation)
    // pub semmsl: usize,
    // pub semmns: usize,
    // pub semopm: usize,
    // pub semmni: usize,
    // pub semvmx: usize,
}

impl Default for IpcLimits {
    fn default() -> Self {
        IpcLimits {
            // Shared Memory
            shmmax: SHMMAX,
            shmmni: SHMMNI,
            shmall: SHMALL,

            // Message Queue
            msgmax: MSGMAX,
            msgmnb: MSGMNB,
            msgmni: MSGMNI,
        }
    }
}

pub struct IpcManager {
    shm: Mutex<ShmManager>,
    msg: Mutex<MsgManager>,
    // TODO: implement System V semaphores
    // sem: Mutex<SemManager>,
    limits: IpcLimits,
}

impl Clone for IpcManager {
    fn clone(&self) -> Self {
        IpcManager {
            shm: Mutex::new(self.shm.lock().clone()),
            msg: Mutex::new(self.msg.lock().clone()),
            limits: self.limits,
        }
    }
}

impl IpcManager {
    pub fn new() -> Self {
        IpcManager {
            shm: Mutex::new(ShmManager::new()),
            msg: Mutex::new(MsgManager::new()),
            limits: IpcLimits::default(),
        }
    }

    pub fn get_shm(&self) -> &Mutex<ShmManager> {
        &self.shm
    }

    pub fn get_msg(&self) -> &Mutex<MsgManager> {
        &self.msg
    }

    pub fn get_limits(&self) -> &IpcLimits {
        &self.limits
    }

    pub fn set_limits(&mut self, limits: IpcLimits) {
        self.limits = limits;
    }

    // Get statistics for all IPC resources
    pub fn get_ipc_stats(&self) -> IpcStats {
        let shm_manager = self.shm.lock();
        let msg_manager = self.msg.lock();

        IpcStats {
            shm_segments: shm_manager.segment_count(),
            msg_queues: msg_manager.queue_count(),
            total_shm_pages: 0, // Would need to calculate from segments
            total_msg_bytes: msg_manager.total_queues_bytes(),
            total_messages: msg_manager.total_messages(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IpcStats {
    pub shm_segments: usize,
    pub msg_queues: usize,
    pub total_shm_pages: usize,
    pub total_msg_bytes: usize,
    pub total_messages: usize,
}

def_resource! {
    pub static IPC_MANAGER: ResArc<Mutex<IpcManager>> = ResArc::new();
}

impl IPC_MANAGER {
    pub fn copy_inner(&self) -> Mutex<IpcManager> {
        Mutex::new(self.lock().clone())
    }
}

// TODO: implement System V Ipc with IpcOps
// pub trait IpcOps {
//     fn get_new() -> i32;
// }

#[ctor_bare::register_ctor]
fn init_ipc_manager() {
    IPC_MANAGER.init_new(Mutex::new(IpcManager::new()));
}
