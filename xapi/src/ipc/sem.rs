//! Semaphore system calls implementation.
use alloc::vec::Vec;

use axerrno::{LinuxError, LinuxResult};

use xcore::{
    ipc::{IPC_MANAGER, SemBuf, SemInfo},
    task::{with_process, with_uspace},
    with_ipc_manager,
};
use xprocess::Pid;
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::{
    ctypes::{
        __kernel_time_t, IPC_PRIVATE, IPC_RMID, IPC_SET, IPC_STAT, SEMMSL, SEMOPM, ipc::SemOpFlags,
    },
    time::monotonic_time_nanos,
};

/// Helper function to get current process ID
fn current_pid() -> Pid {
    with_process(|process| process.pid())
}

/// Get semaphore statistics (example using macro)
pub fn get_sem_stats() -> (usize, usize) {
    with_ipc_manager!(sem, manager, {
        (manager.array_count(), manager.total_semaphores())
    })
}

/// Get a semaphore set identifier
///
/// # Arguments
/// * `key` - IPC key for the semaphore set
/// * `nsems` - Number of semaphores in the set
/// * `semflg` - Flags controlling creation and permissions
pub fn sys_semget(key: i32, nsems: i32, semflg: i32) -> LinuxResult<isize> {
    if nsems < 0 || nsems as usize > SEMMSL {
        return Err(LinuxError::EINVAL);
    }

    let cur_pid = current_pid();

    IPC_MANAGER.with_sem(|sem_manager| {
        // If not IPC_PRIVATE, check if semaphore set already exists
        if key != IPC_PRIVATE
            && let Some(semid) = sem_manager.get_semid_by_key(key)
        {
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

        // Create new semaphore set
        if nsems == 0 {
            return Err(LinuxError::EINVAL);
        }

        let mode = (semflg & 0o777) as u32;
        let semid = sem_manager.create_semset(key, nsems as usize, mode, cur_pid)?;

        Ok(semid as isize)
    })
}

/// Perform operations on semaphores
///
/// # Arguments
/// * `semid` - Semaphore set identifier
/// * `sops` - Array of semaphore operations
/// * `nsops` - Number of operations
pub fn sys_semop(semid: i32, sops: UserConstPtr<SemBuf>, nsops: usize) -> LinuxResult<isize> {
    if nsops == 0 || nsops > SEMOPM {
        return Err(LinuxError::EINVAL);
    }

    // Read operations from user space
    let mut operations = Vec::with_capacity(nsops);
    for i in 0..nsops {
        let sop = with_uspace(|uspace| uspace.read(sops.offset(i)))?;
        operations.push(sop);
    }

    let cur_pid = current_pid();

    // Perform operation with proper error handling
    let should_wait = IPC_MANAGER.with_sem(|sem_manager| -> LinuxResult<bool> {
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
        let mut has_nowait = false;
        for op in &operations {
            let flags = SemOpFlags::from_bits_truncate(op.sem_flg);
            if flags.contains(SemOpFlags::IPC_NOWAIT) {
                has_nowait = true;
                break;
            }
        }

        if !semset.can_perform_operations(&operations) {
            if has_nowait {
                return Err(LinuxError::EAGAIN);
            }
            // Need to wait - add to waiting queue
            semset.add_waiting_process(cur_pid, operations.clone());
            Ok(true)
        } else {
            // Perform operations immediately
            semset.perform_operations(&operations, cur_pid)?;
            Ok(false)
        }
    })?;

    if should_wait {
        // In a real implementation, we would wait here
        // For now, we'll use a simplified approach
        // TODO: Implement proper waiting mechanism

        // Wait for the operation to complete
        // This is a simplified version - in practice you'd want proper blocking
        loop {
            let completed = IPC_MANAGER.with_sem(|sem_manager| -> LinuxResult<bool> {
                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EIDRM)?;

                let mut semset = semset_arc.lock();
                if semset.can_perform_operations(&operations) {
                    semset.perform_operations(&operations, cur_pid)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            })?;

            if completed {
                break;
            }

            // Simple yield - in practice this would be a proper wait
            core::hint::spin_loop();
        }
    } else {
        // Add undo operations if SEM_UNDO is set
        IPC_MANAGER.with_sem(|sem_manager| {
            for op in &operations {
                let flags = SemOpFlags::from_bits_truncate(op.sem_flg);
                if flags.contains(SemOpFlags::SEM_UNDO) && op.sem_op != 0 {
                    sem_manager.add_undo_operation(cur_pid, semid, op.sem_num as usize, op.sem_op);
                }
            }

            // Wake up other waiting processes
            if let Some(semset_arc) = sem_manager.get_semset_by_id(semid) {
                let mut semset = semset_arc.lock();
                semset.wake_up_processes();
            }
        });
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
///
/// # Arguments
/// * `semid` - Semaphore set identifier
/// * `semnum` - Semaphore number within the set
/// * `cmd` - Control command
/// * `arg` - Command argument
pub fn sys_semctl(semid: i32, semnum: i32, cmd: u32, arg: usize) -> LinuxResult<isize> {
    match cmd {
        IPC_RMID => {
            with_ipc_manager!(sem, sem_manager, {
                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

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
            })
        }

        IPC_SET => {
            with_ipc_manager!(sem, sem_manager, {
                let buf_ptr = UserConstPtr::<SemInfo>::from(arg);
                let new_info = with_uspace(|uspace| uspace.read(buf_ptr))?;

                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

                let mut semset = semset_arc.lock();
                semset.sem_info = new_info;
                semset.sem_info.sem_ctime = monotonic_time_nanos() as __kernel_time_t;
                Ok(0)
            })
        }

        IPC_STAT => {
            with_ipc_manager!(sem, sem_manager, {
                let buf_ptr = UserPtr::<SemInfo>::from(arg);

                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

                let semset = semset_arc.lock();
                with_uspace(|uspace| nullable!(uspace.write(buf_ptr, semset.sem_info)))?;
                Ok(0)
            })
        }

        GETVAL => {
            with_ipc_manager!(sem, sem_manager, {
                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

                let semset = semset_arc.lock();
                if semnum < 0 || semnum as usize >= semset.semaphores.len() {
                    return Err(LinuxError::EINVAL);
                }
                Ok(semset.semaphores[semnum as usize].semval as isize)
            })
        }

        SETVAL => {
            with_ipc_manager!(sem, sem_manager, {
                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

                let mut semset = semset_arc.lock();
                if semnum < 0 || semnum as usize >= semset.semaphores.len() {
                    return Err(LinuxError::EINVAL);
                }

                let val = arg as i16;
                if val < 0 || val as usize > xutils::ctypes::SEMVMX {
                    return Err(LinuxError::ERANGE);
                }

                semset.semaphores[semnum as usize].semval = val;
                semset.semaphores[semnum as usize].sempid = current_pid();
                semset.sem_info.sem_ctime = monotonic_time_nanos() as __kernel_time_t;
                semset.wake_up_processes();
                Ok(0)
            })
        }

        GETPID => {
            with_ipc_manager!(sem, sem_manager, {
                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

                let semset = semset_arc.lock();
                if semnum < 0 || semnum as usize >= semset.semaphores.len() {
                    return Err(LinuxError::EINVAL);
                }
                Ok(semset.semaphores[semnum as usize].sempid as isize)
            })
        }

        GETNCNT => {
            with_ipc_manager!(sem, sem_manager, {
                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

                let semset = semset_arc.lock();
                if semnum < 0 || semnum as usize >= semset.semaphores.len() {
                    return Err(LinuxError::EINVAL);
                }
                Ok(semset.semaphores[semnum as usize].semncnt as isize)
            })
        }

        GETZCNT => {
            with_ipc_manager!(sem, sem_manager, {
                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

                let semset = semset_arc.lock();
                if semnum < 0 || semnum as usize >= semset.semaphores.len() {
                    return Err(LinuxError::EINVAL);
                }
                Ok(semset.semaphores[semnum as usize].semzcnt as isize)
            })
        }

        GETALL => {
            with_ipc_manager!(sem, sem_manager, {
                let buf_ptr = UserPtr::<i16>::from(arg);

                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

                let semset = semset_arc.lock();
                for (i, sem) in semset.semaphores.iter().enumerate() {
                    with_uspace(|uspace| uspace.write(buf_ptr.offset(i), sem.semval))?;
                }
                Ok(0)
            })
        }

        SETALL => {
            with_ipc_manager!(sem, sem_manager, {
                let buf_ptr = UserConstPtr::<i16>::from(arg);

                let semset_arc = sem_manager
                    .get_semset_by_id(semid)
                    .ok_or(LinuxError::EINVAL)?;

                let mut semset = semset_arc.lock();
                for (i, sem) in semset.semaphores.iter_mut().enumerate() {
                    let val = with_uspace(|uspace| uspace.read(buf_ptr.offset(i)))?;
                    if val < 0 || val as usize > xutils::ctypes::SEMVMX {
                        return Err(LinuxError::ERANGE);
                    }
                    sem.semval = val;
                    sem.sempid = current_pid();
                }

                semset.sem_info.sem_ctime = monotonic_time_nanos() as __kernel_time_t;
                semset.wake_up_processes();
                Ok(0)
            })
        }

        _ => Err(LinuxError::EINVAL),
    }
}

/// Called when a process exits to perform undo operations
pub fn clear_proc_sem(pid: Pid) {
    with_ipc_manager!(sem, sem_manager, {
        sem_manager.perform_undo_operations(pid);
    });
}
