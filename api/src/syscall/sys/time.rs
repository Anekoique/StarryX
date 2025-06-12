use axerrno::{LinuxError, LinuxResult};
use starry_core::task::time_stat_output;

use crate::{
    ctypes::{__kernel_clockid_t, CLOCK_MONOTONIC, CLOCK_REALTIME, timeval},
    ptr::UserPtr,
    time::*,
};

const CLOCK_PROCESS_CPUTIME_ID: u32 = 2;

pub fn sys_clock_gettime(
    clock_id: __kernel_clockid_t,
    ts: UserPtr<timespec>,
) -> LinuxResult<isize> {
    let now = match clock_id as u32 {
        CLOCK_REALTIME => wall_time(),
        CLOCK_MONOTONIC => monotonic_time(),
        CLOCK_PROCESS_CPUTIME_ID => {
            // 进程 CPU 时间 - 对于基本实现，我们使用单调时间
            // 在完整实现中，这应该是进程的累计 CPU 使用时间
            monotonic_time()
        }
        _ => {
            warn!(
                "Called sys_clock_gettime for unsupported clock {}",
                clock_id
            );
            return Err(LinuxError::EINVAL);
        }
    };
    *ts.get_as_mut()? = timevalue_to_timespec(now);
    Ok(0)
}

pub fn sys_gettimeofday(ts: UserPtr<timeval>) -> LinuxResult<isize> {
    *ts.get_as_mut()? = timevalue_to_timeval(wall_time());
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
    let (_, utime_us, _, stime_us) = time_stat_output();
    *tms.get_as_mut()? = Tms {
        tms_utime: utime_us,
        tms_stime: stime_us,
        tms_cutime: utime_us,
        tms_cstime: stime_us,
    };
    Ok(nanos_to_ticks(monotonic_time_nanos()) as _)
}
