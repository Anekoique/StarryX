use alloc::vec::Vec;
use axerrno::{LinuxError, LinuxResult};
use axprocess::Pid;
use axtask::current;
use starry_core::task::TaskExt;

use crate::{
    ctypes::__kernel_time_t,
    ipc::{
        IPC_MANAGER, IPC_PRIVATE, IPC_RMID, IPC_SET, IPC_STAT, SEMMSL, SEMOPM, SemBuf, SemInfo,
        SemOpFlags,
    },
    ptr::{UserConstPtr, UserPtr, nullable},
    time::monotonic_time_nanos,
};

/// Get a semaphore set identifier
pub fn sys_semget(key: i32, nsems: i32, semflg: i32) -> LinuxResult<isize> {
    if nsems < 0 || nsems as usize > SEMMSL {
        return Err(LinuxError::EINVAL);
    }

    let cur_pid = TaskExt::from_task(&current()).thread.process().pid();
    let ipc_manager = IPC_MANAGER.lock();
    let mut sem_manager = ipc_manager.get_sem().lock();

    // If not IPC_PRIVATE, check if semaphore set already exists
    if key != IPC_PRIVATE {
        if let Some(semid) = sem_manager.get_semid_by_key(key) {
            // Existing semaphore set found
            if let Some(semset_arc) = sem_manager.get_semset_by_id(semid) {
                let semset = semset_arc.lock();
                // Check if nsems matches (if nsems > 0)
                if nsems > 0 && nsems as usize != semset.semaphores.len() {
                    return Err(LinuxError::EINVAL);
                }
                return Ok(semid as isize);
            }
        }
    }

    // Create new semaphore set
    if nsems == 0 {
        return Err(LinuxError::EINVAL);
    }

    let mode = (semflg & 0o777) as u32;
    let semid = sem_manager.create_semset(key, nsems as usize, mode, cur_pid)?;

    Ok(semid as isize)
}

/// Perform operations on semaphores
pub fn sys_semop(semid: i32, sops: UserConstPtr<SemBuf>, nsops: usize) -> LinuxResult<isize> {
    if nsops == 0 || nsops > SEMOPM {
        return Err(LinuxError::EINVAL);
    }

    // Read operations from user space
    let mut operations = Vec::with_capacity(nsops);
    for i in 0..nsops {
        let sop = sops.offset(i).get_as_ref()?;
        operations.push(*sop);
    }

    let cur_pid = TaskExt::from_task(&current()).thread.process().pid();
    let ipc_manager = IPC_MANAGER.lock();
    let sem_manager = ipc_manager.get_sem().lock();

    let semset_arc = sem_manager
        .get_semset_by_id(semid)
        .ok_or(LinuxError::EINVAL)?;

    let mut semset = semset_arc.lock();

    // Validate operations
    for op in &operations {
        if op.sem_num as usize >= semset.semaphores.len() {
            return Err(LinuxError::EFBIG);
        }
    }

    // Check if all operations can be performed immediately
    let mut should_wait = false;
    let mut has_nowait = false;

    for op in &operations {
        let flags = SemOpFlags::from_bits_truncate(op.sem_flg);
        if flags.contains(SemOpFlags::IPC_NOWAIT) {
            has_nowait = true;
        }
    }

    if !semset.can_perform_operations(&operations) {
        if has_nowait {
            return Err(LinuxError::EAGAIN);
        }
        should_wait = true;
    }

    if should_wait {
        // Add to waiting queue and block
        semset.add_waiting_process(cur_pid, operations.clone());
        let wait_queue = semset.wait_queue.clone();
        drop(semset);
        drop(sem_manager);
        drop(ipc_manager);

        // Wait for the operation to complete
        wait_queue.wait();

        // Re-acquire locks and check if operation was successful
        // let ipc_manager = IPC_MANAGER.lock();
        // let sem_manager = ipc_manager.get_sem().lock();
        // let semset_arc = sem_manager.get_semset_by_id(semid)
        //     .ok_or(LinuxError::EIDRM)?;
        // let semset = semset_arc.lock();

        // The operation should have been performed by the waking process
        // or we were woken up due to an error condition
    } else {
        // Perform operations immediately
        semset.perform_operations(&operations, cur_pid)?;

        // Add undo operations if SEM_UNDO is set
        drop(semset);
        let mut sem_manager = sem_manager;
        for op in &operations {
            let flags = SemOpFlags::from_bits_truncate(op.sem_flg);
            if flags.contains(SemOpFlags::SEM_UNDO) && op.sem_op != 0 {
                sem_manager.add_undo_operation(cur_pid, semid, op.sem_num as usize, op.sem_op);
            }
        }

        // Wake up other waiting processes
        let semset_arc = sem_manager.get_semset_by_id(semid).unwrap();
        let mut semset = semset_arc.lock();
        semset.wake_up_processes();
    }

    Ok(0)
}

