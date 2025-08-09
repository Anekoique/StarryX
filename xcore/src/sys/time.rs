//! Time and timer management for process scheduling and timing.

use axhal::{
    irq::with_irqs_disabled,
    time::{NANOS_PER_MICROS, NANOS_PER_SEC, monotonic_time_nanos},
};
use axtask::TaskExtRef;

use xsignal::{SignalInfo, Signo};
use xutils::ctypes::{SI_KERNEL, SIGALRM};

use crate::task::{send_signal_process, with_current, with_xthread};

numeric_enum_macro::numeric_enum! {
    #[repr(i32)]
    #[allow(non_camel_case_types)]
    #[derive(Eq, PartialEq, Debug, Clone, Copy)]
    pub enum TimerType {
    NONE = -1,
    REAL = 0,
    VIRTUAL = 1,
    PROF = 2,
    }
}

impl From<usize> for TimerType {
    fn from(num: usize) -> Self {
        match Self::try_from(num as i32) {
            Ok(val) => val,
            Err(_) => Self::NONE,
        }
    }
}

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

    pub fn output(&self) -> (usize, usize) {
        (self.utime_ns, self.stime_ns)
    }

    pub fn reset(&mut self, current_timestamp: usize) {
        self.utime_ns = 0;
        self.stime_ns = 0;
        self.user_timestamp = 0;
        self.kernel_timestamp = current_timestamp;
    }

    pub fn switch_into_kernel_mode(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.utime_ns += delta;
        self.kernel_timestamp = now_time_ns;
        if self.timer_type != TimerType::NONE {
            self.update_timer(delta);
        };
    }

    pub fn switch_into_user_mode(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.stime_ns += delta;
        self.user_timestamp = now_time_ns;
        if self.timer_type == TimerType::REAL || self.timer_type == TimerType::PROF {
            self.update_timer(delta);
        }
    }

    pub fn switch_from_old_task(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.stime_ns += delta;
        self.kernel_timestamp = now_time_ns;
        if self.timer_type == TimerType::REAL || self.timer_type == TimerType::PROF {
            self.update_timer(delta);
        }
    }

    pub fn switch_to_new_task(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.kernel_timestamp = now_time_ns;
        if self.timer_type == TimerType::REAL {
            self.update_timer(delta);
        }
    }

    pub fn update_real_timer(&mut self, current_timestamp: usize) {
        let now_time_ns = current_timestamp;
        let delta = now_time_ns - self.kernel_timestamp;
        self.kernel_timestamp = now_time_ns;
        if self.timer_type == TimerType::REAL {
            self.update_timer(delta);
        }
    }

    pub fn set_timer(
        &mut self,
        timer_interval_ns: usize,
        timer_remained_ns: usize,
        timer_type: usize,
    ) -> bool {
        with_irqs_disabled(|| {
            debug!(
                "set_timer: {:?}, timer_interval_ns: {:?}, timer_remained_ns: {:?}",
                timer_type, timer_interval_ns, timer_remained_ns
            );
            self.timer_type = timer_type.into();
            self.timer_interval_ns = timer_interval_ns;
            self.timer_remained_ns = timer_remained_ns;
            self.timer_type != TimerType::NONE
        })
    }

    pub fn update_timer(&mut self, delta: usize) {
        if self.timer_remained_ns == 0 {
            return;
        }
        if self.timer_remained_ns > delta {
            self.timer_remained_ns -= delta;
        } else {
            with_current(|curr| {
                curr.set_interrupted(true);
                send_signal_process(
                    curr.task_ext().process_ref(),
                    SignalInfo::new(Signo::from_repr(SIGALRM as u8).unwrap(), SI_KERNEL as _),
                )
                .map_err(|_| panic!("Failed to send signal"))
                .ok();
            });
            self.timer_remained_ns = 0;
        }
    }

    pub fn get_timer_type(&self) -> TimerType {
        self.timer_type
    }

    pub fn stat_timer(&self) -> (TimerType, usize, usize) {
        (
            self.timer_type,
            self.timer_interval_ns,
            self.timer_remained_ns,
        )
    }

    pub fn clear_timer(&mut self) {
        self.timer_type = TimerType::NONE;
        self.timer_interval_ns = 0;
        self.timer_remained_ns = 0;
    }
}

pub fn time_stat_output() -> (usize, usize, usize, usize, usize, usize) {
    let (utime_ns, stime_ns) = with_xthread(|xthread| xthread.time.read().output());
    (
        utime_ns,
        utime_ns / NANOS_PER_SEC as usize,
        utime_ns / NANOS_PER_MICROS as usize,
        stime_ns,
        stime_ns / NANOS_PER_SEC as usize,
        stime_ns / NANOS_PER_MICROS as usize,
    )
}

macro_rules! update_timer {
    ($func_name:ident, $method:ident) => {
        pub fn $func_name() {
            with_irqs_disabled(|| {
                with_xthread(|xthread| {
                    xthread
                        .time
                        .write()
                        .$method(monotonic_time_nanos() as usize);
                });
            });
        }
    };
}

update_timer!(time_stat_from_kernel_to_user, switch_into_user_mode);
update_timer!(time_stat_from_user_to_kernel, switch_into_kernel_mode);
update_timer!(time_stat_switch_to_new_task, switch_to_new_task);
update_timer!(time_stat_switch_from_old_task, switch_from_old_task);

pub fn set_timer(timer_interval_ns: usize, timer_remained_ns: usize, timer_type: usize) -> bool {
    with_irqs_disabled(|| {
        with_xthread(|xthread| {
            xthread
                .time
                .write()
                .set_timer(timer_interval_ns, timer_remained_ns, timer_type)
        })
    })
}

pub fn clear_timer() {
    with_irqs_disabled(|| {
        with_xthread(|xthread| {
            xthread.time.write().clear_timer();
        })
    })
}
