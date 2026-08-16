// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0

//! Linux-compatible virtual dynamic shared object component.
//!
//! The build script obtains a pinned prebuilt Linux vDSO. This crate embeds
//! that image, owns the Linux-compatible vvar data pages, and publishes safe
//! accessors for a kernel profile to map them into user address spaces.

#![no_std]

use core::{
    arch::global_asm,
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering, fence},
};

use memory_addr::PAGE_SIZE_4K;
use spin::Once;
use xhal::{cpu::this_cpu_is_bsp, time};
use xmas_elf::{ElfFile, sections::SectionData, symbol_table::Entry};

macro_rules! embed_linux_vdso {
    () => {
        concat!(
            ".section .rodata.vdso, \"a\"\n",
            ".balign 4096\n",
            ".global xvdso_image_start, xvdso_image_end\n",
            "xvdso_image_start:\n",
            ".incbin \"",
            env!("XVDSO_IMAGE_PATH"),
            "\"\n",
            ".balign 4096\n",
            "xvdso_image_end:\n",
            ".previous\n",
        )
    };
}

global_asm!(embed_linux_vdso!());

// `.incbin` is invisible to Cargo's dependency tracking. Referencing the same
// file here makes a provider image change invalidate this crate.
const _VDSO_BUILD_DEPENDENCY: &[u8] = include_bytes!(env!("XVDSO_IMAGE_PATH"));

unsafe extern "C" {
    static xvdso_image_start: u8;
    static xvdso_image_end: u8;
}

const CLOCK_REALTIME: usize = 0;
const CLOCK_MONOTONIC: usize = 1;
const CLOCK_MONOTONIC_RAW: usize = 4;
const CLOCK_REALTIME_COARSE: usize = 5;
const CLOCK_MONOTONIC_COARSE: usize = 6;
const CLOCK_BOOTTIME: usize = 7;
const CLOCK_TAI: usize = 11;
const VDSO_BASES: usize = CLOCK_TAI + 1;

const CS_HRES_COARSE: usize = 0;
const CS_RAW: usize = 1;
const CS_BASES: usize = CS_RAW + 1;

const VDSO_CLOCKMODE_NONE: i32 = 0;
const VDSO_CLOCKMODE_ARCHTIMER: i32 = 1;
const SHIFT: u32 = 24;
const MULT: u32 = {
    let frequency = time::timer_frequency();
    match (time::NANOS_PER_SEC << SHIFT).checked_div(frequency) {
        Some(value) if value <= u32::MAX as u64 => value as u32,
        _ => 0,
    }
};

#[cfg(target_arch = "riscv64")]
pub const VVAR_PAGES: usize = 2;
// Linux 6.8 LoongArch uses the generic, time-namespace, and one
// architecture-data page when NR_CPUS fits in a single page.
#[cfg(target_arch = "loongarch64")]
pub const VVAR_PAGES: usize = 3;

/// Linux `struct vdso_timestamp` from `include/vdso/datapage.h`.
#[repr(C)]
#[derive(Clone, Copy)]
struct VdsoTimestamp {
    sec: u64,
    nsec: u64,
}

impl VdsoTimestamp {
    const ZERO: Self = Self { sec: 0, nsec: 0 };

    fn from_nanos(nanos: u64, shifted: bool) -> Self {
        let subsec = nanos % time::NANOS_PER_SEC;
        Self {
            sec: nanos / time::NANOS_PER_SEC,
            nsec: if shifted { subsec << SHIFT } else { subsec },
        }
    }
}

/// Linux 6.8 `struct vdso_data`.
///
/// The external image reads this layout directly, so its field order and
/// sizes are part of the userspace ABI.
#[repr(C)]
struct VdsoData {
    seq: AtomicU32,
    clock_mode: i32,
    cycle_last: u64,
    mask: u64,
    mult: u32,
    shift: u32,
    basetime: [VdsoTimestamp; VDSO_BASES],
    tz_minuteswest: i32,
    tz_dsttime: i32,
    hrtimer_res: u32,
    __unused: u32,
}

