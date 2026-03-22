use axerrno::{LinuxError, LinuxResult};

use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::{
    ctypes::{
        __kernel_clockid_t, CLOCK_MONOTONIC, CLOCK_MONOTONIC_RAW, CLOCK_PROCESS_CPUTIME_ID,
        CLOCK_REALTIME, CLOCK_THREAD_CPUTIME_ID, ITIMER_PROF, ITIMER_REAL, ITIMER_VIRTUAL,
        itimerval, sigevent, sys::Tms, timespec, timeval,
    },
    time::{TimeValueLike, monotonic_time, monotonic_time_nanos, nanos_to_ticks, wall_time},
};

use xcore::{
    task::{XProcess, XThread, with_thread, with_uspace},
    time::{clear_timer, set_timer, time_stat_output},
};

/// Get current time from a clock.
///
/// # Arguments
/// * `clock_id` - Clock identifier (CLOCK_REALTIME, CLOCK_MONOTONIC, etc.)
/// * `tp` - Buffer to store the current time
pub fn sys_clock_gettime(
    clock_id: __kernel_clockid_t,
    tp: UserPtr<timespec>,
) -> LinuxResult<isize> {
    let now = match clock_id as u32 {
        CLOCK_REALTIME => wall_time(),
        CLOCK_MONOTONIC | CLOCK_MONOTONIC_RAW => monotonic_time(),
        CLOCK_PROCESS_CPUTIME_ID => wall_time(),
        CLOCK_THREAD_CPUTIME_ID => wall_time(),
        _ => {
            warn!(
                "Called sys_clock_gettime for unsupported clock {}",
                clock_id
            );
            return Err(LinuxError::EINVAL);
        }
    };
    with_uspace(|uspace| uspace.write(tp, timespec::from_time_value(now)))?;
    trace!("sys_clock_gettime: {:?}", tp);
    Ok(0)
}

/// Set time for a clock.
///
/// # Arguments
/// * `_clock_id` - Clock identifier (currently unused)
/// * `_tp` - New time to set (currently unused)
pub fn sys_clock_settime(
    _clock_id: __kernel_clockid_t,
    _tp: UserConstPtr<timespec>,
) -> LinuxResult<isize> {
    warn!("sys_clock_settime not implemented");
    Ok(0)
}

/// Get clock resolution.
///
/// # Arguments
/// * `clock_id` - Clock identifier
/// * `res` - Buffer to store the clock resolution
pub fn sys_clock_getres(
    clock_id: __kernel_clockid_t,
    res: UserPtr<timespec>,
) -> LinuxResult<isize> {
    if clock_id < 0 {
        return Err(LinuxError::EINVAL);
    }
    with_uspace(|uspace| nullable!(uspace.write(res, timespec::from_nanos(1))))?;
    Ok(0)
}

/// Get current time of day.
///
/// # Arguments
/// * `ts` - Buffer to store the current time
pub fn sys_gettimeofday(ts: UserPtr<timeval>) -> LinuxResult<isize> {
    with_uspace(|uspace| uspace.write(ts, timeval::from_time_value(wall_time())))?;
    Ok(0)
}

/// Get process times.
///
/// # Arguments
/// * `tms` - Buffer to store process time information
pub fn sys_times(tms: UserPtr<Tms>) -> LinuxResult<isize> {
    let (_, _, utime_us, _, _, stime_us) = time_stat_output();
    with_uspace(|uspace| {
        uspace.write(
            tms,
            Tms {
                tms_utime: utime_us,
                tms_stime: stime_us,
                tms_cutime: utime_us,
                tms_cstime: stime_us,
            },
        )?;
        Ok(nanos_to_ticks(monotonic_time_nanos()) as _)
    })
}

/// Get interval timer value.
///
/// # Arguments
/// * `which` - Timer type (ITIMER_REAL, ITIMER_VIRTUAL, ITIMER_PROF)
/// * `value` - Buffer to store timer value
pub fn sys_getitimer(which: u32, value: UserPtr<itimerval>) -> LinuxResult<isize> {
    with_thread(|thread| {
        let uspace = XProcess::from_thread(thread).uspace();
        if let Some(value) = nullable!(uspace.raw_ptr(value))? {
            match which {
                ITIMER_REAL | ITIMER_VIRTUAL | ITIMER_PROF => {
                    let (_, interval_ns, remained_ns) =
                        XThread::from_thread(thread).time.read().stat_timer();
                    *value = itimerval {
                        it_interval: timeval::from_nanos(interval_ns as u64),
                        it_value: timeval::from_nanos(remained_ns as u64),
                    };
                    Ok(0)
                }
                _ => {
                    warn!("Called sys_getitimer for unsupported timer type {}", which);
                    Err(LinuxError::EINVAL)
                }
            }
        } else {
            Err(LinuxError::EFAULT)
        }
    })
}

/// Set interval timer value.
///
/// # Arguments
/// * `which` - Timer type (ITIMER_REAL, ITIMER_VIRTUAL, ITIMER_PROF)
/// * `new_value` - New timer value
/// * `old_value` - Buffer to store previous timer value (NULL if not needed)
pub fn sys_setitimer(
    which: u32,
    new_value: UserPtr<itimerval>,
    old_value: UserPtr<itimerval>,
) -> LinuxResult<isize> {
    if !old_value.is_null() {
        sys_getitimer(which, old_value)?;
    }

    with_thread(|thread| {
        let uspace = XProcess::from_thread(thread).uspace();
        if let Some(new_value) = nullable!(uspace.raw_ptr(new_value))? {
            match which {
                ITIMER_REAL | ITIMER_VIRTUAL | ITIMER_PROF => {
                    let interval_ns = new_value.it_interval.to_nanos();
                    let remained_ns = new_value.it_value.to_nanos();

                    if remained_ns == 0 {
                        clear_timer();
                    } else {
                        let timer_type = which as usize;
                        set_timer(interval_ns as usize, remained_ns as usize, timer_type);
                    }
                    Ok(0)
                }
                _ => {
                    warn!("Called sys_setitimer for unsupported timer type {}", which);
                    Err(LinuxError::EINVAL)
                }
            }
        } else {
            Err(LinuxError::EFAULT)
        }
    })
}

/// Create a per-process timer.
///
/// # Arguments
/// * `_clock_id` - Clock identifier (currently unused)
/// * `_sigev` - Signal event structure (currently unused)
/// * `_timer_id` - Buffer to store timer ID (currently unused)
pub fn sys_timer_create(
    _clock_id: __kernel_clockid_t,
    _sigev: UserPtr<sigevent>,
    _timer_id: UserPtr<u8>,
) -> LinuxResult<isize> {
    warn!("sys_timer_create not implemented");
    Ok(0)
}
