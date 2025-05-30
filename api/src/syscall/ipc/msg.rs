use alloc::{sync::Arc, vec};
use axerrno::{LinuxError, LinuxResult};
use axsync::Mutex;
use axtask::{TaskExtRef, current};

use crate::{
    ctypes::{__kernel_mode_t, c_long},
    ipc::{
        IPC_CREAT, IPC_EXCL, IPC_INFO, IPC_MANAGER, IPC_PRIVATE, IPC_RMID, IPC_SET, IPC_STAT,
        Message, MsgQueue, MsgRcvFlags, MsgSndFlags, MsgidDs,
    },
    ptr::UserPtr,
};

// System call: msgget - get message queue identifier
pub fn sys_msgget(key: i32, msgflg: i32) -> LinuxResult<isize> {
    info!("sys_msgget: key = {}, msgflg = {}", key, msgflg);
    let current_pid = current().task_ext().thread.process().pid();
    let ipc_manager = IPC_MANAGER.lock();
    let mut msg_manager = ipc_manager.get_msg().lock();

    // Check if key already exists
    if key != IPC_PRIVATE {
        if let Some(existing_msgid) = msg_manager.get_msgid_by_key(key) {
            // Key exists, check flags
            if msgflg & (IPC_CREAT as i32 | IPC_EXCL as i32) == (IPC_CREAT as i32 | IPC_EXCL as i32)
            {
                return Err(LinuxError::EEXIST);
            }

            // Check permissions
            if let Some(queue_arc) = msg_manager.get_queue_by_msgid(existing_msgid) {
                let queue = queue_arc.lock();
                let mode = queue.msqid_ds.msg_perm.mode;

                // Basic permission check (simplified)
                if (msgflg & 0o777) & !(mode as i32 & 0o777) != 0 {
                    return Err(LinuxError::EACCES);
                }

                return Ok(existing_msgid as isize);
            }
        }
    }

    // Create new message queue
    if msgflg & IPC_CREAT as i32 == 0 {
        return Err(LinuxError::ENOENT);
    }

    // Check system limits
    if msg_manager.queue_count() >= ipc_manager.get_limits().msgmni {
        return Err(LinuxError::ENOSPC);
    }

    let msgid = msg_manager.allocate_msgid();
    let mode = (msgflg & 0o777) as __kernel_mode_t;
    let queue = Arc::new(Mutex::new(MsgQueue::new(key, msgid, mode, current_pid)));

    msg_manager.insert_msgid_queue(msgid, queue);
    if key != IPC_PRIVATE {
        msg_manager.insert_key_msgid(key, msgid);
    }

    Ok(msgid as isize)
}

// System call: msgsnd - send message to queue
pub fn sys_msgsnd(msqid: i32, msgp: UserPtr<u8>, msgsz: usize, msgflg: i32) -> LinuxResult<isize> {
    info!(
        "sys_msgsnd: msqid = {}, msgsz = {}, msgflg = {}",
        msqid, msgsz, msgflg
    );
    let ipc_manager = IPC_MANAGER.lock();
    if msgsz > ipc_manager.get_limits().msgmax {
        return Err(LinuxError::EINVAL);
    }

    let current_pid = current().task_ext().thread.process().pid();

    let msg_manager = ipc_manager.get_msg().lock();

    let queue_arc = msg_manager
        .get_queue_by_msgid(msqid)
        .ok_or(LinuxError::EINVAL)?;
    // Read message from user space (simplified - in real implementation would use copy_from_user)
    let mtype_ptr = msgp.cast::<c_long>();
    let mtype = *mtype_ptr.get_as_mut()?;

    if mtype <= 0 {
        return Err(LinuxError::EINVAL);
    }

    let mtext_ptr = msgp.offset(core::mem::size_of::<c_long>());
    let mut mtext = vec![0u8; msgsz];
    unsafe {
        core::ptr::copy_nonoverlapping(mtext_ptr.get_as_mut()?, mtext.as_mut_ptr(), msgsz);
    }

    let msg = Message::new(mtype, mtext, current_pid);

    let mut queue = queue_arc.lock();

    // Check if queue is marked for removal
    if queue.rmid {
        return Err(LinuxError::EIDRM);
    }

    // Check permissions (simplified)
    if queue.msqid_ds.msg_perm.mode & 0o200 == 0 {
        return Err(LinuxError::EACCES);
    }

    // Try to send message
    match queue.send_message(msg) {
        Ok(()) => Ok(0),
        Err(LinuxError::EAGAIN) => {
            if msgflg & MsgSndFlags::IPC_NOWAIT.bits() as i32 != 0 {
                Err(LinuxError::EAGAIN)
            } else {
                //TODO
                // In real implementation, would block here
                // For now, just return error
                Err(LinuxError::EAGAIN)
            }
        }
        Err(e) => Err(e),
    }
}