// Semctl command constants
const GETVAL: u32 = 12;
const SETVAL: u32 = 16;
const GETPID: u32 = 11;
const GETNCNT: u32 = 14;
const GETZCNT: u32 = 15;
const GETALL: u32 = 13;
const SETALL: u32 = 17;

/// Control operations on semaphores
pub fn sys_semctl(semid: i32, semnum: i32, cmd: u32, arg: usize) -> LinuxResult<isize> {
    let ipc_manager = IPC_MANAGER.lock();
    let mut sem_manager = ipc_manager.get_sem().lock();

    let semset_arc = sem_manager
        .get_semset_by_id(semid)
        .ok_or(LinuxError::EINVAL)?;

    match cmd {
        IPC_RMID => {
            let mut semset = semset_arc.lock();
            semset.rmid = true;
            // Wake up all waiting processes with error
            while let Some(mut waiting) = semset.waiting_queue.lock().pop_front() {
                waiting.error = Some(LinuxError::EIDRM);
                semset.wait_queue.notify_all(true);
            }
            drop(semset);
            sem_manager.remove_semset(semid);
            Ok(0)
        }
        IPC_SET => {
            let buf_ptr = UserConstPtr::<SemInfo>::from(arg);
            let new_info = buf_ptr.get_as_ref()?;
            let mut semset = semset_arc.lock();
            semset.sem_info = *new_info;
            semset.sem_info.sem_ctime = monotonic_time_nanos() as __kernel_time_t;
            Ok(0)
        }
        IPC_STAT => {
            let buf_ptr = UserPtr::<SemInfo>::from(arg);
            let semset = semset_arc.lock();
            if let Some(buf) = nullable!(buf_ptr.get_as_mut())? {
                *buf = semset.sem_info;
            }
            Ok(0)
        }
        GETVAL => {
            if semnum < 0 || semnum as usize >= semset_arc.lock().semaphores.len() {
                return Err(LinuxError::EINVAL);
            }
            let semset = semset_arc.lock();
            Ok(semset.semaphores[semnum as usize].semval as isize)
        }
        SETVAL => {
            if semnum < 0 || semnum as usize >= semset_arc.lock().semaphores.len() {
                return Err(LinuxError::EINVAL);
            }
            let mut semset = semset_arc.lock();
            let val = arg as i16;
            if val < 0 || val as usize > crate::ipc::SEMVMX {
                return Err(LinuxError::ERANGE);
            }
            semset.semaphores[semnum as usize].semval = val;
            semset.semaphores[semnum as usize].sempid =
                TaskExt::from_task(&current()).thread.process().pid();
            semset.sem_info.sem_ctime = monotonic_time_nanos() as __kernel_time_t;
            semset.wake_up_processes();
            Ok(0)
        }
        GETPID => {
            if semnum < 0 || semnum as usize >= semset_arc.lock().semaphores.len() {
                return Err(LinuxError::EINVAL);
            }
            let semset = semset_arc.lock();
            Ok(semset.semaphores[semnum as usize].sempid as isize)
        }
        GETNCNT => {
            if semnum < 0 || semnum as usize >= semset_arc.lock().semaphores.len() {
                return Err(LinuxError::EINVAL);
            }
            let semset = semset_arc.lock();
            Ok(semset.semaphores[semnum as usize].semncnt as isize)
        }
        GETZCNT => {
            if semnum < 0 || semnum as usize >= semset_arc.lock().semaphores.len() {
                return Err(LinuxError::EINVAL);
            }
            let semset = semset_arc.lock();
            Ok(semset.semaphores[semnum as usize].semzcnt as isize)
        }
        GETALL => {
            let buf_ptr = UserPtr::<i16>::from(arg);
            let semset = semset_arc.lock();
            for (i, sem) in semset.semaphores.iter().enumerate() {
                *buf_ptr.offset(i).get_as_mut()? = sem.semval;
            }
            Ok(0)
        }
        SETALL => {
            let buf_ptr = UserConstPtr::<i16>::from(arg);
            let mut semset = semset_arc.lock();
            for (i, sem) in semset.semaphores.iter_mut().enumerate() {
                let val = *buf_ptr.offset(i).get_as_ref()?;
                if val < 0 || val as usize > crate::ipc::SEMVMX {
                    return Err(LinuxError::ERANGE);
                }
                sem.semval = val;
                sem.sempid = TaskExt::from_task(&current()).thread.process().pid();
            }
            semset.sem_info.sem_ctime = monotonic_time_nanos() as __kernel_time_t;
            semset.wake_up_processes();
            Ok(0)
        }
        _ => Err(LinuxError::EINVAL),
    }
}

/// Called when a process exits to perform undo operations
pub fn clear_proc_sem(pid: Pid) {
    let ipc_manager = IPC_MANAGER.lock();
    let mut sem_manager = ipc_manager.get_sem().lock();
    sem_manager.perform_undo_operations(pid);
}
