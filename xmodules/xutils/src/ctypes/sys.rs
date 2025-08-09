use super::{__kernel_old_time_t, c_int, timespec};

#[repr(C)]
#[allow(non_camel_case_types, dead_code)]
pub struct rtc_time {
    pub tm_sec: c_int,
    pub tm_min: c_int,
    pub tm_hour: c_int,
    pub tm_mday: c_int,
    pub tm_mon: c_int,
    pub tm_year: c_int,
    pub tm_wday: c_int,
    pub tm_yday: c_int,
    pub tm_isdst: c_int,
}

#[repr(C)]
pub struct Tms {
    /// Process user mode execution time in microseconds
    pub tms_utime: usize,
    /// Process kernel mode execution time in microseconds
    pub tms_stime: usize,
    /// Sum of child processes' user mode execution time in microseconds
    pub tms_cutime: usize,
    /// Sum of child processes' kernel mode execution time in microseconds
    pub tms_cstime: usize,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct utimbuf {
    pub actime: __kernel_old_time_t,
    pub modtime: __kernel_old_time_t,
}

/// Timer specification for timerfd
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct itimerspec {
    /// Timer interval for periodic timers
    pub it_interval: timespec,
    /// Initial expiration time
    pub it_value: timespec,
}

impl Default for itimerspec {
    fn default() -> Self {
        Self {
            it_interval: timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
            it_value: timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
        }
    }
}
