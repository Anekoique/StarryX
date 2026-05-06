//! Time-related operations.

pub use core::time::Duration;

/// A measurement of the system clock.
///
/// Currently, it reuses the [`core::time::Duration`] type. But it does not
/// represent a duration, but a clock time.
pub type TimeValue = Duration;

#[cfg(feature = "irq")]
pub use crate::platform::irq::TIMER_IRQ_NUM;
#[cfg(feature = "irq")]
pub use crate::platform::time::set_oneshot_timer;
pub use crate::platform::time::{current_ticks, epochoffset_nanos, nanos_to_ticks, ticks_to_nanos};

/// Returns the platform's hardware timer frequency in Hz.
///
/// Re-exports `axconfig::devices::TIMER_FREQUENCY` so callers (e.g. the
/// vDSO `mult/shift` derivation) don't have to import `axconfig` directly.
/// Returns 0 on the `dummy` platform (no real timer); callers should treat
/// 0 as "vDSO time fast path disabled".
pub const fn timer_frequency() -> u64 {
    #[cfg(any(
        platform_family = "riscv64-qemu-virt",
        platform_family = "riscv64-visionfive2",
        platform_family = "loongarch64-qemu-virt",
    ))]
    {
        axconfig::devices::TIMER_FREQUENCY as u64
    }
    #[cfg(not(any(
        platform_family = "riscv64-qemu-virt",
        platform_family = "riscv64-visionfive2",
        platform_family = "loongarch64-qemu-virt",
    )))]
    {
        0
    }
}

/// Number of milliseconds in a second.
pub const MILLIS_PER_SEC: u64 = 1_000;
/// Number of microseconds in a second.
pub const MICROS_PER_SEC: u64 = 1_000_000;
/// Number of nanoseconds in a second.
pub const NANOS_PER_SEC: u64 = 1_000_000_000;
/// Number of nanoseconds in a millisecond.
pub const NANOS_PER_MILLIS: u64 = 1_000_000;
/// Number of nanoseconds in a microsecond.
pub const NANOS_PER_MICROS: u64 = 1_000;

/// Returns nanoseconds elapsed since system boot.
pub fn monotonic_time_nanos() -> u64 {
    ticks_to_nanos(current_ticks())
}

/// Returns the time elapsed since system boot in [`TimeValue`].
pub fn monotonic_time() -> TimeValue {
    TimeValue::from_nanos(monotonic_time_nanos())
}

/// Returns nanoseconds elapsed since epoch (also known as realtime).
pub fn wall_time_nanos() -> u64 {
    monotonic_time_nanos() + epochoffset_nanos()
}

/// Returns milliseconds elapsed since epoch (also known as realtime).
pub fn wall_time_millis() -> u64 {
    wall_time_nanos() / NANOS_PER_MILLIS
}

/// Returns seconds elapsed since epoch (also known as realtime).
pub fn wall_time_secs() -> u64 {
    wall_time_nanos() / NANOS_PER_SEC
}

/// Returns the time elapsed since epoch (also known as realtime) in [`TimeValue`].
pub fn wall_time() -> TimeValue {
    TimeValue::from_nanos(monotonic_time_nanos() + epochoffset_nanos())
}

/// Busy waiting for the given duration.
pub fn busy_wait(dur: Duration) {
    busy_wait_until(wall_time() + dur);
}

/// Busy waiting until reaching the given deadline.
pub fn busy_wait_until(deadline: TimeValue) {
    while wall_time() < deadline {
        core::hint::spin_loop();
    }
}
