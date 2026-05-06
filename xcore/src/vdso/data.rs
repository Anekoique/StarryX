//! Shared vDSO data page: layout, seqlock writer, timer-tick hook.
//!
//! `VDSO_DATA` is a single page-aligned `'static`; every user process maps
//! the same physical page (R-only) via `map_linear`. The boot CPU's timer
//! ISR is the sole writer.
//!
//! Layout MUST match the user-side mirror in `xmodules/xvdso/src/lib.rs`.

use core::sync::atomic::{AtomicU32, Ordering, fence};

use axhal::{cpu::this_cpu_is_bsp, time};

#[repr(C, align(4096))]
pub struct VdsoData {
    /// Seqlock counter. Even = stable; odd = writer in progress.
    pub seq: AtomicU32,
    pub cpu: u32,
    pub wall_sec: u64,
    pub wall_nsec: u32,
    pub _reserved0: u32,
    pub mono_ns: u64,
    pub mono_cycles_at_capture: u64,
    pub mult: u32,
    pub shift: u32,
}

const _: () = assert!(core::mem::size_of::<VdsoData>() <= 4096);
const _: () = assert!(core::mem::align_of::<VdsoData>() == 4096);

#[unsafe(link_section = ".data.vdso")]
pub static mut VDSO_DATA: VdsoData = VdsoData {
    seq: AtomicU32::new(0),
    cpu: 0,
    wall_sec: 0,
    wall_nsec: 0,
    _reserved0: 0,
    mono_ns: 0,
    mono_cycles_at_capture: 0,
    mult: 0,
    shift: 0,
};

/// `delta_ns = (delta * mult) >> shift`. Computed once at boot from the
/// platform's timer frequency.
const SHIFT: u32 = 24;
const MULT: u32 = {
    let f = time::timer_frequency();
    if f == 0 {
        0
    } else {
        ((time::NANOS_PER_SEC << SHIFT) / f) as u32
    }
};

/// Kernel-virtual address of `VDSO_DATA` (used to derive its phys addr
/// when mapping into user space).
pub fn kernel_addr() -> usize {
    core::ptr::addr_of!(VDSO_DATA) as usize
}

/// Refresh `VDSO_DATA` from the current `axhal::time` snapshot.
///
/// Boot CPU only — secondary CPUs short-circuit so the seqlock stays
/// single-writer. Called from the timer ISR (IRQs disabled).
fn refresh() {
    if !this_cpu_is_bsp() {
        return;
    }
    let mono_cycles = time::current_ticks();
    let mono_ns = time::monotonic_time_nanos();
    let wall_ns = time::wall_time_nanos();

    // SAFETY: single-writer by the BSP guard above.
    let data = unsafe { &mut *core::ptr::addr_of_mut!(VDSO_DATA) };
    let s = data.seq.load(Ordering::Relaxed);
    debug_assert_eq!(s & 1, 0, "vdso seqlock entered with odd seq");

    data.seq.store(s.wrapping_add(1), Ordering::Release);
    fence(Ordering::Release);

    data.mono_cycles_at_capture = mono_cycles;
    data.mono_ns = mono_ns;
    data.wall_sec = wall_ns / time::NANOS_PER_SEC;
    data.wall_nsec = (wall_ns % time::NANOS_PER_SEC) as u32;
    data.mult = MULT;
    data.shift = SHIFT;

    fence(Ordering::Release);
    data.seq.store(s.wrapping_add(2), Ordering::Release);
}

struct VdsoTickImpl;

#[crate_interface::impl_interface]
impl axruntime::VdsoTickIf for VdsoTickImpl {
    fn on_timer_tick() {
        refresh();
    }
}
