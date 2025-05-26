use lazyinit::LazyInit;
use loongArch64::time::Time;

static NANOS_PER_TICK: LazyInit<u64> = LazyInit::new();

/// RTC wall time offset in nanoseconds at monotonic time base.
static mut RTC_EPOCHOFFSET_NANOS: u64 = 0;

/// Returns the current clock time in hardware ticks.
#[inline]
pub fn current_ticks() -> u64 {
    Time::read() as _
}

/// Return epoch offset in nanoseconds (wall time offset to monotonic clock start).
#[inline]
pub fn epochoffset_nanos() -> u64 {
    unsafe { RTC_EPOCHOFFSET_NANOS }
}

/// Converts hardware ticks to nanoseconds.
#[inline]
pub fn ticks_to_nanos(ticks: u64) -> u64 {
    ticks * *NANOS_PER_TICK
}

/// Converts nanoseconds to hardware ticks.
#[inline]
pub fn nanos_to_ticks(nanos: u64) -> u64 {
    nanos / *NANOS_PER_TICK
}

/// Set a one-shot timer.
///
/// A timer interrupt will be triggered at the specified monotonic time deadline (in nanoseconds).
///
/// LoongArch64 TCFG CSR: <https://loongson.github.io/LoongArch-Documentation/LoongArch-Vol1-EN.html#timer-configuration>
#[cfg(feature = "irq")]
pub fn set_oneshot_timer(deadline_ns: u64) {
    use loongArch64::register::tcfg;

    let ticks_now = current_ticks();
    let ticks_deadline = nanos_to_ticks(deadline_ns);
    let init_value = ticks_deadline - ticks_now;
    tcfg::set_init_val(init_value as _);
    tcfg::set_en(true);
}

pub fn init_early() {
    // Reference: https://gitlab.com/qemu-project/qemu/-/blob/v10.0.0/hw/rtc/ls7a_rtc.c?ref_type=tags
    #[cfg(feature = "rtc")]
    if axconfig::devices::RTC_PADDR != 0 {
        use crate::mem::phys_to_virt;
        use chrono::{TimeZone, Timelike, Utc};
        use memory_addr::PhysAddr;

        const LS7A_RTC: PhysAddr = pa!(axconfig::devices::RTC_PADDR);

        const SYS_TOY_READ0: usize = 0x2C;
        const SYS_TOY_READ1: usize = 0x30;
        const SYS_RTCCTRL: usize = 0x40;

        const TOY_ENABLE: u32 = 1 << 11;
        const OSC_ENABLE: u32 = 1 << 8;

        let base = phys_to_virt(LS7A_RTC).as_usize();

        fn extract_bits(value: u32, range: core::ops::Range<u32>) -> u32 {
            (value >> range.start) & ((1 << (range.end - range.start)) - 1)
        }

        let (value, year) = unsafe {
            ((base + SYS_RTCCTRL) as *mut u32).write_volatile(TOY_ENABLE | OSC_ENABLE);
            let value = ((base + SYS_TOY_READ0) as *const u32).read_volatile();
            let year = ((base + SYS_TOY_READ1) as *const u32).read_volatile();
            (value, year)
        };

        let time = Utc
            .with_ymd_and_hms(
                1900 + year as i32,
                extract_bits(value, 26..32),
                extract_bits(value, 21..26),
                extract_bits(value, 16..21),
                extract_bits(value, 10..16),
                extract_bits(value, 4..10),
            )
            .unwrap()
            .with_nanosecond(extract_bits(value, 0..4) * crate::time::NANOS_PER_MILLIS as u32)
            .unwrap();
        let epoch_time_nanos = time.timestamp_nanos_opt().unwrap();

        unsafe {
            RTC_EPOCHOFFSET_NANOS = epoch_time_nanos as u64 - ticks_to_nanos(current_ticks());
        }
    }
}

pub(super) fn init_percpu() {
    #[cfg(feature = "irq")]
    {
        use loongArch64::register::tcfg;
        tcfg::set_init_val(0);
        tcfg::set_periodic(false);
        tcfg::set_en(true);
        super::irq::set_enable(super::irq::TIMER_IRQ_NUM, true);
    }
}

pub(super) fn init_primary() {
    NANOS_PER_TICK
        .init_once(crate::time::NANOS_PER_SEC / loongArch64::time::get_timer_freq() as u64);
}
