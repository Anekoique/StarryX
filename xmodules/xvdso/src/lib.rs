//! StarryX vDSO image source.
//!
//! Built as a `cdylib` for `*-unknown-none` with a per-arch linker script.
//! Time entries serve `CLOCK_REALTIME` and `CLOCK_MONOTONIC[_RAW]` from
//! the kernel-published shared data page; unsupported clocks fall through
//! to the syscall trap.

#![no_std]

use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};

mod arch;

/// Mirror of the kernel-side `xkernel::vdso::data::VdsoData`. The kernel
/// patches `arch::VDSO_DATA_ADDR` at install time so the body below can
/// locate this struct position-independently.
#[repr(C, align(4096))]
struct VdsoData {
    seq: AtomicU32,
    cpu: u32,
    wall_sec: u64,
    wall_nsec: u32,
    _reserved0: u32,
    mono_ns: u64,
    mono_cycles_at_capture: u64,
    mult: u32,
    shift: u32,
}

#[repr(C)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

#[repr(C)]
pub struct Timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

const CLOCK_REALTIME: i32 = 0;
const CLOCK_MONOTONIC: i32 = 1;
const CLOCK_MONOTONIC_RAW: i32 = 4;
const NANOS_PER_SEC: u64 = 1_000_000_000;

const NR_CLOCK_GETTIME: usize = 113;
const NR_GETTIMEOFDAY: usize = 169;
const NR_CLOCK_GETRES: usize = 114;
const NR_GETCPU: usize = 168;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

struct Snapshot {
    wall_sec: u64,
    wall_nsec: u32,
    mono_ns: u64,
    mono_cycles_at_capture: u64,
    mult: u32,
    shift: u32,
}

/// Read `VdsoData` under the seqlock. Returns `None` if too many writers
/// passed through (very unlikely — single-writer in practice).
#[inline(always)]
unsafe fn read_data() -> Option<Snapshot> {
    let data = arch::vdso_data_addr() as *const VdsoData;
    let seq = unsafe { &(*data).seq };
    for _ in 0..1024 {
        let s1 = seq.load(Ordering::Acquire);
        if s1 & 1 != 0 {
            core::hint::spin_loop();
            continue;
        }
        let snap = Snapshot {
            wall_sec: unsafe { (*data).wall_sec },
            wall_nsec: unsafe { (*data).wall_nsec },
            mono_ns: unsafe { (*data).mono_ns },
            mono_cycles_at_capture: unsafe { (*data).mono_cycles_at_capture },
            mult: unsafe { (*data).mult },
            shift: unsafe { (*data).shift },
        };
        if seq.load(Ordering::Acquire) == s1 {
            return Some(snap);
        }
    }
    None
}

#[inline(always)]
fn monotonic_ns(s: &Snapshot) -> u64 {
    let delta = unsafe { arch::rdtime() }.wrapping_sub(s.mono_cycles_at_capture);
    let delta_ns = (delta as u128 * s.mult as u128) >> s.shift;
    s.mono_ns.wrapping_add(delta_ns as u64)
}

#[inline(always)]
fn realtime_ns(s: &Snapshot) -> u64 {
    let mono = monotonic_ns(s);
    let wall_ns = s.wall_sec.wrapping_mul(NANOS_PER_SEC) + s.wall_nsec as u64;
    wall_ns.wrapping_add(mono.wrapping_sub(s.mono_ns))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __vdso_clock_gettime(clock_id: i32, ts: *mut Timespec) -> i32 {
    if ts.is_null() {
        return -14; // -EFAULT
    }
    if matches!(
        clock_id,
        CLOCK_REALTIME | CLOCK_MONOTONIC | CLOCK_MONOTONIC_RAW
    ) {
        if let Some(snap) = unsafe { read_data() }
            && snap.mult != 0
        {
            let ns = if clock_id == CLOCK_REALTIME {
                realtime_ns(&snap)
            } else {
                monotonic_ns(&snap)
            };
            unsafe {
                (*ts).tv_sec = (ns / NANOS_PER_SEC) as i64;
                (*ts).tv_nsec = (ns % NANOS_PER_SEC) as i64;
            }
            return 0;
        }
    }
    unsafe { arch::syscall2(NR_CLOCK_GETTIME, clock_id as usize, ts as usize) as i32 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __vdso_gettimeofday(tv: *mut Timeval, tz: *mut c_void) -> i32 {
    if !tv.is_null()
        && let Some(snap) = unsafe { read_data() }
        && snap.mult != 0
    {
        let ns = realtime_ns(&snap);
        unsafe {
            (*tv).tv_sec = (ns / NANOS_PER_SEC) as i64;
            (*tv).tv_usec = ((ns % NANOS_PER_SEC) / 1_000) as i64;
        }
        return 0;
    }
    unsafe { arch::syscall2(NR_GETTIMEOFDAY, tv as usize, tz as usize) as i32 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __vdso_clock_getres(clock_id: i32, res: *mut Timespec) -> i32 {
    if matches!(
        clock_id,
        CLOCK_REALTIME | CLOCK_MONOTONIC | CLOCK_MONOTONIC_RAW
    ) {
        if !res.is_null() {
            unsafe {
                (*res).tv_sec = 0;
                (*res).tv_nsec = 1;
            }
        }
        return 0;
    }
    unsafe { arch::syscall2(NR_CLOCK_GETRES, clock_id as usize, res as usize) as i32 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __vdso_time(tloc: *mut i64) -> i64 {
    let mut ts = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { __vdso_clock_gettime(CLOCK_REALTIME, &mut ts) } != 0 {
        return -1;
    }
    if !tloc.is_null() {
        unsafe { *tloc = ts.tv_sec };
    }
    ts.tv_sec
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __vdso_getcpu(
    cpu: *mut u32,
    node: *mut u32,
    _tcache: *mut c_void,
) -> i32 {
    unsafe { arch::syscall3(NR_GETCPU, cpu as usize, node as usize, 0) as i32 }
}

pub use arch::__vdso_rt_sigreturn;
