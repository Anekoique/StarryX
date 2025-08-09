use alloc::{collections::VecDeque, sync::Arc, vec::Vec};

use axerrno::{LinuxError, LinuxResult};
use axsync::Mutex;

use xprocess::Pid;
use xutils::{
    collections::btreemap::BTreeMap,
    ctypes::ipc::MsgRcvFlags,
    ctypes::{
        __kernel_mode_t, __kernel_pid_t, __kernel_size_t, __kernel_time_t, MSGMAX, MSGMNB, MSGMNI,
        MSGPOOL, MSGTQL, c_long, c_ushort,
    },
    time::monotonic_time_nanos,
};

use super::{IpcPerm, IpcidGenerator};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MsgidDs {
    pub msg_perm: IpcPerm,
    pub msg_stime: __kernel_time_t,  // last msgsnd time
    pub msg_rtime: __kernel_time_t,  // last msgrcv time
    pub msg_ctime: __kernel_time_t,  // last change time
    pub msg_cbytes: __kernel_size_t, // current number of bytes on queue
    pub msg_qnum: __kernel_size_t,   // number of messages in queue
    pub msg_qbytes: __kernel_size_t, // max number of bytes on queue
    pub msg_lspid: __kernel_pid_t,   // pid of last msgsnd
    pub msg_lrpid: __kernel_pid_t,   // pid of last msgrcv
}

impl MsgidDs {
    pub fn new(key: i32, mode: __kernel_mode_t, pid: __kernel_pid_t) -> Self {
        Self {
            msg_perm: IpcPerm {
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
            msg_stime: 0,
            msg_rtime: 0,
            msg_ctime: monotonic_time_nanos() as __kernel_time_t,
            msg_cbytes: 0,
            msg_qnum: 0,
            msg_qbytes: MSGMNB as __kernel_size_t,
            msg_lspid: pid,
            msg_lrpid: 0,
        }
    }
}

#[derive(Clone)]
pub struct Message {
    pub mtype: c_long,   // message type
    pub mtext: Vec<u8>,  // message text
    pub sender_pid: Pid, // sender process ID
    pub timestamp: __kernel_time_t,
}

impl Message {
    pub fn new(mtype: c_long, mtext: Vec<u8>, sender_pid: Pid) -> Self {
        Self {
            mtype,
            mtext,
            sender_pid,
            timestamp: monotonic_time_nanos() as __kernel_time_t,
        }
    }

    pub fn size(&self) -> usize {
        self.mtext.len()
    }
}

#[derive(Clone)]
pub struct MsgQueue {
    pub msgid: i32,
    pub messages: VecDeque<Message>,
    pub rmid: bool,
    pub msqid_ds: MsgidDs,
    pub waiting_senders: Vec<Pid>,   // processes waiting to send
    pub waiting_receivers: Vec<Pid>, // processes waiting to receive
}

impl MsgQueue {
    pub fn new(key: i32, msgid: i32, mode: __kernel_mode_t, pid: Pid) -> Self {
        Self {
            msgid,
            messages: VecDeque::new(),
            rmid: false,
            msqid_ds: MsgidDs::new(key, mode, pid as __kernel_pid_t),
            waiting_senders: Vec::new(),
            waiting_receivers: Vec::new(),
        }
    }

    pub fn try_update(&mut self, mode: __kernel_mode_t, _pid: Pid) -> LinuxResult<isize> {
        if mode != self.msqid_ds.msg_perm.mode {
            return Err(LinuxError::EINVAL);
        }
        Ok(self.msgid as isize)
    }

    pub fn can_send(&self, msg_size: usize) -> bool {
        let current_bytes = self.msqid_ds.msg_cbytes as usize;
        let max_bytes = self.msqid_ds.msg_qbytes as usize;

        current_bytes + msg_size <= max_bytes && msg_size <= MSGMAX
    }

    pub fn send_message(&mut self, msg: Message) -> LinuxResult<()> {
        if !self.can_send(msg.size()) {
            return Err(LinuxError::EAGAIN);
        }

        let msg_size = msg.size();
        self.messages.push_back(msg.clone());
        self.msqid_ds.msg_cbytes += msg_size as __kernel_size_t;
        self.msqid_ds.msg_qnum += 1;
        self.msqid_ds.msg_stime = monotonic_time_nanos() as __kernel_time_t;
        self.msqid_ds.msg_lspid = msg.sender_pid as __kernel_pid_t;

        Ok(())
    }

    pub fn receive_message(
        &mut self,
        msgtyp: c_long,
        msgflg: u32,
        pid: Pid,
    ) -> LinuxResult<Message> {
        let msg_index = self.find_message(msgtyp, msgflg)?;

        match msg_index {
            Some(index) => {
                let msg = self.messages.remove(index).unwrap();
                self.msqid_ds.msg_cbytes -= msg.size() as __kernel_size_t;
                self.msqid_ds.msg_qnum -= 1;
                self.msqid_ds.msg_rtime = monotonic_time_nanos() as __kernel_time_t;
                self.msqid_ds.msg_lrpid = pid as __kernel_pid_t;
                Ok(msg)
            }
            None => {
                if msgflg & MsgRcvFlags::IPC_NOWAIT.bits() != 0 {
                    Err(LinuxError::ENOMSG)
                } else {
                    // Would block - caller should handle waiting
                    Err(LinuxError::EAGAIN)
                }
            }
        }
    }