// System call: msgrcv - receive message from queue
pub fn sys_msgrcv(
    msqid: i32,
    msgp: UserPtr<u8>,
    msgsz: usize,
    msgtyp: c_long,
    msgflg: i32,
) -> LinuxResult<isize> {
    info!(
        "sys_msgrcv: msqid = {}, msgsz = {}, msgtyp = {}, msgflg = {}",
        msqid, msgsz, msgtyp, msgflg
    );
    let current_pid = current().task_ext().thread.process().pid();
    let ipc_manager = IPC_MANAGER.lock();
    let msg_manager = ipc_manager.get_msg().lock();

    let queue_arc = msg_manager
        .get_queue_by_msgid(msqid)
        .ok_or(LinuxError::EINVAL)?;

    let mut queue = queue_arc.lock();

    // Check if queue is marked for removal
    if queue.rmid {
        return Err(LinuxError::EIDRM);
    }

    // Check permissions (simplified)
    if queue.msqid_ds.msg_perm.mode & 0o400 == 0 {
        return Err(LinuxError::EACCES);
    }

    // Try to receive message
    match queue.receive_message(msgtyp, msgflg as u32, current_pid) {
        Ok(msg) => {
            if msg.mtext.len() > msgsz && (msgflg & MsgRcvFlags::MSG_NOERROR.bits() as i32 == 0) {
                return Err(LinuxError::E2BIG);
            }
            // Truncate message

            // Copy message to user space (simplified)
            unsafe {
                let mtype_ptr = msgp.cast::<c_long>();
                *mtype_ptr.get_as_mut()? = msg.mtype;
                let copy_size = core::cmp::min(msg.size(), msgsz);
                let text_ptr = msgp.offset(core::mem::size_of::<c_long>());
                core::ptr::copy_nonoverlapping(
                    msg.mtext.as_ptr(),
                    text_ptr.get_as_mut()?,
                    copy_size,
                );
            }

            Ok(msg.mtext.len() as isize)
        }
        Err(LinuxError::EAGAIN) => {
            if msgflg & MsgRcvFlags::IPC_NOWAIT.bits() as i32 != 0 {
                Err(LinuxError::ENOMSG)
            } else {
                //TODO
                // In real implementation, would block here
                Err(LinuxError::EAGAIN)
            }
        }
        Err(e) => Err(e),
    }
}

// System call: msgctl - message queue control operations
pub fn sys_msgctl(msqid: i32, cmd: i32, buf: UserPtr<MsgidDs>) -> LinuxResult<isize> {
    let ipc_manager = IPC_MANAGER.lock();
    let mut msg_manager = ipc_manager.get_msg().lock();

    match cmd as u32 {
        IPC_STAT => {
            let queue_arc = msg_manager
                .get_queue_by_msgid(msqid)
                .ok_or(LinuxError::EINVAL)?;

            let queue = queue_arc.lock();
            if queue.rmid {
                return Err(LinuxError::EIDRM);
            }

            // Check permissions
            if queue.msqid_ds.msg_perm.mode & 0o400 == 0 {
                return Err(LinuxError::EACCES);
            }

            if !buf.is_null() {
                *buf.get_as_mut()? = queue.get_queue_info();
            }

            Ok(0)
        }

        IPC_SET => {
            let queue_arc = msg_manager
                .get_queue_by_msgid(msqid)
                .ok_or(LinuxError::EINVAL)?;

            let mut queue = queue_arc.lock();
            if queue.rmid {
                return Err(LinuxError::EIDRM);
            }

            // TODO: uid?
            // Check permissions (owner or superuser)
            // if queue.msqid_ds.msg_perm.cuid != current_uid as u32 && current_uid != 0 {
            //     return Err(LinuxError::EPERM);
            // }

            if !buf.is_null() {
                let new_info = *buf.get_as_mut()?;
                // Update modifiable fields
                queue.msqid_ds.msg_perm.uid = new_info.msg_perm.uid;
                queue.msqid_ds.msg_perm.gid = new_info.msg_perm.gid;
                queue.msqid_ds.msg_perm.mode =
                    (queue.msqid_ds.msg_perm.mode & !0o777) | (new_info.msg_perm.mode & 0o777);
                queue.msqid_ds.msg_qbytes = new_info.msg_qbytes;
                queue.msqid_ds.msg_ctime =
                    crate::time::monotonic_time_nanos() as crate::ctypes::__kernel_time_t;
            }

            Ok(0)
        }

        IPC_RMID => {
            let queue_arc = msg_manager
                .get_queue_by_msgid(msqid)
                .ok_or(LinuxError::EINVAL)?;

            let mut queue = queue_arc.lock();
            if queue.rmid {
                return Err(LinuxError::EIDRM);
            }

            // TODO: uid?
            // Check permissions (owner or superuser)
            // if queue.msqid_ds.msg_perm.cuid != current_uid as u32 && current_uid != 0 {
            //     return Err(LinuxError::EPERM);
            // }

            // Mark for removal
            queue.rmid = true;

            // TODO
            // Wake up all waiting processes with EIDRM
            // In a real implementation, you would notify all blocked processes

            // Remove from manager
            drop(queue);
            msg_manager.remove_msgid(msqid);

            Ok(0)
        }

        IPC_INFO => Ok(0),

        _ => Err(LinuxError::EINVAL),
    }
}
