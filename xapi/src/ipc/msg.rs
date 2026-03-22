//! Message queue system calls implementation.
use alloc::{sync::Arc, vec};

use axerrno::{LinuxError, LinuxResult};
use axsync::Mutex;

use xuspace::{UserPtr, UserSpaceAccess};
use xutils::ctypes::{
    __kernel_mode_t, IPC_CREAT, IPC_EXCL, IPC_INFO, IPC_PRIVATE, IPC_RMID, IPC_SET, IPC_STAT,
    c_long,
    ipc::{MsgRcvFlags, MsgSndFlags},
};

use xcore::{
    ipc::{IPC_MANAGER, Message, MsgQueue, MsgidDs},
    task::{with_process, with_uspace},
    with_ipc_manager,
};

/// Helper function to get current process ID
fn current_pid() -> xprocess::Pid {
    with_process(|process| process.pid())
}

/// Get message queue statistics (example using macro)
pub fn get_msg_stats() -> (usize, usize, usize) {
    with_ipc_manager!(msg, manager, {
        (
            manager.queue_count(),
            manager.total_queues_bytes(),
            manager.total_messages(),
        )
    })
}

/// Get message queue identifier
///
/// # Arguments
/// * `key` - IPC key for the message queue
/// * `msgflg` - Flags controlling creation and permissions
pub fn sys_msgget(key: i32, msgflg: i32) -> LinuxResult<isize> {
    info!("sys_msgget: key = {}, msgflg = {}", key, msgflg);
    let current_pid = current_pid();

    // Acquire the limits before acquiring the msg lock.
    let limits = *IPC_MANAGER.lock().get_limits();

    IPC_MANAGER.with_msg(|msg_manager| {
        // Check if key already exists
        if key != IPC_PRIVATE
            && let Some(existing_msgid) = msg_manager.get_msgid_by_key(key)
        {
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

        // Create new message queue
        if msgflg & IPC_CREAT as i32 == 0 {
            return Err(LinuxError::ENOENT);
        }

        // Check system limits
        if msg_manager.queue_count() >= limits.msgmni {
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
    })
}

/// Send message to queue
///
/// # Arguments
/// * `msqid` - Message queue identifier
/// * `msgp` - Pointer to message structure
/// * `msgsz` - Size of message text
/// * `msgflg` - Flags controlling send behavior
pub fn sys_msgsnd(msqid: i32, msgp: UserPtr<u8>, msgsz: usize, msgflg: i32) -> LinuxResult<isize> {
    info!(
        "sys_msgsnd: msqid = {}, msgsz = {}, msgflg = {}",
        msqid, msgsz, msgflg
    );

    // Check message size against system limits
    let limits = *IPC_MANAGER.lock().get_limits();
    if msgsz > limits.msgmax {
        return Err(LinuxError::EINVAL);
    }

    let msg = with_uspace(|uspace| {
        let mtype_ptr = msgp.cast::<c_long>();
        let mtype = uspace.read(mtype_ptr)?;

        if mtype <= 0 {
            return Err(LinuxError::EINVAL);
        }

        let mtext_ptr = msgp.offset(core::mem::size_of::<c_long>());
        let mut mtext = vec![0u8; msgsz];
        uspace.read_slice_to(mtext_ptr, &mut mtext)?;

        Ok(Message::new(mtype, mtext, current_pid()))
    })?;

    IPC_MANAGER.with_msg(|msg_manager| {
        let queue_arc = msg_manager
            .get_queue_by_msgid(msqid)
            .ok_or(LinuxError::EINVAL)?;

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
                    // TODO: In real implementation, would block here
                    // For now, just return error
                    Err(LinuxError::EAGAIN)
                }
            }
            Err(e) => Err(e),
        }
    })
}

