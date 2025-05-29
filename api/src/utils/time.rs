pub use axhal::time::{
    TimeValue, monotonic_time, monotonic_time_nanos, nanos_to_ticks, wall_time, wall_time_nanos,
};

pub use crate::ctypes::{__kernel_old_timeval, timespec, timeval};

pub fn timevalue_to_timespec(tv: TimeValue) -> timespec {
    timespec {
        tv_sec: tv.as_secs() as _,
        tv_nsec: tv.subsec_nanos() as _,
    }
}

pub fn timespec_to_timevalue(ts: timespec) -> TimeValue {
    TimeValue::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

pub fn timevalue_to_timeval(tv: TimeValue) -> timeval {
    timeval {
        tv_sec: tv.as_secs() as _,
        tv_usec: tv.subsec_micros() as _,
    }
}

pub fn timeval_to_timevalue(tv: timeval) -> TimeValue {
    TimeValue::new(tv.tv_sec as u64, tv.tv_usec as u32 * 1_000)
}

pub fn old_timeval_to_timevalue(tv: __kernel_old_timeval) -> TimeValue {
    TimeValue::new(tv.tv_sec as u64, tv.tv_usec as u32 * 1_000)
}

pub fn timevalue_to_old_timeval(tv: TimeValue) -> __kernel_old_timeval {
    __kernel_old_timeval {
        tv_sec: tv.as_secs() as _,
        tv_usec: tv.subsec_micros() as _,
    }
}

/// Helper function to convert nanoseconds to timeval
pub fn timeval_from_nanos(nanos: u64) -> timeval {
    timeval {
        tv_sec: (nanos / 1_000_000_000) as i64,
        tv_usec: ((nanos % 1_000_000_000) / 1_000) as i64,
    }
}

/// Helper function to convert timeval to nanoseconds
pub fn timeval_to_nanos(tv: &timeval) -> u64 {
    (tv.tv_sec as u64) * 1_000_000_000 + (tv.tv_usec as u64) * 1_000
}
