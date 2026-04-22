use alloc::sync::Arc;
use core::mem;

use axerrno::{LinuxError, LinuxResult};
use axhal::arch::TrapFrame;

use xprocess::{Pid, Thread};
use xsignal::{SignalInfo, SignalSet, SignalStack, Signo};
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::{
    ctypes::{
        MINSIGSTKSZ, SI_TKILL, SI_USER, SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK, kernel_sigaction,
        siginfo, timespec,
    },
    time::TimeValueLike,
};

use xcore::task::*;

use crate::task::check_signals;

fn check_sigset_size(size: usize) -> LinuxResult<()> {
    if size != size_of::<SignalSet>() {
        return Err(LinuxError::EINVAL);
    }
    Ok(())
}

pub fn parse_signo(signo: u32) -> LinuxResult<Signo> {
    Signo::from_repr(signo as u8).ok_or(LinuxError::EINVAL)
}

/// Examine and change blocked signals.
///
/// # Arguments
/// * `how` - How to change the signal mask (SIG_BLOCK, SIG_UNBLOCK, SIG_SETMASK)
/// * `set` - New signal set (NULL to only query current mask)
/// * `oldset` - Buffer to store previous signal mask (NULL if not needed)
/// * `sigsetsize` - Size of the signal set
pub fn sys_rt_sigprocmask(
    how: i32,
    set: UserConstPtr<SignalSet>,
    oldset: UserPtr<SignalSet>,
    sigsetsize: usize,
) -> LinuxResult<isize> {
    check_sigset_size(sigsetsize)?;

    with_thread(|thread| {
        let uspace = XProcess::from_thread(thread).uspace();
        let xthread = XThread::from_thread(thread);
        xthread
            .signal
            .with_blocked_mut::<LinuxResult<_>>(|blocked| {
                nullable!(uspace.write(oldset, *blocked))?;
                if let Some(set) = nullable!(uspace.read(set))? {
                    match how as u32 {
                        SIG_BLOCK => *blocked |= set,
                        SIG_UNBLOCK => *blocked &= !set,
                        SIG_SETMASK => *blocked = set,
                        _ => return Err(LinuxError::EINVAL),
                    }
                }
                Ok(())
            })?;
        Ok(0)
    })
}

/// Examine and change signal action.
///
/// # Arguments
/// * `signo` - Signal number
/// * `act` - New signal action (NULL to only query current action)
/// * `oldact` - Buffer to store previous signal action (NULL if not needed)
/// * `sigsetsize` - Size of the signal set
pub fn sys_rt_sigaction(
    signo: u32,
    act: UserConstPtr<kernel_sigaction>,
    oldact: UserPtr<kernel_sigaction>,
    sigsetsize: usize,
) -> LinuxResult<isize> {
    check_sigset_size(sigsetsize)?;

    with_xprocess(|xprocess| {
        let uspace = xprocess.uspace();
        let signo = parse_signo(signo)?;
        if matches!(signo, Signo::SIGKILL | Signo::SIGSTOP) {
            return Err(LinuxError::EINVAL);
        }
        debug!("sys_rt_sigaction <= signo: {:?}", signo);

        let mut actions = xprocess.signal.actions.lock();
        if let Some(oldact) = nullable!(uspace.raw_ptr(oldact))? {
            actions[signo].to_ctype(oldact);
        }
        if let Some(act) = nullable!(uspace.read(act))? {
            actions[signo] = act.try_into()?;
        }
        Ok(0)
    })
}

/// Examine pending signals.
///
/// # Arguments
/// * `set` - Buffer to store pending signal set
/// * `sigsetsize` - Size of the signal set
pub fn sys_rt_sigpending(set: UserPtr<SignalSet>, sigsetsize: usize) -> LinuxResult<isize> {
    check_sigset_size(sigsetsize)?;
    with_thread(|thread| {
        let uspace = XProcess::from_thread(thread).uspace();
        let xthread = XThread::from_thread(thread);
        uspace.write(set, xthread.signal.pending())?;
        Ok(0)
    })
}

