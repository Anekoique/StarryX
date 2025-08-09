use axerrno::{LinuxError, LinuxResult};
pub use axhal::time::{
    NANOS_PER_MICROS, NANOS_PER_MILLIS, NANOS_PER_SEC, TimeValue, monotonic_time,
    monotonic_time_nanos, nanos_to_ticks, wall_time, wall_time_nanos,
};

pub use crate::ctypes::{
    __kernel_old_timespec, __kernel_old_timeval, __kernel_sock_timeval, __kernel_timespec,
    timespec, timeval,
};

/// A helper trait for converting from and to `TimeValue`.
pub trait TimeValueLike {
    /// Converts from `TimeValue`.
    fn from_time_value(tv: TimeValue) -> Self;

    /// Tries to convert into `TimeValue`.
    fn to_time_value(self) -> LinuxResult<TimeValue>;

    /// Converts from nanoseconds.
    fn from_nanos(nanos: u64) -> Self;

    /// Converts to nanoseconds.
    fn to_nanos(self) -> u64;
}

/// Macro to implement TimeValueLike for types with tv_sec and tv_nsec fields (nanosecond precision)
macro_rules! impl_timevaluelike_for_timespec {
    ($($type:ty),+ $(,)?) => {
        $(
            impl TimeValueLike for $type {
                fn from_time_value(tv: TimeValue) -> Self {
                    Self {
                        tv_sec: tv.as_secs() as _,
                        tv_nsec: tv.subsec_nanos() as _,
                    }
                }

                fn to_time_value(self) -> LinuxResult<TimeValue> {
                    if self.tv_nsec < 0 || self.tv_nsec > 999_999_999 || self.tv_sec < 0 {
                        return Err(LinuxError::EINVAL);
                    }
                    Ok(TimeValue::new(self.tv_sec as u64, self.tv_nsec as u32))
                }

                fn from_nanos(nanos: u64) -> Self {
                    Self {
                        tv_sec: (nanos / NANOS_PER_SEC) as _,
                        tv_nsec: (nanos % NANOS_PER_SEC) as _,
                    }
                }

                fn to_nanos(self) -> u64 {
                    (self.tv_sec as u64) * NANOS_PER_SEC + (self.tv_nsec as u64)
                }
            }
        )+
    };
}

/// Macro to implement TimeValueLike for types with tv_sec and tv_usec fields (microsecond precision)
macro_rules! impl_timevaluelike_for_timeval {
    ($($type:ty),+ $(,)?) => {
        $(
            impl TimeValueLike for $type {
                fn from_time_value(tv: TimeValue) -> Self {
                    Self {
                        tv_sec: tv.as_secs() as _,
                        tv_usec: tv.subsec_micros() as _,
                    }
                }

                fn to_time_value(self) -> LinuxResult<TimeValue> {
                    if self.tv_usec < 0 || self.tv_usec > 999_999 || self.tv_sec < 0 {
                        return Err(LinuxError::EINVAL);
                    }
                    Ok(TimeValue::new(
                        self.tv_sec as u64,
                        self.tv_usec as u32 * 1000,
                    ))
                }

                fn from_nanos(nanos: u64) -> Self {
                    Self {
                        tv_sec: (nanos / NANOS_PER_SEC) as _,
                        tv_usec: ((nanos % NANOS_PER_SEC) / NANOS_PER_MICROS) as _,
                    }
                }

                fn to_nanos(self) -> u64 {
                    (self.tv_sec as u64) * NANOS_PER_SEC + (self.tv_usec as u64) * NANOS_PER_MICROS
                }
            }
        )+
    };
}

// Special implementation for TimeValue (no conversion needed)
impl TimeValueLike for TimeValue {
    fn from_time_value(tv: TimeValue) -> Self {
        tv
    }

    fn to_time_value(self) -> LinuxResult<TimeValue> {
        Ok(self)
    }

    fn from_nanos(nanos: u64) -> Self {
        TimeValue::new(nanos / NANOS_PER_SEC, (nanos % NANOS_PER_SEC) as u32)
    }

    fn to_nanos(self) -> u64 {
        self.as_secs() * NANOS_PER_SEC + self.subsec_nanos() as u64
    }
}

// Use macros to implement TimeValueLike for all timespec-like types
impl_timevaluelike_for_timespec! {
    timespec,
    __kernel_timespec,
    __kernel_old_timespec,
}

// Use macros to implement TimeValueLike for all timeval-like types
impl_timevaluelike_for_timeval! {
    timeval,
    __kernel_old_timeval,
    __kernel_sock_timeval,
}
