use core::sync::atomic::Ordering;

use axerrno::{LinuxError, LinuxResult};
use axtask::current;
use axuspace::{UserConstPtr, UserPtr, UserSpace, nullable};
use starry_core::task::{TaskExt, ThreadData, get_thread};

use crate::{
    ctypes::{
        FUTEX_CMD_MASK, FUTEX_CMP_REQUEUE, FUTEX_REQUEUE, FUTEX_WAIT, FUTEX_WAKE,
        ROBUST_LIST_LIMIT, robust_list, robust_list_head, timespec,
    },
    utils::time::TimeValueLike,
};

/// Fast user-space locking system call.
///
/// # Arguments
/// * `uaddr` - Address of the futex variable
/// * `futex_op` - Operation to perform (FUTEX_WAIT, FUTEX_WAKE, etc.)
/// * `value` - Expected value for FUTEX_WAIT or wake count for FUTEX_WAKE
/// * `timeout` - Timeout for FUTEX_WAIT (NULL for infinite)
/// * `uaddr2` - Second futex address for FUTEX_REQUEUE operations
/// * `value3` - Additional value for some operations
pub fn sys_futex(
    uaddr: UserConstPtr<u32>,
    futex_op: u32,
    value: u32,
    timeout: UserConstPtr<timespec>,
    uaddr2: UserPtr<u32>,
    value3: u32,
) -> LinuxResult<isize> {
    info!("futex {:?} {} {}", uaddr.address(), futex_op, value);

    let uspace = UserSpace::new(TaskExt::from_task(&current()).process_data());
    let futex_table = &TaskExt::from_task(&current()).process_data().futex_table;

    let addr = uaddr.address().as_usize();
    let command = futex_op & (FUTEX_CMD_MASK as u32);
    match command {
        FUTEX_WAIT => {
            if uspace.read(uaddr)? != value {
                return Err(LinuxError::EAGAIN);
            }
            let futex = futex_table.get_or_insert(addr);

            if let Some(timeout) = nullable!(uspace.read(timeout))? {
                futex.wq.wait_timeout(timespec::to_time_value(timeout));
            } else {
                futex.wq.wait();
            }
            if futex.owner_dead.swap(false, Ordering::SeqCst) {
                Err(LinuxError::EOWNERDEAD)
            } else {
                Ok(0)
            }
        }
        FUTEX_WAKE => {
            let futex = futex_table.get(addr);
            let mut count = 0;
            if let Some(futex) = futex {
                for _ in 0..value {
                    if !futex.wq.notify_one(false) {
                        break;
                    }
                    count += 1;
                }
            }
            axtask::yield_now();
            Ok(count)
        }
        FUTEX_REQUEUE | FUTEX_CMP_REQUEUE => {
            if command == FUTEX_CMP_REQUEUE && uspace.read(uaddr)? != value3 {
                return Err(LinuxError::EAGAIN);
            }
            let value2 = timeout.address().as_usize() as u32;

            let futex = futex_table.get(addr);
            let futex2 = futex_table.get_or_insert(uaddr2.address().as_usize());

            let mut count = 0;
            if let Some(futex) = futex {
                for _ in 0..value {
                    if !futex.wq.notify_one(false) {
                        break;
                    }
                    count += 1;
                }
                if count == value as isize {
                    count += futex.wq.requeue(value2 as usize, &futex2.wq) as isize;
                }
            }
            Ok(count)
        }
        _ => Err(LinuxError::ENOSYS),
    }
}

/// Get robust futex list head for a thread.
///
/// # Arguments
/// * `tid` - Thread ID (0 for calling thread)
/// * `head` - Buffer to store robust list head pointer
/// * `size` - Buffer to store robust list head size
pub fn sys_get_robust_list(
    tid: u32,
    head: UserPtr<UserConstPtr<robust_list_head>>,
    size: UserPtr<usize>,
) -> LinuxResult<isize> {
    let thr = if tid == 0 {
        TaskExt::from_task(&current()).thread.clone()
    } else {
        get_thread(tid)?
    };
    let uspace = UserSpace::new(TaskExt::from_task(&current()).process_data());
    uspace.write(
        head,
        thr.data::<ThreadData>()
            .unwrap()
            .robust_list_head
            .load(Ordering::SeqCst)
            .into(),
    )?;
    uspace.write(size, size_of::<robust_list_head>())?;

    Ok(0)
}

/// Set robust futex list head for the calling thread.
///
/// # Arguments
/// * `head` - Robust list head pointer
/// * `size` - Size of the robust list head structure
pub fn sys_set_robust_list(
    head: UserConstPtr<robust_list_head>,
    size: usize,
) -> LinuxResult<isize> {
    if size != size_of::<robust_list_head>() {
        return Err(LinuxError::EINVAL);
    }
    TaskExt::from_task(&current())
        .thread_data()
        .robust_list_head
        .store(head.address().as_usize(), Ordering::SeqCst);

    Ok(0)
}

fn handle_futex_death(entry: *mut robust_list, offset: i64) -> LinuxResult<()> {
    let address = (entry as u64)
        .checked_add_signed(offset)
        .ok_or(LinuxError::EINVAL)?;
    let address: usize = address.try_into().map_err(|_| LinuxError::EINVAL)?;

    let futex_table = &TaskExt::from_task(&current()).process_data().futex_table;

    let Some(futex) = futex_table.get(address) else {
        return Ok(());
    };
    futex.owner_dead.store(true, Ordering::SeqCst);
    futex.wq.notify_one(false);
    Ok(())
}

pub fn exit_robust_list(head: &mut robust_list_head) -> LinuxResult<()> {
    let mut limit = ROBUST_LIST_LIMIT;
    let uspace = UserSpace::new(TaskExt::from_task(&current()).process_data());

    let mut entry = head.list.next;
    let offset = head.futex_offset;
    let pending = head.list_op_pending;

    while entry != &mut head.list as *mut _ {
        let entry_ptr = UserPtr::from(entry);
        let next_entry = uspace.read(entry_ptr)?.next;
        if entry != pending {
            handle_futex_death(entry, offset)?;
        }
        entry = next_entry;

        limit -= 1;
        if limit == 0 {
            return Err(LinuxError::ELOOP);
        }
        axtask::yield_now();
    }

    Ok(())
}
