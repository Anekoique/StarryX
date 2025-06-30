//! Time and timer management for process scheduling and timing.

use axsignal::{SignalInfo, Signo};
use axtask::current;
use linux_raw_sys::general::{SI_KERNEL, SIGALRM};

use crate::task::{TaskExt, send_signal_process};

numeric_enum_macro::numeric_enum! {
    #[repr(i32)]
    #[allow(non_camel_case_types)]
    #[derive(Eq, PartialEq, Debug, Clone, Copy)]
    pub enum TimerType {
    /// Indicates no timer is currently active (not in Linux specification, OS-defined)
    NONE = -1,
    /// Tracks real system runtime
    REAL = 0,
    /// Tracks user mode runtime only
    VIRTUAL = 1,
    /// Tracks all user/kernel mode runtime for the process
    PROF = 2,
    }
}

impl From<usize> for TimerType {
    /// Convert a usize value to TimerType, returns NONE if invalid
    fn from(num: usize) -> Self {
        match Self::try_from(num as i32) {
            Ok(val) => val,
            Err(_) => Self::NONE,
        }
    }
}

/// Time statistics and timer state for a process
pub struct TimeStat {
    utime_ns: usize,
    stime_ns: usize,
    user_timestamp: usize,
    kernel_timestamp: usize,
    timer_type: TimerType,
    timer_interval_ns: usize,
    timer_remained_ns: usize,
}

impl Default for TimeStat {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeStat {
    /// Create a new TimeStat instance with default values
    pub fn new() -> Self {
        Self {
            utime_ns: 0,
            stime_ns: 0,
            user_timestamp: 0,
            kernel_timestamp: 0,
            timer_type: TimerType::NONE,
            timer_interval_ns: 0,
            timer_remained_ns: 0,
        }
    }

    /// Get the current user time and system time statistics
    pub fn output(&self) -> (usize, usize) {
        (self.utime_ns, self.stime_ns)
    }

    /// Reset all time statistics to zero with the given timestamp
    pub fn reset(&mut self, current_timestamp: usize) {
        self.utime_ns = 0;
        self.stime_ns = 0;
        self.user_timestamp = 0;
        self.kernel_timestamp = current_timestamp;
    }

    /// Update statistics when switching from user mode to kernel mode
    pub fn switch_into_kernel_mode(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.utime_ns += delta;
        self.kernel_timestamp = now_time_ns;
        if self.timer_type != TimerType::NONE {
            self.update_timer(delta);
        };
    }

    /// Update statistics when switching from kernel mode to user mode
    pub fn switch_into_user_mode(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.stime_ns += delta;
        self.user_timestamp = now_time_ns;
        if self.timer_type == TimerType::REAL || self.timer_type == TimerType::PROF {
            self.update_timer(delta);
        }
    }

    /// Update statistics when switching away from the current task
    pub fn switch_from_old_task(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.stime_ns += delta;
        self.kernel_timestamp = now_time_ns;
        if self.timer_type == TimerType::REAL || self.timer_type == TimerType::PROF {
            self.update_timer(delta);
        }
    }

    /// Update statistics when switching to a new task
    pub fn switch_to_new_task(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.kernel_timestamp = now_time_ns;
        if self.timer_type == TimerType::REAL {
            self.update_timer(delta);
        }
    }

    /// Update the real-time timer with current timestamp
    pub fn update_real_timer(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.kernel_timestamp = now_time_ns;
        if self.timer_type == TimerType::REAL {
            self.update_timer(delta);
        }
    }

    /// Set a new timer with specified interval, remaining time, and type
    /// Returns true if the timer type is valid and set successfully
    pub fn set_timer(
        &mut self,
        timer_interval_ns: usize,
        timer_remained_ns: usize,
        timer_type: usize,
    ) -> bool {
        self.timer_type = timer_type.into();
        self.timer_interval_ns = timer_interval_ns;
        self.timer_remained_ns = timer_remained_ns;
        self.timer_type != TimerType::NONE
    }

    /// Update timer countdown and send SIGALRM signal when timer expires
    pub fn update_timer(&mut self, delta: usize) {
        if self.timer_remained_ns == 0 {
            return;
        }
        if self.timer_remained_ns > delta {
            self.timer_remained_ns -= delta;
        } else {
            let _ = send_signal_process(
                TaskExt::from_task(&current()).thread.process(),
                SignalInfo::new(Signo::from_repr(SIGALRM as u8).unwrap(), SI_KERNEL as _),
            );
            self.timer_remained_ns = 0;
        }
    }

    /// Get the current timer type
    pub fn get_timer_type(&self) -> TimerType {
        self.timer_type
    }

    /// Get the current timer statistics (type, interval, remaining time)
    pub fn stat_timer(&self) -> (TimerType, usize, usize) {
        (
            self.timer_type,
            self.timer_interval_ns,
            self.timer_remained_ns,
        )
    }

    /// Clear and stop the current timer
    pub fn clear_timer(&mut self) {
        self.timer_type = TimerType::NONE;
        self.timer_interval_ns = 0;
        self.timer_remained_ns = 0;
    }
}
