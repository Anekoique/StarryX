use alloc::sync::Arc;
use core::any::Any;

use axerrno::{LinuxError, LinuxResult};
use axio::PollState;
use axsync::Mutex;

use xutils::{
    ctypes::{
        __kernel_clockid_t, CLOCK_MONOTONIC, CLOCK_REALTIME, S_IFIFO, TFD_NONBLOCK,
        TFD_TIMER_ABSTIME, fs::Kstat, sys::itimerspec, timespec,
    },
    time::{TimeValue, TimeValueLike, monotonic_time, wall_time},
};

use crate::{fs::file::FileLike, task::have_signals};

/// Internal timer state
#[derive(Debug, Clone)]
struct TimerState {
    /// Clock ID (CLOCK_REALTIME or CLOCK_MONOTONIC)
    clock_id: __kernel_clockid_t,
    /// Timer specification
    spec: itimerspec,
    /// When the timer was set (in nanoseconds since boot)
    set_time: u64,
    /// Number of expirations since last read
    expirations: u64,
    /// Whether timer is active
    active: bool,
    /// Whether timer is in absolute time mode
    abstime: bool,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            clock_id: CLOCK_REALTIME as _,
            spec: itimerspec::default(),
            set_time: 0,
            expirations: 0,
            active: false,
            abstime: false,
        }
    }
}

impl TimerState {
    /// Get current time for the timer's clock
    fn get_current_time(&self) -> TimeValue {
        match self.clock_id as u32 {
            CLOCK_REALTIME => wall_time(),
            CLOCK_MONOTONIC => monotonic_time(),
            _ => wall_time(), // fallback
        }
    }

    /// Check if timer has expired and return number of expirations
    fn check_expiration(&mut self) -> u64 {
        if !self.active || self.spec.it_value.to_nanos() == 0 {
            return 0;
        }

        let now = self.get_current_time();
        let current_ns = now.as_nanos() as u64;

        let expire_time_ns = if self.abstime {
            // Absolute time mode
            self.spec.it_value.to_nanos()
        } else {
            // Relative time mode - add to when timer was set
            self.set_time + self.spec.it_value.to_nanos()
        };

        if current_ns >= expire_time_ns {
            let interval_ns = self.spec.it_interval.to_nanos();

            if let Some(periods_elapsed) = (current_ns - expire_time_ns).checked_div(interval_ns) {
                // Periodic timer
                let periods = periods_elapsed + 1;
                self.expirations += periods;

                // Update next expiration time
                if self.abstime {
                    self.spec.it_value =
                        timespec::from_nanos(expire_time_ns + periods * interval_ns);
                } else {
                    self.set_time = expire_time_ns + periods * interval_ns;
                }

                return self.expirations;
            } else {
                // One-shot timer
                self.active = false;
                self.expirations += 1;
                return self.expirations;
            }
        }

        0
    }

    /// Set timer with new specification
    fn set_timer(&mut self, spec: itimerspec, flags: i32) -> LinuxResult<itimerspec> {
        let old_spec = self.spec;

        self.spec = spec;
        self.abstime = (flags & TFD_TIMER_ABSTIME as i32) != 0;
        self.active = spec.it_value.to_nanos() != 0;
        self.expirations = 0;

        if self.active && !self.abstime {
            // For relative timers, record when they were set
            self.set_time = self.get_current_time().as_nanos() as u64;
        }

        Ok(old_spec)
    }

    /// Get current timer specification
    fn get_timer(&self) -> itimerspec {
        if !self.active {
            return itimerspec {
                it_interval: self.spec.it_interval,
                it_value: timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                },
            };
        }

        let now = self.get_current_time();
        let current_ns = now.as_nanos() as u64;

        let expire_time_ns = if self.abstime {
            self.spec.it_value.to_nanos()
        } else {
            self.set_time + self.spec.it_value.to_nanos()
        };

        let remaining_ns = expire_time_ns.saturating_sub(current_ns);

        itimerspec {
            it_interval: self.spec.it_interval,
            it_value: timespec::from_nanos(remaining_ns),
        }
    }
}

/// TimerFd implementation for timer file descriptors
pub struct TimerFd {
    /// Timer state protected by mutex
    state: Arc<Mutex<TimerState>>,
    /// Whether this fd is non-blocking
    nonblocking: bool,
}

impl TimerFd {
    /// Create a new TimerFd with specified clock and flags
    pub fn new(clock_id: __kernel_clockid_t, flags: i32) -> LinuxResult<Self> {
        // Validate clock ID
        match clock_id as u32 {
            CLOCK_REALTIME | CLOCK_MONOTONIC => {}
            _ => return Err(LinuxError::EINVAL),
        }

        let state = TimerState {
            clock_id,
            ..Default::default()
        };

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            nonblocking: (flags & TFD_NONBLOCK as i32) != 0,
        })
    }

    /// Set timer with new specification
    pub fn set_timer(&self, spec: itimerspec, flags: i32) -> LinuxResult<itimerspec> {
        self.state.lock().set_timer(spec, flags)
    }

    /// Get current timer specification
    pub fn get_timer(&self) -> itimerspec {
        self.state.lock().get_timer()
    }
}

impl FileLike for TimerFd {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        if buf.len() < 8 {
            return Err(LinuxError::EINVAL);
        }

        loop {
            let mut state = self.state.lock();
            let expirations = state.check_expiration();

            if expirations > 0 {
                // Reset expiration counter
                state.expirations = 0;
                drop(state);

                // Write expiration count as little-endian u64
                buf[..8].copy_from_slice(&expirations.to_le_bytes());
                return Ok(8);
            }

            if self.nonblocking {
                return Err(LinuxError::EAGAIN);
            }

            // Drop the lock and wait
            drop(state);
            if have_signals() {
                return Err(LinuxError::EINTR);
            }
            axtask::yield_now();
        }
    }

    fn write(&self, _buf: &[u8]) -> LinuxResult<usize> {
        Err(LinuxError::EINVAL)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        Ok(Kstat {
            mode: S_IFIFO | 0o600u32,
            ..Default::default()
        })
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        let mut state = self.state.lock();
        let expirations = state.check_expiration();

        Ok(PollState {
            readable: expirations > 0,
            writable: false,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) {
        // TimerFd nonblocking behavior is set at creation time
        // This is a no-op for compatibility
    }

    fn is_nonblocking(&self) -> bool {
        self.nonblocking
    }
}