fn make_siginfo(signo: u32, code: i32) -> LinuxResult<Option<SignalInfo>> {
    if signo == 0 {
        return Ok(None);
    }
    let signo = parse_signo(signo)?;
    Ok(Some(SignalInfo::new(signo, code)))
}

/// Send signal to a process or process group.
///
/// # Arguments
/// * `pid` - Process ID or process group ID
/// * `signo` - Signal number to send
pub fn sys_kill(pid: i32, signo: u32) -> LinuxResult<isize> {
    let Some(sig) = make_siginfo(signo, SI_USER as _)? else {
        // TODO: should also check permissions
        return Ok(0);
    };

    match pid {
        1.. => {
            let proc = get_process(pid as Pid)?;
            send_signal_process(&proc, sig)?;
            Ok(0)
        }
        0 => with_process(|process| {
            let pg = process.group();
            Ok(send_signal_process_group(&pg, sig) as _)
        }),
        -1 => {
            let mut count = 0;
            for proc in processes() {
                if proc.is_init() {
                    // init process
                    continue;
                }
                send_signal_process(&proc, sig.clone())?;
                count += 1;
            }
            Ok(count)
        }
        ..-1 => {
            let pg = get_process_group((-pid) as Pid)?;
            Ok(send_signal_process_group(&pg, sig) as _)
        }
    }
}

/// Send signal to a specific thread.
///
/// # Arguments
/// * `tid` - Thread ID
/// * `signo` - Signal number to send
pub fn sys_tkill(tid: Pid, signo: u32) -> LinuxResult<isize> {
    let Some(sig) = make_siginfo(signo, SI_TKILL)? else {
        // TODO: should also check permissions
        return Ok(0);
    };

    let thr = get_thread(tid)?;
    send_signal_thread(&thr, sig)?;
    Ok(0)
}

/// Send signal to a specific thread in a specific thread group.
///
/// # Arguments
/// * `tgid` - Thread group ID (process ID)
/// * `tid` - Thread ID
/// * `signo` - Signal number to send
pub fn sys_tgkill(tgid: Pid, tid: Pid, signo: u32) -> LinuxResult<isize> {
    let Some(sig) = make_siginfo(signo, SI_TKILL)? else {
        // TODO: should also check permissions
        return Ok(0);
    };

    send_signal_thread(find_thread_in_group(tgid, tid)?.as_ref(), sig)?;
    Ok(0)
}

fn find_thread_in_group(tgid: Pid, tid: Pid) -> LinuxResult<Arc<Thread>> {
    let thr = get_thread(tid)?;
    if thr.process().pid() != tgid {
        return Err(LinuxError::ESRCH);
    }
    Ok(thr)
}

pub fn make_queue_signal_info(
    tgid: Pid,
    signo: u32,
    info: UserPtr<SignalInfo>,
) -> LinuxResult<SignalInfo> {
    with_thread(|thread| {
        let signo = parse_signo(signo)?;
        let uspace = XProcess::from_thread(thread).uspace();
        let mut info = uspace.raw_ptr(info)?.clone();
        info.set_signo(signo);
        if thread.process().pid() != tgid && (info.code() >= 0 || info.code() == SI_TKILL) {
            return Err(LinuxError::EPERM);
        }
        Ok(info)
    })
}

/// Queue a signal with additional data to a process.
///
/// # Arguments
/// * `tgid` - Target process ID
/// * `signo` - Signal number to queue
/// * `sig` - Signal information structure
/// * `sigsetsize` - Size of the signal set
pub fn sys_rt_sigqueueinfo(
    pid: Pid,
    signo: u32,
    info: UserPtr<SignalInfo>,
    sigsetsize: usize,
) -> LinuxResult<isize> {
    check_sigset_size(sigsetsize)?;

    let info = make_queue_signal_info(pid, signo, info)?;
    send_signal_process(get_process(pid)?.as_ref(), info)?;
    Ok(0)
}