impl VdsoData {
    const fn empty() -> Self {
        Self {
            seq: AtomicU32::new(0),
            clock_mode: VDSO_CLOCKMODE_NONE,
            cycle_last: 0,
            mask: u64::MAX,
            mult: 0,
            shift: 0,
            basetime: [VdsoTimestamp::ZERO; VDSO_BASES],
            tz_minuteswest: 0,
            tz_dsttime: 0,
            hrtimer_res: 1,
            __unused: 0,
        }
    }
}

const _: () = assert!(core::mem::size_of::<VdsoData>() == 240);

#[repr(C, align(4096))]
struct VdsoDataPage {
    clocks: UnsafeCell<[VdsoData; CS_BASES]>,
}

// SAFETY: kernel writes are serialized by `VDSO_UPDATE_LOCK` with local
// interrupts disabled. Userspace receives read-only mappings and synchronizes
// through the sequence counters used by the Linux vDSO implementation.
unsafe impl Sync for VdsoDataPage {}

#[unsafe(link_section = ".data.vdso")]
static VDSO_DATA_PAGE: VdsoDataPage = VdsoDataPage {
    clocks: UnsafeCell::new([VdsoData::empty(), VdsoData::empty()]),
};

const _: () = assert!(core::mem::size_of::<VdsoDataPage>() == PAGE_SIZE_4K);

#[cfg(target_arch = "loongarch64")]
#[repr(C, align(4096))]
struct VdsoArchDataPage([u8; PAGE_SIZE_4K]);

#[cfg(target_arch = "loongarch64")]
static VDSO_ARCH_DATA_PAGE: VdsoArchDataPage = VdsoArchDataPage([0; PAGE_SIZE_4K]);

static VDSO_UPDATE_LOCK: xsync::spin::SpinNoIrq<()> = xsync::spin::SpinNoIrq::new(());
static RT_SIGRETURN_OFFSET: Once<usize> = Once::new();

/// Returns the embedded, immutable, page-aligned Linux vDSO image.
pub fn image() -> &'static [u8] {
    let start = core::ptr::addr_of!(xvdso_image_start) as usize;
    let end = core::ptr::addr_of!(xvdso_image_end) as usize;
    let len = end
        .checked_sub(start)
        .expect("external vDSO linker symbols are reversed");
    // SAFETY: the assembly block defines `start..end` as one immutable,
    // page-aligned region that remains live for the kernel's lifetime.
    unsafe { core::slice::from_raw_parts(start as *const u8, len) }
}

/// Returns the symbol offset of Linux's signal-return trampoline.
pub fn rt_sigreturn_offset() -> usize {
    *RT_SIGRETURN_OFFSET.call_once(|| {
        let elf = ElfFile::new(image()).expect("external vDSO is not a valid ELF");
        elf.section_iter()
            .find_map(|section| {
                let SectionData::DynSymbolTable64(entries) = section.get_data(&elf).ok()? else {
                    return None;
                };
                entries.iter().find_map(|symbol| {
                    (symbol.get_name(&elf).ok()? == "__vdso_rt_sigreturn")
                        .then_some(symbol.value() as usize)
                })
            })
            .expect("external vDSO is missing __vdso_rt_sigreturn")
    })
}

/// Returns the kernel-owned Linux vvar data page.
pub fn data_page() -> &'static impl Sync {
    &VDSO_DATA_PAGE
}

/// Returns the kernel-owned LoongArch architecture data page.
#[cfg(target_arch = "loongarch64")]
pub fn arch_data_page() -> &'static impl Sync {
    &VDSO_ARCH_DATA_PAGE
}

