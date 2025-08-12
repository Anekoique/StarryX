// ref starry-mix
use core::sync::atomic::Ordering;

use axerrno::{LinuxError, LinuxResult};
use axtask::{TaskExtRef, current};

use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::{
    ctypes::{
        FUTEX_CMD_MASK, FUTEX_CMP_REQUEUE, FUTEX_REQUEUE, FUTEX_WAIT, FUTEX_WAIT_BITSET,
        FUTEX_WAKE, FUTEX_WAKE_BITSET, ROBUST_LIST_LIMIT, robust_list, robust_list_head, timespec,
    },
    time::TimeValueLike,
};

use xcore::task::{FutexKey, XProcess, XThread, get_thread, with_uspace, with_xthread};

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
    debug!("futex {:?} {} {}", uaddr.address(), futex_op, value);

    let xprocess = current().task_ext().xprocess();
    let uspace = xprocess.uspace();
    let futex_table = &xprocess.futex_table;

    let key = FutexKey::new(uaddr.address().as_usize());
    let command = futex_op & (FUTEX_CMD_MASK as u32);
    match command {
        FUTEX_WAIT | FUTEX_WAIT_BITSET => {
            let uaddr_val = uspace.read(uaddr)?;
            if uaddr_val != value {
                return Err(LinuxError::EAGAIN);
            }
            let timeout = nullable!(uspace.read(timeout))?
                .map(timespec::to_time_value)
                .transpose()?;
            let mut first_call = true;
            let mut mismatches = false;
            let condition = || {
                if first_call {
                    mismatches = uaddr_val != value;
                    first_call = false;
                    mismatches
                } else {
                    true
                }
            };

            let futex = futex_table.get_or_insert(&key);

            if command == FUTEX_WAIT_BITSET {
                with_xthread(|xthread| {
                    xthread.futex_bitset.store(value3, Ordering::SeqCst);
                });
            }

            if let Some(timeout) = timeout {
                if futex.wq.wait_timeout_until(timeout, condition) {
                    return Err(LinuxError::ETIMEDOUT);
                }
            } else {
                futex.wq.wait_until(condition);
            }
            if mismatches {
                return Err(LinuxError::EAGAIN);
            }

            if futex.owner_dead.swap(false, Ordering::SeqCst) {
                Err(LinuxError::EOWNERDEAD)
            } else {
                Ok(0)
            }
        }
        FUTEX_WAKE | FUTEX_WAKE_BITSET => {
            let futex = futex_table.get(&key);
            let mut count = 0;
            if let Some(futex) = futex {
                futex.wq.notify_all_if(false, |_| {
                    if count >= value {
                        false
                    } else {
                        let wake = if command == FUTEX_WAKE_BITSET {
                            let bitset =
                                with_xthread(|xthread| xthread.futex_bitset.load(Ordering::SeqCst));
                            (bitset & value3) != 0
                        } else {
                            true
                        };
                        count += wake as u32;
                        wake
                    }
                });
            }
            axtask::yield_now();
            Ok(count as isize)
        }
        FUTEX_REQUEUE | FUTEX_CMP_REQUEUE => {
            if (value as i32) < 0 {
                return Err(LinuxError::EINVAL);
            }

            if command == FUTEX_CMP_REQUEUE && uspace.read(uaddr)? != value3 {
                return Err(LinuxError::EAGAIN);
            }
            let value2 = timeout.address().as_usize() as u32;
            if (value2 as i32) < 0 {
                return Err(LinuxError::EINVAL);
            }

            let futex = futex_table.get(&key);
            let key2 = FutexKey::new(uaddr2.address().as_usize());
            let futex2 = futex_table.get_or_insert(&key2);

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
    let thr = get_thread(tid)?;
    let xthread = XThread::from_thread(&thr);
    let uspace = XProcess::from_thread(&thr).uspace();
    uspace.write(head, xthread.robust_list_head.load(Ordering::SeqCst).into())?;
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
    with_xthread(|xthread| {
        xthread
            .robust_list_head
            .store(head.address().as_usize(), Ordering::SeqCst);
    });

    Ok(0)
}

fn handle_futex_death(entry: *mut robust_list, offset: i64) -> LinuxResult<()> {
    let address = (entry as u64)
        .checked_add_signed(offset)
        .ok_or(LinuxError::EINVAL)?;
    let address: usize = address.try_into().map_err(|_| LinuxError::EINVAL)?;

    let futex_table = &current().task_ext().xprocess().futex_table;

    let Some(futex) = futex_table.get(&FutexKey::new(address)) else {
        return Ok(());
    };
    futex.owner_dead.store(true, Ordering::SeqCst);
    futex.wq.notify_one(false);
    Ok(())
}

pub fn exit_robust_list(head: &mut robust_list_head) -> LinuxResult<()> {
    let mut limit = ROBUST_LIST_LIMIT;

    let mut entry = head.list.next;
    let offset = head.futex_offset;
    let pending = head.list_op_pending;

    with_uspace(|uspace| {
        while !core::ptr::eq(entry, &head.list) {
            let next_entry = uspace.read(UserPtr::from(entry))?.next;
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
    })
}