/// Queue a signal with additional data to a specific thread.
///
/// # Arguments
/// * `tgid` - Thread group ID (process ID)
/// * `tid` - Target thread ID
/// * `signo` - Signal number to queue
/// * `sig` - Signal information structure
/// * `sigsetsize` - Size of the signal set
pub fn sys_rt_tgsigqueueinfo(
    tgid: Pid,
    tid: Pid,
    signo: u32,
    info: UserPtr<SignalInfo>,
    sigsetsize: usize,
) -> LinuxResult<isize> {
    check_sigset_size(sigsetsize)?;

    let info = make_queue_signal_info(tgid, signo, info)?;
    send_signal_thread(find_thread_in_group(tgid, tid)?.as_ref(), info)?;
    Ok(0)
}

/// Return from signal handler and restore context.
///
/// # Arguments
/// * `tf` - Trap frame to restore
pub fn sys_rt_sigreturn(tf: &mut TrapFrame) -> LinuxResult<isize> {
    with_thread(|thread| {
        XThread::from_thread(thread).signal.restore(tf);
        Ok(tf.retval() as isize)
    })
}

/// Wait for queued signals with timeout.
///
/// # Arguments
/// * `set` - Set of signals to wait for
/// * `info` - Buffer to store signal information (NULL if not needed)
/// * `timeout` - Timeout specification (NULL for infinite)
/// * `sigsetsize` - Size of the signal set
pub fn sys_rt_sigtimedwait(
    set: UserConstPtr<SignalSet>,
    info: UserPtr<siginfo>,
    timeout: UserConstPtr<timespec>,
    sigsetsize: usize,
) -> LinuxResult<isize> {
    check_sigset_size(sigsetsize)?;

    with_thread(|thread| {
        let xthread = XThread::from_thread(thread);
        let uspace = XProcess::from_thread(thread).uspace();
        let set = uspace.read(set)?;
        let timeout = nullable!(uspace.read(timeout))?
            .map(timespec::to_time_value)
            .transpose()?;

        let Some(sig) = xthread.signal.wait_timeout(set, timeout) else {
            return Err(LinuxError::EAGAIN);
        };

        nullable!(uspace.write(info, sig.0))?;
        Ok(0)
    })
}

/// Temporarily replace signal mask and suspend until signal.
///
/// # Arguments
/// * `tf` - Trap frame for signal handling
/// * `set` - Temporary signal mask
/// * `sigsetsize` - Size of the signal set
pub fn sys_rt_sigsuspend(
    tf: &mut TrapFrame,
    set: UserConstPtr<SignalSet>,
    sigsetsize: usize,
) -> LinuxResult<isize> {
    check_sigset_size(sigsetsize)?;

    with_xtask(|xtask| {
        let xprocess = xtask.xprocess_ref();
        let uspace = xprocess.uspace();
        let mut set = uspace.read(set)?;
        set.remove(Signo::SIGKILL);
        set.remove(Signo::SIGSTOP);
        let old_blocked = xtask
            .xthread_ref()
            .signal
            .with_blocked_mut(|blocked| mem::replace(blocked, set));

        loop {
            if check_signals(tf, Some(old_blocked)) {
                break;
            }
            xprocess.signal.wait_signal();
        }
        Err(LinuxError::EINTR)
    })
}

/// Set and/or get signal stack context.
///
/// # Arguments
/// * `ss` - New signal stack (NULL to only query current stack)
/// * `old_ss` - Buffer to store current signal stack (NULL if not needed)
pub fn sys_sigaltstack(
    ss: UserConstPtr<SignalStack>,
    old_ss: UserPtr<SignalStack>,
) -> LinuxResult<isize> {
    with_thread(|thread| {
        XThread::from_thread(thread).signal.with_stack_mut(|stack| {
            let uspace = XProcess::from_thread(thread).uspace();
            nullable!(uspace.write(old_ss, *stack))?;
            if let Some(ss) = nullable!(uspace.read(ss))? {
                if ss.size <= MINSIGSTKSZ as usize {
                    return Err(LinuxError::ENOMEM);
                }
                let stack_ptr: UserConstPtr<u8> = ss.sp.into();
                let _ = uspace.read_slice(stack_ptr, ss.size)?;

                *stack = ss;
            }
            Ok(0)
        })
    })
}
