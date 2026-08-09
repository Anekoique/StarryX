//! Task APIs for single-task configuration.

/// For single-task situation, we just relax the CPU and wait for incoming
/// interrupts.
pub fn yield_now() {
    if cfg!(feature = "irq") {
        xhal::arch::wait_for_irqs();
    } else {
        core::hint::spin_loop();
    }
}

/// For single-task situation, we just busy wait for the given duration.
pub fn sleep(dur: core::time::Duration) {
    xhal::time::busy_wait(dur);
}

/// For single-task situation, we just busy wait until reaching the given
/// deadline.
pub fn sleep_until(deadline: xhal::time::TimeValue) {
    xhal::time::busy_wait_until(deadline);
}