unsafe fn begin_update(data: *mut VdsoData) -> u32 {
    // SAFETY: `data` points into `VDSO_DATA_PAGE` and the caller holds the
    // update lock. Atomic access is required by the Linux reader protocol.
    let seq = unsafe { &*core::ptr::addr_of!((*data).seq) };
    let value = seq.load(Ordering::Relaxed);
    debug_assert_eq!(value & 1, 0, "vDSO update started with an odd sequence");
    seq.store(value.wrapping_add(1), Ordering::Relaxed);
    fence(Ordering::SeqCst);
    value
}

unsafe fn finish_update(data: *mut VdsoData, previous: u32) {
    fence(Ordering::SeqCst);
    // SAFETY: see `begin_update`; this release store publishes all field
    // updates to Linux vDSO readers.
    unsafe { &*core::ptr::addr_of!((*data).seq) }
        .store(previous.wrapping_add(2), Ordering::Release);
}

unsafe fn write_common_fields(data: *mut VdsoData, cycles: u64) {
    let clock_mode = if MULT == 0 {
        VDSO_CLOCKMODE_NONE
    } else {
        VDSO_CLOCKMODE_ARCHTIMER
    };
    // SAFETY: the caller holds `VDSO_UPDATE_LOCK`; every pointer targets a
    // naturally aligned field in the shared data page.
    unsafe {
        core::ptr::addr_of_mut!((*data).clock_mode).write_volatile(clock_mode);
        core::ptr::addr_of_mut!((*data).cycle_last).write_volatile(cycles);
        core::ptr::addr_of_mut!((*data).mask).write_volatile(u64::MAX);
        core::ptr::addr_of_mut!((*data).mult).write_volatile(MULT);
        core::ptr::addr_of_mut!((*data).shift).write_volatile(SHIFT);
        core::ptr::addr_of_mut!((*data).hrtimer_res).write_volatile(1);
    }
}

unsafe fn write_timestamp(data: *mut VdsoData, clock_id: usize, nanos: u64, shifted: bool) {
    debug_assert!(clock_id < VDSO_BASES);
    let timestamp = VdsoTimestamp::from_nanos(nanos, shifted);
    // SAFETY: the caller holds `VDSO_UPDATE_LOCK`, the index is within
    // `basetime`, and the destination is naturally aligned.
    unsafe {
        core::ptr::addr_of_mut!((*data).basetime)
            .cast::<VdsoTimestamp>()
            .add(clock_id)
            .write_volatile(timestamp);
    }
}

/// Refreshes the shared Linux vvar data from the current clock source.
pub fn refresh_data() {
    if !this_cpu_is_bsp() {
        return;
    }

    let _guard = VDSO_UPDATE_LOCK.lock();
    let cycles = time::current_ticks();
    let monotonic = time::ticks_to_nanos(cycles);
    let realtime = monotonic + time::epochoffset_nanos();

    // SAFETY: the lock serializes all writers and disables local interrupts;
    // both pointers refer to the two Linux clocksource records in the page.
    unsafe {
        let clocks = (*VDSO_DATA_PAGE.clocks.get()).as_mut_ptr();

        let hres = clocks.add(CS_HRES_COARSE);
        let sequence = begin_update(hres);
        write_common_fields(hres, cycles);
        write_timestamp(hres, CLOCK_REALTIME, realtime, true);
        write_timestamp(hres, CLOCK_MONOTONIC, monotonic, true);
        write_timestamp(hres, CLOCK_BOOTTIME, monotonic, true);
        write_timestamp(hres, CLOCK_REALTIME_COARSE, realtime, false);
        write_timestamp(hres, CLOCK_MONOTONIC_COARSE, monotonic, false);
        finish_update(hres, sequence);

        let raw = clocks.add(CS_RAW);
        let sequence = begin_update(raw);
        write_common_fields(raw, cycles);
        write_timestamp(raw, CLOCK_MONOTONIC_RAW, monotonic, true);
        finish_update(raw, sequence);
    }
}