    fn find_message(&self, msgtyp: c_long, _msgflg: u32) -> LinuxResult<Option<usize>> {
        match msgtyp {
            0 => {
                // Get first message
                if self.messages.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(0))
                }
            }
            n if n > 0 => {
                // Get first message of type msgtyp
                self.messages
                    .iter()
                    .position(|msg| msg.mtype == msgtyp)
                    .map_or(Ok(None), |i| Ok(Some(i)))
            }
            n => {
                // msgtyp < 0: get message with lowest type <= |msgtyp|
                let abs_msgtyp = n.abs();
                let (index, _) = self
                    .messages
                    .iter()
                    .enumerate()
                    .filter(|(_, msg)| msg.mtype <= abs_msgtyp)
                    .min_by_key(|(_, msg)| msg.mtype)
                    .unzip();
                Ok(index)
            }
        }
    }

    pub fn get_queue_info(&self) -> MsgidDs {
        self.msqid_ds
    }

    pub fn set_queue_bytes(&mut self, qbytes: __kernel_size_t) -> LinuxResult<()> {
        if qbytes > MSGMNB as __kernel_size_t {
            return Err(LinuxError::EINVAL);
        }
        self.msqid_ds.msg_qbytes = qbytes;
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn total_bytes(&self) -> usize {
        self.msqid_ds.msg_cbytes as usize
    }
}

pub struct MsgManager {
    index: BTreeMap<i32, i32>,                   // key -> msgid mapping
    queues: BTreeMap<i32, Arc<Mutex<MsgQueue>>>, // msgid -> queue mapping
    id_generator: Mutex<IpcidGenerator>,
}

impl MsgManager {
    pub const fn new() -> Self {
        Self {
            index: BTreeMap::new(),
            queues: BTreeMap::new(),
            id_generator: Mutex::new(IpcidGenerator::new()),
        }
    }

    // used by sys_msgget
    pub fn get_msgid_by_key(&self, key: i32) -> Option<i32> {
        self.index.get(&key).cloned()
    }

    // get message queue by msgid
    pub fn get_queue_by_msgid(&self, msgid: i32) -> Option<Arc<Mutex<MsgQueue>>> {
        self.queues.get(&msgid).cloned()
    }

    // used by sys_msgget
    pub fn insert_key_msgid(&mut self, key: i32, msgid: i32) {
        self.index.insert(key, msgid);
    }

    // used by sys_msgget
    pub fn insert_msgid_queue(&mut self, msgid: i32, queue: Arc<Mutex<MsgQueue>>) {
        self.queues.insert(msgid, queue);
    }

    // cleanup when queue is removed
    pub fn remove_msgid(&mut self, msgid: i32) {
        // Find and remove the key mapping
        let mut key_to_remove = None;
        for (key, id) in &self.index {
            if *id == msgid {
                key_to_remove = Some(*key);
                break;
            }
        }

        if let Some(key) = key_to_remove {
            self.index.remove(&key);
        }

        self.queues.remove(&msgid);
    }

    pub fn allocate_msgid(&self) -> i32 {
        self.id_generator.lock().alloc()
    }

    pub fn get_all_msgids(&self) -> Vec<i32> {
        self.queues.keys().cloned().collect()
    }

    pub fn queue_count(&self) -> usize {
        self.queues.len()
    }

    pub fn total_queues_bytes(&self) -> usize {
        let mut total = 0;
        for queue in self.queues.values() {
            total += queue.lock().total_bytes();
        }
        total
    }

    pub fn total_messages(&self) -> usize {
        let mut total = 0;
        for queue in self.queues.values() {
            total += queue.lock().message_count();
        }
        total
    }

    /// Clear all message queue resources
    pub fn clear(&mut self) {
        // Get all message queue IDs to remove
        let all_msgids: Vec<i32> = self.queues.keys().cloned().collect();

        // Mark all queues for removal and clear their messages
        for msgid in all_msgids {
            if let Some(queue_arc) = self.queues.get(&msgid) {
                let mut queue = queue_arc.lock();
                queue.rmid = true;
                // Clear all messages in the queue
                queue.messages.clear();
                queue.msqid_ds.msg_cbytes = 0;
                queue.msqid_ds.msg_qnum = 0;
                // Clear waiting processes
                queue.waiting_senders.clear();
                queue.waiting_receivers.clear();
            }
            self.remove_msgid(msgid);
        }

        // Clear all mappings
        self.index.clear();
        self.queues.clear();
    }
}

impl Clone for MsgManager {
    fn clone(&self) -> Self {
        Self {
            index: self.index.clone(),
            queues: self.queues.clone(),
            id_generator: Mutex::new(self.id_generator.lock().clone()),
        }
    }
}

// Message queue statistics structure
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MsgInfo {
    pub msgpool: c_long,
    pub msgmap: c_long,
    pub msgmax: c_long,
    pub msgmnb: c_long,
    pub msgmni: c_long,
    pub msgssz: c_long,
    pub msgtql: c_long,
    pub msgseg: c_ushort,
}

impl Default for MsgInfo {
    fn default() -> Self {
        Self {
            msgpool: MSGPOOL as c_long,
            msgmap: MSGMNI as c_long,
            msgmax: MSGMAX as c_long,
            msgmnb: MSGMNB as c_long,
            msgmni: MSGMNI as c_long,
            msgssz: 16,
            msgtql: MSGTQL as c_long,
            msgseg: 0x7fff,
        }
    }
}
