use axerrno::{LinuxError, LinuxResult};
use starry_core::task::{
    time_stat_clear_timer, time_stat_get_timer, time_stat_output, time_stat_set_timer,
};

use crate::{
    ctypes::{
        __kernel_clockid_t, CLOCK_MONOTONIC, CLOCK_REALTIME, ITIMER_PROF, ITIMER_REAL,
        ITIMER_VIRTUAL, itimerval, timespec, timeval,
    },
    ptr::{UserPtr, nullable},
    time::{TimeValueLike, monotonic_time, monotonic_time_nanos, nanos_to_ticks, wall_time},
};

pub fn sys_clock_gettime(
    clock_id: __kernel_clockid_t,
    ts: UserPtr<timespec>,
) -> LinuxResult<isize> {
    let now = match clock_id as u32 {
        CLOCK_REALTIME => wall_time(),
        CLOCK_MONOTONIC => monotonic_time(),
        _ => {
            warn!(
                "Called sys_clock_gettime for unsupported clock {}",
                clock_id
            );
            return Err(LinuxError::EINVAL);
        }
    };
    *ts.get_as_mut()? = timespec::from_time_value(now);
    debug!("sys_clock_gettime: {:?}", ts.get_as_mut()?);
    Ok(0)
}

pub fn sys_gettimeofday(ts: UserPtr<timeval>) -> LinuxResult<isize> {
    *ts.get_as_mut()? = timeval::from_time_value(wall_time());
    Ok(0)
}

#[repr(C)]
pub struct Tms {
    /// Process user mode execution time in microseconds
    tms_utime: usize,
    /// Process kernel mode execution time in microseconds
    tms_stime: usize,
    /// Sum of child processes' user mode execution time in microseconds
    tms_cutime: usize,
    /// Sum of child processes' kernel mode execution time in microseconds
    tms_cstime: usize,
}

pub fn sys_times(tms: UserPtr<Tms>) -> LinuxResult<isize> {
    let (_, _, utime_us, _, _, stime_us) = time_stat_output();
    *tms.get_as_mut()? = Tms {
        tms_utime: utime_us,
        tms_stime: stime_us,
        tms_cutime: utime_us,
        tms_cstime: stime_us,
    };
    Ok(nanos_to_ticks(monotonic_time_nanos()) as _)
}

/// Get interval timer value
///
/// POSIX specification: getitimer() gets the current value of the timer specified by `which`
/// and stores it in the structure pointed to by `value`.
pub fn sys_getitimer(which: u32, value: UserPtr<itimerval>) -> LinuxResult<isize> {
    if let Some(value) = nullable!(value.get_as_mut())? {
        match which {
            ITIMER_REAL | ITIMER_VIRTUAL | ITIMER_PROF => {
                let (_timer_type, interval_ns, remained_ns) = time_stat_get_timer();
                *value = itimerval {
                    it_interval: timeval::from_nanos(interval_ns as u64),
                    it_value: timeval::from_nanos(remained_ns as u64),
                };
                debug!("getitimer: {:?}", value);
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
}

/// Set interval timer value
///
/// POSIX specification: setitimer() sets the timer specified by `which` to the value in `new_value`.
/// If `old_value` is not NULL, the old value of the timer is stored there.
pub fn sys_setitimer(
    which: u32,
    new_value: UserPtr<itimerval>,
    old_value: UserPtr<itimerval>,
) -> LinuxResult<isize> {
    if !old_value.is_null() {
        sys_getitimer(which, old_value)?;
    }

    if let Some(new_value) = nullable!(new_value.get_as_mut())? {
        match which {
            ITIMER_REAL | ITIMER_VIRTUAL | ITIMER_PROF => {
                let interval_ns = new_value.it_interval.to_nanos();
                let remained_ns = new_value.it_value.to_nanos();

                if remained_ns == 0 {
                    time_stat_clear_timer();
                } else {
                    let timer_type = which as usize;
                    time_stat_set_timer(interval_ns as usize, remained_ns as usize, timer_type);
                }
                debug!("timer_type: {:?}", which);
                debug!("interval_ns: {}, remained_ns: {}", interval_ns, remained_ns);
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
}