/// Receive message from queue
///
/// # Arguments
/// * `msqid` - Message queue identifier
/// * `msgp` - Pointer to message buffer
/// * `msgsz` - Maximum message size
/// * `msgtyp` - Message type filter
/// * `msgflg` - Flags controlling receive behavior
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

    IPC_MANAGER.with_msg(|msg_manager| {
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
        match queue.receive_message(msgtyp, msgflg as u32, current_pid()) {
            Ok(msg) => {
                if msg.mtext.len() > msgsz && (msgflg & MsgRcvFlags::MSG_NOERROR.bits() as i32 == 0)
                {
                    return Err(LinuxError::E2BIG);
                }

                // Copy message to user space (simplified)
                with_uspace(|uspace| {
                    let mtype_ptr = msgp.cast::<c_long>();
                    uspace.write(mtype_ptr, msg.mtype)?;
                    let copy_size = core::cmp::min(msg.size(), msgsz);
                    let text_ptr = msgp.offset(core::mem::size_of::<c_long>());
                    uspace.write_slice(text_ptr, &msg.mtext[..copy_size])
                })?;

                Ok(msg.mtext.len() as isize)
            }
            Err(LinuxError::EAGAIN) => {
                if msgflg & MsgRcvFlags::IPC_NOWAIT.bits() as i32 != 0 {
                    Err(LinuxError::ENOMSG)
                } else {
                    // TODO: In real implementation, would block here
                    Err(LinuxError::EAGAIN)
                }
            }
            Err(e) => Err(e),
        }
    })
}

/// Message queue control operations
///
/// # Arguments
/// * `msqid` - Message queue identifier
/// * `cmd` - Control command (IPC_STAT, IPC_SET, IPC_RMID, etc.)
/// * `buf` - Buffer for queue information
pub fn sys_msgctl(msqid: i32, cmd: i32, buf: UserPtr<MsgidDs>) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        match cmd as u32 {
            IPC_STAT => {
                with_ipc_manager!(msg, msg_manager, {
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
                        uspace.write(buf, queue.get_queue_info())?;
                    }

                    Ok(0)
                })
            }

            IPC_SET => {
                with_ipc_manager!(msg, msg_manager, {
                    let queue_arc = msg_manager
                        .get_queue_by_msgid(msqid)
                        .ok_or(LinuxError::EINVAL)?;

                    let mut queue = queue_arc.lock();
                    if queue.rmid {
                        return Err(LinuxError::EIDRM);
                    }

                    // TODO: uid? Check permissions (owner or superuser)
                    if !buf.is_null() {
                        let new_info = uspace.read(buf)?;
                        // Update modifiable fields
                        queue.msqid_ds.msg_perm.uid = new_info.msg_perm.uid;
                        queue.msqid_ds.msg_perm.gid = new_info.msg_perm.gid;
                        queue.msqid_ds.msg_perm.mode = (queue.msqid_ds.msg_perm.mode & !0o777)
                            | (new_info.msg_perm.mode & 0o777);
                        queue.msqid_ds.msg_qbytes = new_info.msg_qbytes;
                        queue.msqid_ds.msg_ctime =
                            xutils::time::monotonic_time_nanos() as xutils::ctypes::__kernel_time_t;
                    }

                    Ok(0)
                })
            }

            IPC_RMID => {
                with_ipc_manager!(msg, msg_manager, {
                    let queue_arc = msg_manager
                        .get_queue_by_msgid(msqid)
                        .ok_or(LinuxError::EINVAL)?;

                    let mut queue = queue_arc.lock();
                    if queue.rmid {
                        return Err(LinuxError::EIDRM);
                    }

                    // TODO: uid? Check permissions (owner or superuser)

                    // Mark for removal
                    queue.rmid = true;

                    // TODO: Wake up all waiting processes with EIDRM
                    // In a real implementation, you would notify all blocked processes

                    // Remove from manager
                    drop(queue);
                    msg_manager.remove_msgid(msqid);

                    Ok(0)
                })
            }

            IPC_INFO => Ok(0),

            _ => Err(LinuxError::EINVAL),
        }
    })
}
