pub use axhal::time::{
    NANOS_PER_MICROS, NANOS_PER_MILLIS, NANOS_PER_SEC, TimeValue, monotonic_time,
    monotonic_time_nanos, nanos_to_ticks, wall_time, wall_time_nanos,
};

pub use crate::ctypes::{
    __kernel_old_timespec, __kernel_old_timeval, __kernel_sock_timeval, __kernel_timespec,
    timespec, timeval,
};

pub trait TimeValueLike {
    fn from_time_value(tv: TimeValue) -> Self;

    fn to_time_value(self) -> TimeValue;

    fn from_nanos(nanos: u64) -> Self;

    fn to_nanos(self) -> u64;
}

impl TimeValueLike for TimeValue {
    fn from_time_value(tv: TimeValue) -> Self {
        tv
    }

    fn to_time_value(self) -> TimeValue {
        self
    }

    fn from_nanos(nanos: u64) -> Self {
        TimeValue::new(nanos / NANOS_PER_SEC, (nanos % NANOS_PER_SEC) as u32)
    }

    fn to_nanos(self) -> u64 {
        self.as_secs() * NANOS_PER_SEC + self.subsec_nanos() as u64
    }
}

impl TimeValueLike for timespec {
    fn from_time_value(tv: TimeValue) -> Self {
        Self {
            tv_sec: tv.as_secs() as _,
            tv_nsec: tv.subsec_nanos() as _,
        }
    }

    fn to_time_value(self) -> TimeValue {
        TimeValue::new(self.tv_sec as u64, self.tv_nsec as u32)
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

impl TimeValueLike for __kernel_timespec {
    fn from_time_value(tv: TimeValue) -> Self {
        Self {
            tv_sec: tv.as_secs() as _,
            tv_nsec: tv.subsec_nanos() as _,
        }
    }

    fn to_time_value(self) -> TimeValue {
        TimeValue::new(self.tv_sec as u64, self.tv_nsec as u32)
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

impl TimeValueLike for __kernel_old_timespec {
    fn from_time_value(tv: TimeValue) -> Self {
        Self {
            tv_sec: tv.as_secs() as _,
            tv_nsec: tv.subsec_nanos() as _,
        }
    }

    fn to_time_value(self) -> TimeValue {
        TimeValue::new(self.tv_sec as u64, self.tv_nsec as u32)
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

impl TimeValueLike for timeval {
    fn from_time_value(tv: TimeValue) -> Self {
        Self {
            tv_sec: tv.as_secs() as _,
            tv_usec: tv.subsec_micros() as _,
        }
    }

    fn to_time_value(self) -> TimeValue {
        TimeValue::new(self.tv_sec as u64, self.tv_usec as u32 * 1000)
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

impl TimeValueLike for __kernel_old_timeval {
    fn from_time_value(tv: TimeValue) -> Self {
        Self {
            tv_sec: tv.as_secs() as _,
            tv_usec: tv.subsec_micros() as _,
        }
    }

    fn to_time_value(self) -> TimeValue {
        TimeValue::new(self.tv_sec as u64, self.tv_usec as u32 * 1000)
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

impl TimeValueLike for __kernel_sock_timeval {
    fn from_time_value(tv: TimeValue) -> Self {
        Self {
            tv_sec: tv.as_secs() as _,
            tv_usec: tv.subsec_micros() as _,
        }
    }

    fn to_time_value(self) -> TimeValue {
        TimeValue::new(self.tv_sec as u64, self.tv_usec as u32 * 1000)
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
