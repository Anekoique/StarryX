# `vdso-support` PLAN `02`

> Status: Revised
> Feature: `vdso-support`
> Iteration: `02`
> Owner: Executor
> Depends on:
> - Previous Plan: `01_PLAN.md`
> - Review: `01_REVIEW.md`
> - Master Directive: none

---

## Summary

Iteration 02 closes the five non-blocking MEDIUMs from `01_REVIEW.md` (R-101..R-105). The macro-design is unchanged: per-arch `cdylib` vDSO from a workspace-excluded `xmodules/xvdso/`, embedded into the kernel, mapped into every user address space alongside a kernel-published seqlock-protected time data page, with `AT_SYSINFO_EHDR` in auxv. Signal-return migrates from `SIGNAL_TRAMPOLINE` into the vDSO atomically in Phase 4. This iteration tightens five spots the executor would otherwise trip on: (a) the timebase-frequency source is named correctly (`axconfig::devices::TIMER_FREQUENCY`); (b) Phase 4's "one atomic commit" diff list is enumerated file-by-file including `xsignal/src/api/thread.rs:106` and the dead-arch `arch/{x86_64,aarch64}.rs` files; (c) V-UT-5's Verdef check moves to a host-side script (`scripts/check-vdso-verdef.sh`) invoked by `make vdso-blob`; (d) the data page is explicitly a single shared kernel-resident phys page mapped via `map_linear`, not per-process alloc — required for the seqlock contract; (e) `make build` / `make clippy` / `make rv` / `make la` / `make vf2` gain a `vdso-blob` prerequisite, with the kernel `build.rs` panicking on missing blob with a pointer to the right Make target.

## Log

[**Added**]

- New constraint **C-14**: `VdsoData` is a single kernel-resident `'static` instance; the user-side data-page mapping is by-phys-addr (`map_linear`), shared across all processes (R-104).
- New constraint **C-15**: `make build`, `make clippy`, `make rv`, `make la`, `make vf2` all carry a `vdso-blob` prerequisite. The kernel's `build.rs` panics with a clear "run `make vdso-blob ARCH=$ARCH` first" message on missing blob; cargo-direct invocation is documented as unsupported (R-105).
- New host-side check **V-UT-5'**: `scripts/check-vdso-verdef.sh` runs `llvm-readelf -V` on the blob and asserts `LINUX_2.6` Verdef + symbol bindings; invoked at the end of `make vdso-blob`. Replaces the original V-UT-5 (which couldn't run inside a `*-unknown-none` cdylib's test harness) (R-103).
- Phase 4 diff list now enumerates: `xmodules/xsignal/src/api/process.rs`, `xmodules/xsignal/src/api/thread.rs:104-106` (field-to-method rename), `xmodules/xsignal/src/arch/{riscv,loongarch64,x86_64,aarch64}.rs` (the on-disk filename is `riscv.rs`, not `riscv64.rs` — verified at `xmodules/xsignal/src/arch/`), `xcore/src/config.rs:38`, `xcore/src/mm/init.rs:35-39 + 154`, `xcore/src/task/proc.rs:215-218`, `xcore/src/vdso/{install,resolve}.rs` (R-102).
- Mult/shift derivation cites the **actual** kernel surface: `axconfig::devices::TIMER_FREQUENCY` (per `arceos/modules/axhal/src/platform/riscv64_qemu_virt/time.rs:3`). Two viable wirings now pinned: (a) add a thin re-export `pub fn axhal::time::timer_frequency() -> u64` (preferred — keeps `xcore::vdso` from importing `axconfig` directly); (b) import `axconfig::devices::TIMER_FREQUENCY` directly in `xcore::vdso::tick`. Plan picks **(a)**: clean re-export, minimal cross-cutting change (R-101).
- SMP guard concretized: `axhal::cpu::this_cpu_is_bsp()` (verified at `arceos/modules/axhal/src/cpu.rs:21`) — no `BOOT_CPU` constant invented.

[**Changed**]

- `## Spec` is restated in full (deep-tier rule).
- C-9 / G-3 / Architecture pseudocode tightened to call out the data page's *shared* nature.
- Architecture file-tree corrected: `xmodules/xvdso/src/arch/riscv64.rs` is **kept** (the vDSO source crate is fresh, so we name files at will) but the **xsignal** edits in Phase 4 target `xmodules/xsignal/src/arch/riscv.rs` (existing tree's filename — no rename).
- Phase 1's `vdso-blob` target now also runs `scripts/check-vdso-verdef.sh` post-build.
- Phase 4 diff list enumerated file-by-file.

[**Removed**]

- The original `V-UT-5` (in-crate Verdef check via `[[test]] required-features`). Replaced by host-side `scripts/check-vdso-verdef.sh` invoked from `make vdso-blob` (R-103).

[**Unresolved**]

- vDSO ASLR (NG-1, unchanged).
- Per-CPU `getcpu` correctness (NG-2, unchanged).

[**Response Matrix**]

(R-001..R-013 from `00_REVIEW.md` — all carried forward, all already resolved in `01_PLAN.md`. Not duplicated here. Below: only `01_REVIEW.md`'s findings.)

| Source | ID    | Decision  | Resolution                                                                                                                                                                                                          |
| ------ | ----- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Review | R-101 | Accepted  | Mult/shift derivation cites `axconfig::devices::TIMER_FREQUENCY`. New thin re-export `axhal::time::timer_frequency() -> u64` exposes it; `xcore::vdso::tick` consumes the re-export. Phase 3 diff updated.            |
| Review | R-102 | Accepted  | Phase 4 diff list enumerated file-by-file: `xmodules/xsignal/src/api/{process.rs, thread.rs}` (thread.rs:104-106 field→method); `xmodules/xsignal/src/arch/{riscv,loongarch64,x86_64,aarch64}.rs` (filename is `riscv.rs`, verified — and yes, x86_64/aarch64 lose their dead trampoline asm in this commit too); `xcore/src/{config.rs:38, mm/init.rs:35-39+154, task/proc.rs:215-218, vdso/install.rs, vdso/resolve.rs}`. |
| Review | R-103 | Accepted  | V-UT-5 (in-crate Verdef test) replaced by V-UT-5' = host-side `scripts/check-vdso-verdef.sh` invoked at the tail of `make vdso-blob`. Catches the regression at build time, before `make run-tests`.                  |
| Review | R-104 | Accepted  | Added C-14: `VdsoData` is a single global `'static` instance (kernel-side); user-side mapping is by-phys-addr via `map_linear` — same primitive currently used for `SIGNAL_TRAMPOLINE`. Only the *code* page is alloc-backed-and-copied per process. Architecture pseudocode + G-3 + C-10 rewritten.   |
| Review | R-105 | Accepted  | Added C-15: `make build`, `make clippy`, `make rv`, `make la`, `make vf2` gain a `vdso-blob` prerequisite (in `scripts/make/build.mk` + top-level Makefile). Kernel's `build.rs` panics on missing blob with a "run `make vdso-blob ARCH=…` first" message. Direct `cargo build` documented as unsupported. |

---

## Spec `Core specification`

[**Goals**]

- G-1: A new crate `xmodules/xvdso/` (excluded from the root workspace) builds two per-arch position-independent ELF blobs (`vdso-riscv64.so`, `vdso-loongarch64.so`) using the pinned toolchain (`nightly-2026-03-15`). Built as `cdylib` with a per-arch linker script that places `.text`, `.note.linux`, `.eh_frame_hdr`, `.eh_frame`, `.dynamic`, and `.dynsym`/`.dynstr`/`.gnu.version*` into a single PT_LOAD segment fitting in 8 KiB. Produced ELFs export the symbols listed under `[**API Surface**]` with the `__vdso_*` and `__kernel_*` aliases, plus a versioned `LINUX_2.6` `Verdef` (built via an explicit `--version-script linker/vdso-version.lds`).

- G-2: A separate `xmodules/xvdso-data/` crate (also excluded; pure `no_std`, no `cdylib`) defines `VdsoData` once. Both `xvdso` (user side) and `xcore::vdso` (kernel side) depend on it via a path dependency. The kernel root crate `include_bytes!`-es the correct per-arch vDSO blob via `cfg(target_arch=…)` selection from a known path under `target/vdso/`, written by `make vdso-blob`. `xcore::vdso::image()` returns `&'static [u8]`.

- G-3: On `execve` (`xcore::mm::init::load_app`), the kernel maps three regions inside the user address space, in this order, **after** `unmap_user_areas` has run on non-init paths:
  1. **vDSO data page** at `USER_VDSO_DATA` (R-only to user): a single kernel-resident `'static VdsoData` page mapped by-phys-addr via `AddrSpace::map_linear` (the same primitive currently used for `SIGNAL_TRAMPOLINE`). The kernel writes the data page through its own kernel-virtual alias; every user process reads the **same physical page**. This is required for the seqlock contract — a single-writer/multi-reader scheme cannot work if each process has its own copy (C-14).
  2. **vDSO code page(s)** at `USER_VDSO_BASE` (R-X to user): 1–2 4 KiB pages backed by `map_alloc`, written from `image()` while still W-able, then re-protected to R-X via `AddrSpace::protect` (`arceos/modules/axmm/src/aspace.rs:433`). Per-process alloc is fine for the code page — it's read-only at runtime, so duplication is just a TLB-coherence convenience, not a correctness requirement.
  3. The `AT_SYSINFO_EHDR` auxv entry is set to the code page base.

- G-4: `__vdso_clock_gettime(clock_id, *timespec)`, `__vdso_gettimeofday(*timeval, *tz)`, `__vdso_clock_getres(clock_id, *timespec)`, and `__vdso_time(*time_t)` serve `CLOCK_REALTIME`, `CLOCK_MONOTONIC`, and `CLOCK_MONOTONIC_RAW` entirely from the data page using `rdtime` (RV64) / `rdtime.d` (LA64), without trapping. Unsupported clocks fall through to a syscall via the trap instruction (`ecall` on RV, `syscall 0` on LA), returning the syscall's result unchanged. `__vdso_clock_getres` returns `1 ns` for the supported clocks (matching `xapi/src/sys/time.rs:71`), syscall fallback otherwise. `__vdso_time` returns `data.wall_sec` directly. `__vdso_getcpu` is exported for ABI parity but currently returns `-ENOSYS`; SMP correctness deferred to a follow-up (NG-2).

- G-5: `__vdso_rt_sigreturn` executes the trap instruction with `nr = NR_rt_sigreturn` (139 on RV64, 139 on LA64). The kernel resolves its absolute address by parsing the embedded vDSO ELF's `.dynsym` once at boot (cached as `XCORE_VDSO_RT_SIGRETURN_OFFSET: AtomicUsize` in `xcore::vdso`) and adding the per-process vDSO base on each `execve`. After `execve` finishes installing the vDSO, `xcore::vdso::install` calls `process.signal.set_default_restorer(absolute_addr)`. Signal-frame writers in `xsignal` consume `default_restorer` exactly as they do today — no API change in the call path, only at the constructor + setter.

- G-6: `xcore::config::SIGNAL_TRAMPOLINE`, `xcore::mm::init::map_trampoline`, `xsignal::arch::signal_trampoline_address()`, and the `signal_trampoline` assembly that backs it are all removed in **one** commit (Phase 4) atomic with the vDSO base shift to `0x4001_0000` and the `set_default_restorer` wiring. No intermediate commit references a deleted symbol or aliases the same VA from two mappings.

- G-7: Test rootfs gains four first-party C tests under `xtest/c/`, each one `.c` → one statically-linked ELF picked up by `run-c.sh` per `specs/features/redesign-xtest/SPEC.md`'s staging contract:
  - `vdso_clock_monotonic.c` — 1,000,000 `clock_gettime(CLOCK_MONOTONIC)` reads, asserts monotonicity + bounded total elapsed time.
  - `vdso_gettimeofday.c` — `gettimeofday` agrees with `clock_gettime(CLOCK_REALTIME)` to within 100 ms.
  - `vdso_clock_getres.c` — `clock_getres(CLOCK_MONOTONIC)` returns `{0, 1}`.
  - `vdso_rt_sigreturn.c` — installs `SIGUSR1` handler, raises it, returns from handler; verifies process did not crash and a flag is set.
  - `vdso_rdtime_smoke.c` (Phase 0 only) — emits `rdtime`/`rdtime.d` from U-mode and prints the value; runs at task boot during the spike, then is **deleted** before Phase 5 ships if both arches succeed (it's a feasibility gate, not a regression test).

- G-8: All LTP-style cases in `src/init.sh` that touch covered surface — `clock_gettime02`, `kill06`, `kill11`, `signal02`, `signal03`, `signal04`, `signal05`, `tkill01` — pass on both `make rv` and `make la`. `make build ARCH={riscv64,loongarch64}` succeeds; both kernels boot to userspace via `make rv` / `make la`. The new C tests above pass under `make run-tests ARCH={riscv64,loongarch64}`.

- NG-1: vDSO ASLR / per-process randomized base. v1 maps the vDSO at a fixed VA per-process. Randomization deferred.
- NG-2: `getcpu` correctness on SMP. ABI exported but stub returns `-ENOSYS`. Per-CPU mapping deferred.
- NG-3: vDSO build outside the pinned toolchain.
- NG-4: User-readable HW counter on platforms beyond `riscv64-qemu-virt`, `riscv64-visionfive2`, `loongarch64-qemu-virt`. The Phase 0 spike covers all three.
- NG-5: x86_64 / aarch64 vDSO. Out of scope.
- NG-6: Compatibility with statically-linked binaries that ignore auxv — they continue to syscall.

[**Architecture**]

```
xmodules/xvdso-data/                (NEW; workspace-excluded; no_std; pure data layout)
├── Cargo.toml
└── src/lib.rs                      #[repr(C, align(8))] struct VdsoData

xmodules/xvdso/                     (NEW; workspace-excluded; cdylib for *-unknown-none)
├── Cargo.toml                      cdylib, no_std, panic=abort; depends on xvdso-data (path)
├── build.rs                        per-arch link with linker scripts + version script
├── src/
│   ├── lib.rs                      #![no_std] entry; cfg per-arch
│   ├── time.rs                     seqlock read; clock dispatch
│   ├── arch/
│   │   ├── riscv64.rs              rdtime; ecall fallback; rt_sigreturn asm
│   │   └── loongarch64.rs          rdtime.d; syscall fallback; rt_sigreturn asm
│   └── exports.rs                  #[no_mangle] __vdso_* / __kernel_*
└── linker/
    ├── vdso-riscv64.lds
    ├── vdso-loongarch64.lds
    └── vdso-version.lds            VERSION { LINUX_2.6 { global: __vdso_*; local: *; }; }

xcore/src/vdso/                     (NEW module inside xcore; depends on xvdso-data path)
├── mod.rs                          fn image(), fn install(uspace, proc) -> AxResult<VdsoBinding>
├── data.rs                         re-export VdsoData; kernel-side seqlock writer
├── blob.rs                         include_bytes! the per-arch blob from target/vdso/
├── tick.rs                         vdso_tick(): boot-CPU-only timer-ISR refresher
└── resolve.rs                      one-shot ELF .dynsym parse for rt_sigreturn offset

xcore/src/mm/init.rs                EDIT: load_app maps vDSO + data page; auxv += AT_SYSINFO_EHDR
xcore/src/config.rs                 EDIT (Phase 4 only): drop SIGNAL_TRAMPOLINE; add USER_VDSO_BASE, USER_VDSO_DATA
xmodules/xsignal/src/api/process.rs EDIT: default_restorer becomes AtomicUsize; add set_default_restorer
xmodules/xsignal/src/arch/*.rs      EDIT (Phase 4 only): remove signal_trampoline asm & signal_trampoline_address
xcore/src/task/proc.rs              EDIT: ProcessSignalManager::new(actions, 0); xcore::vdso::install fills it later
arceos/crates/kernel_elf_parser     EDIT: pub const AUXV_LEN: usize = 18; auxv_vector / map_elf use it
Makefile + scripts/make/*.mk        EDIT: new vdso-blob target invoking cargo with --manifest-path
Cargo.toml (root)                   EDIT: workspace exclude += ["xmodules/xvdso", "xmodules/xvdso-data"]

xtest/c/{vdso_clock_monotonic,vdso_gettimeofday,vdso_clock_getres,vdso_rt_sigreturn}.c   NEW
xtest/c/vdso_rdtime_smoke.c        NEW (Phase 0 only; deleted at Phase 5)
```

End-to-end flow:

```
boot:
  Phase 0 spike artifact runs once:  vdso_rdtime_smoke prints rdtime → if both arches pass, design holds
  axhal::time init  ─────►  vdso_tick() seeds VdsoData{ wall_sec, mono_ns_offset, mult, shift, seq=2 }
  timer_irq         ─────►  if cpu_id() == BOOT_CPU { vdso_tick() }   // SMP guard (R-010)

execve (load_app):
  if !init {
      uspace.unmap_user_areas()
      // NOTE: map_trampoline removed in Phase 4 (so this whole block also vanishes)
  }
  map ELF segments
  map heap
  map ustack
  ── NEW ────────────
  map vDSO data page (R)        @ USER_VDSO_DATA   // map_linear → shared phys page (one VdsoData global)
  map vDSO code  page(s) (R-X)  @ USER_VDSO_BASE   // map_alloc + write + protect → per-process copy
  auxv[AT_SYSINFO_EHDR] = USER_VDSO_BASE
  proc.vdso = VdsoBinding { base: USER_VDSO_BASE, rt_sigreturn: USER_VDSO_BASE + RT_SIGRETURN_OFFSET }
  proc.signal.set_default_restorer(proc.vdso.rt_sigreturn.as_usize())

userspace fast path (no trap):
  glibc/musl ld.so reads AT_SYSINFO_EHDR → resolves __vdso_clock_gettime via Verdef LINUX_2.6
  app calls clock_gettime(CLOCK_MONOTONIC, &ts)
    └─ libc → __vdso_clock_gettime
              └─ seqlock-read VdsoData
                  └─ now = (rdtime() * mult) >> shift + mono_ns
              return 0

signal:
  kernel delivery: user_frame.pretcode = proc.signal.default_restorer.load()    // was SIGNAL_TRAMPOLINE
  user handler returns to pretcode → __vdso_rt_sigreturn → trap → kernel restores
```

[**Mult/Shift Derivation**] (R-007)

`mult` and `shift` convert raw counter `delta` to nanoseconds:

```
delta_ns = (delta * mult) >> shift
```

For a counter frequency `f_hz`:

```
mult  = ((NANOS_PER_SEC as u64) << shift) / f_hz
```

`shift = 24` (Linux convention; gives ≥32 bits of precision for `f_hz` in `[1 MHz, 4 GHz]`). At boot:

- The kernel surface is `axconfig::devices::TIMER_FREQUENCY`, populated per-platform (see `arceos/modules/axhal/src/platform/riscv64_qemu_virt/time.rs:3` and the LA mirror).
- This iteration adds a thin re-export `pub fn axhal::time::timer_frequency() -> u64` returning `axconfig::devices::TIMER_FREQUENCY as u64`. `xcore::vdso::tick` consumes the re-export — `xcore::vdso` does not import `axconfig` directly. The new accessor is part of Phase 3's diff.
- For both arches the value is filled at platform init time (LA reads `cpucfg` and stashes; RV bakes the dtb `/cpus.timebase-frequency` value at `axconfig` build time).

`vdso_tick()` re-captures `mono_cycles_at_capture` and recomputes `mono_ns` once per timer tick (≥100 Hz). The maximum delta between captures is `1 / 100 s = 10 ms`. For `f_hz = 10 MHz`, `delta_max = 100_000`; `delta * mult` with `mult ≤ 1.7e11` fits in u64 by 5 orders of magnitude — no overflow.

Compile-time check: `const _: () = assert!((u64::MAX / mult_for(MAX_FREQ_HZ)) >= MAX_DELTA_PER_TICK);`.

[**Data Structure**]

```rust
// xmodules/xvdso-data/src/lib.rs   (single source of truth, shared by xvdso AND xcore::vdso)
#![no_std]

use core::sync::atomic::AtomicU32;

#[repr(C, align(8))]
pub struct VdsoData {
    /// Seqlock counter. Even = stable; odd = writer in progress.
    pub seq: AtomicU32,
    /// CPU id (v1: always 0; per-CPU correctness deferred).
    pub cpu: u32,

    /// Wall-clock seconds at the point captured by `mono_cycles_at_capture`.
    pub wall_sec: u64,
    /// Wall-clock nanoseconds (sub-second).
    pub wall_nsec: u32,
    /// Reserved (alignment will pad to 8 anyway; named field for clarity).
    pub _reserved0: u32,

    /// Monotonic nanoseconds at capture.
    pub mono_ns: u64,
    pub mono_cycles_at_capture: u64,
    pub mult: u32,
    pub shift: u32,
}

const _: () = assert!(core::mem::size_of::<VdsoData>() <= 4096);
const _: () = assert!(core::mem::align_of::<VdsoData>() == 8);

// xcore/src/vdso/mod.rs
pub fn image() -> &'static [u8];               // include_bytes! result
pub fn install(uspace: &mut AddrSpace, proc: &XProcess) -> AxResult<VdsoBinding>;
pub fn data_writer() -> &'static VdsoDataWriter;

pub struct VdsoBinding {
    pub base: VirtAddr,
    pub rt_sigreturn: VirtAddr,
}

// xcore::vdso::resolve  (parsed once at first install; cached)
pub fn rt_sigreturn_offset() -> usize;          // returns the offset within image()

// xmodules/xsignal/src/api/process.rs (revised)
pub struct ProcessSignalManager<M, WQ> {
    // ... existing fields ...
    default_restorer: AtomicUsize,
}
impl<M: RawMutex, WQ: WaitQueue> ProcessSignalManager<M, WQ> {
    pub fn new(actions: Arc<Mutex<M, SignalActions>>, default_restorer: usize) -> Self { ... }
    pub fn set_default_restorer(&self, addr: usize) { self.default_restorer.store(addr, Release); }
    pub(crate) fn default_restorer(&self) -> usize { self.default_restorer.load(Acquire) }
}
```

[**API Surface**]

```rust
// vDSO exports (xmodules/xvdso/src/exports.rs); aliases via linker version script
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_clock_gettime(clock_id: i32, ts: *mut Timespec) -> i32;
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_gettimeofday(tv: *mut Timeval, tz: *mut c_void) -> i32;
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_clock_getres(clock_id: i32, res: *mut Timespec) -> i32;
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_time(tloc: *mut i64) -> i64;
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_getcpu(cpu: *mut u32, node: *mut u32, tcache: *mut c_void) -> i32; // returns -ENOSYS in v1
#[unsafe(naked)]    pub unsafe extern "C" fn __vdso_rt_sigreturn();
// vdso-version.lds installs LINUX_2.6 aliases:
//   __kernel_clock_gettime, __kernel_gettimeofday, __kernel_rt_sigreturn,
//   __kernel_clock_getres,  __kernel_getcpu

// xcore::vdso public API
pub fn image() -> &'static [u8];
pub fn install(uspace: &mut AddrSpace, proc: &XProcess) -> AxResult<VdsoBinding>;
pub fn data_writer() -> &'static VdsoDataWriter;
pub fn rt_sigreturn_offset() -> usize;

impl VdsoDataWriter {
    pub fn refresh(&self, mono_cycles: u64, mono_ns: u64, wall_sec: u64, wall_nsec: u32);
}

// kernel_elf_parser
pub const AUXV_LEN: usize = 18;
pub fn auxv_vector(&self, pagesz: usize) -> [AuxvEntry; AUXV_LEN]; // updated signature
// xcore::mm::init
pub fn map_elf(uspace: &mut AddrSpace, elf: &ElfFile) -> AxResult<(VirtAddr, [AuxvEntry; AUXV_LEN])>;
```

[**Constraints**]

- C-1: vDSO blob size ≤ 8 KiB on both arches; `image().len() <= 2 * PAGE_SIZE_4K` asserted at compile time on the kernel side.
- C-2: `VdsoData` is `#[repr(C, align(8))]`, single source of truth in `xmodules/xvdso-data`. Layout asserts compile-time.
- C-3: vDSO code is `no_std`, `panic = "abort"`, no allocation, no global mutable state outside the data page.
- C-4: vDSO accesses only registers, the data page (`USER_VDSO_DATA`), and its own code page (`USER_VDSO_BASE`).
- C-5: Seqlock invariant — `seq` even ⇒ stable. **Single writer**: only the boot CPU's timer ISR calls `vdso_tick()`; secondary CPUs short-circuit. Writer disables interrupts around the two `seq` increments. Multi-reader is unbounded.
- C-6: `kernel_elf_parser` exports `pub const AUXV_LEN: usize = 18;` Both `auxv_vector(...) -> [AuxvEntry; AUXV_LEN]` (`info.rs:145`) and `xcore::mm::init::map_elf(...) -> AxResult<(VirtAddr, [AuxvEntry; AUXV_LEN])>` (`mm/init.rs:44`) reference the constant. No literal `17` survives in either crate.
- C-7: vDSO is position-independent (`-fpic` via Rust default for `cdylib`).
- C-8: vDSO has no relocations except `R_*_RELATIVE`. Stripped of debug + symbols (other than `.dynsym`).
- C-9: User-mode `rdtime` (RV64) and `rdtime.d` (LA64) **must** be trap-free for: `riscv64-qemu-virt`, `riscv64-visionfive2`, `loongarch64-qemu-virt`. Required hw bits per arch:
  - **RV64**: `mcounteren.tm = 1` (set by OpenSBI fw_jump on `riscv64-qemu-virt` and on `riscv64-visionfive2` per the StarFive U-Boot SBI shim — verified by spike).
  - **LA64**: User-readable stable counter is the LA architectural baseline; QEMU `loongarch64-virt` exposes it unconditionally.
  Phase 0 spike validates all three. If any fails, that target's vDSO replaces its time entries with the syscall-fallback path inside the vDSO code (still no auxv change; signal path unaffected).
- C-10: vDSO mapping runs **after** `unmap_user_areas`, before `map_elf`, on every `execve`. Init path also maps the vDSO. **Code page**: `map_alloc(... R|W|U)` → `uspace.write(image_bytes)` → `AddrSpace::protect(... R|X|U)` — same trick `map_elf` uses for `.rodata`. **Data page**: `map_linear(USER_VDSO_DATA, virt_to_phys(&VDSO_DATA), 4096, R|U)` — by-phys-addr, sharing the single kernel-resident `VdsoData` global with all processes.
- C-11: `SIGNAL_TRAMPOLINE` removal and vDSO `rt_sigreturn` wiring land in **one** Phase 4 commit. This commit also shifts `USER_VDSO_BASE` from the transitional `0x4002_0000` (used in Phase 2) to the final `0x4001_0000`. No commit ever has both mappings claiming the same VA.
- C-12: vDSO build does not require Docker. `make vdso-blob ARCH=riscv64` runs `cargo build -Z build-std=core --manifest-path xmodules/xvdso/Cargo.toml --target riscv64imac-unknown-none-elf --release`; same for LA. Output: `target/vdso/<arch>/release/libxvdso.so`. The kernel's `build.rs` `include_bytes!`-es from there.
- C-13: `xmodules/xvdso` and `xmodules/xvdso-data` are listed in the root `Cargo.toml`'s `exclude = [...]`. They are **not** members; `cargo --workspace`, `cargo test --workspace`, and `make clippy` do not see them. The `xmodules/*` glob in `members` is preserved.
- C-14: `VdsoData` lives as a single `'static` instance in `xcore::vdso::data` (kernel side); all user-space mappings of `USER_VDSO_DATA` resolve to the **same physical page** via `map_linear`. The `VdsoDataWriter` writes through the kernel-virtual alias of that page; readers in every process see updates. This is what makes the seqlock + boot-CPU single-writer scheme correct.
- C-15: The kernel build has a hard dependency on the vDSO blob existing on disk under `target/vdso/<arch>/release/libxvdso.so`. `make build`, `make clippy`, `make rv`, `make la`, `make vf2` carry `vdso-blob` as a Make prerequisite. The kernel root crate's `build.rs` also emits `cargo:rerun-if-changed=$VDSO_BLOB` and panics with `panic!("vDSO blob missing — run `make vdso-blob ARCH={arch}` first")` if the file is absent. Direct `cargo build` from the kernel side without the blob is unsupported and will fail with that message.

## Runtime `runtime logic`

[**Main Flow — clock_gettime fast path**]

1. App calls `libc::clock_gettime(CLOCK_MONOTONIC, &ts)`.
2. libc dispatches to `__vdso_clock_gettime` (resolved via `AT_SYSINFO_EHDR` + `LINUX_2.6` Verdef).
3. vDSO body:
   a. `seq1 = data.seq.load(Acquire)`.
   b. If `seq1 & 1`, retry (likely just done; rare).
   c. Read `wall_sec`, `mono_ns`, `mono_cycles_at_capture`, `mult`, `shift`.
   d. `seq2 = data.seq.load(Acquire)`. If `seq2 != seq1` or `seq2 & 1`, retry.
   e. `delta = rdtime() − mono_cycles_at_capture`.
   f. `delta_ns = (delta * mult) >> shift`.
   g. `now_ns = mono_ns + delta_ns` (or `wall_sec * 1e9 + wall_nsec + delta_ns` for `CLOCK_REALTIME`).
   h. Write `ts->tv_sec`, `ts->tv_nsec`. Return 0.

[**Main Flow — rt_sigreturn**]

1. Kernel signal delivery writes `user_frame.pretcode = proc.signal.default_restorer()`, sets PC = handler.
2. Handler returns → PC = `__vdso_rt_sigreturn`.
3. vDSO: `li a7, 139; ecall` (RV64) or `li $a7, 139; syscall 0` (LA64).
4. Kernel `sys_rt_sigreturn` restores ucontext.

[**Failure Flow**]

1. Unsupported clock id → vDSO falls through to in-vDSO trap instruction (`ecall`/`syscall 0`); kernel returns standard syscall result.
2. Phase 0 spike fails for some arch → that arch's vDSO time entries always trap (still ABI-compatible; no perf win for that arch).
3. Seqlock writer pre-empted → cannot happen (single writer in boot-CPU ISR with IRQ disabled in the increment window).
4. Auxv array overflow → caught at compile time by `const _: () = assert!(AUXV_LEN >= 18);`.
5. ELF parse for rt_sigreturn offset fails → `xcore::vdso::install` panics at boot (the blob is part of the kernel; this is a build-time failure).

[**State Transitions**]

- Boot → `xcore::vdso::init()` runs Phase 0 spike, then parses the embedded ELF for `__vdso_rt_sigreturn` offset (cached), seeds `VdsoData` from `axhal::time`, registers `vdso_tick` on the boot-CPU timer ISR.
- `execve` → `xcore::vdso::install(uspace, proc)` maps both pages, returns `VdsoBinding`, calls `proc.signal.set_default_restorer(rt_sigreturn_address.as_usize())`.
- Timer ISR (boot CPU) → `data_writer().refresh(...)` updates `VdsoData`.
- Signal delivery → `proc.signal.default_restorer()` → vDSO trap → `sys_rt_sigreturn`.

## Implementation `split task into phases`

[**Phase 0 — Spike: validate user-mode counter access (C-9)**]

- Add `xtest/c/vdso_rdtime_smoke.c` printing `rdtime` (RV64) / `rdtime.d` (LA64) from U-mode.
- Run via `make run-tests ARCH=riscv64` and `make run-tests ARCH=loongarch64`; manually verify via `make vf2` if the board is reachable, otherwise rely on the QEMU vf2 emulation path.
- Verify the per-arch hw bits documented in C-9.
- If both QEMU arches pass → proceed to Phase 1. If vf2 fails, file as a known limitation and revisit before promoting `riscv64-visionfive2` to "vDSO-fast" — task ships v1 with vf2 in syscall-fallback mode if needed.

[**Phase 1 — vDSO crate skeleton + blob pipeline (G-1, G-2, C-1, C-3, C-7, C-8, C-12, C-13)**]

- Create `xmodules/xvdso-data/` (no_std lib, no cdylib) with `VdsoData` only.
- Create `xmodules/xvdso/` with `Cargo.toml` (`crate-type = ["cdylib"]`, `panic = "abort"`, `lto = true`, `opt-level = 3`, `strip = true`); depends on `xvdso-data` via path.
- Add per-arch `linker/vdso-{rv,la}.lds` and `linker/vdso-version.lds` with `LINUX_2.6` block.
- `build.rs`: emit linker flags `-Tlinker/vdso-<arch>.lds`, `--version-script=linker/vdso-version.lds`, `-soname=linux-vdso.so.1`, `--build-id=none`, `--no-undefined`.
- Add `Cargo.toml` `exclude = ["xmodules/xvdso", "xmodules/xvdso-data"]` at root.
- Add Makefile target `vdso-blob` that runs cargo with `--manifest-path` for each arch, drops output at `target/vdso/<arch>/release/libxvdso.so`, then runs `scripts/check-vdso-verdef.sh $BLOB` to assert the `LINUX_2.6` Verdef block (V-UT-5').
- Wire `vdso-blob` as a prerequisite of `build`, `clippy`, `rv`, `la`, `vf2` in `scripts/make/build.mk` / top-level Makefile (C-15).
- Stub all entries to syscall-fallback (no seqlock yet).
- Kernel-side `xcore::vdso::blob` `include_bytes!`-es per `cfg(target_arch)`. Root crate `build.rs` panics with the C-15 message if the blob is missing.
- Kernel-side compile-time size assert.
- V-UT-3 + V-UT-5' (size + Verdef) green here.

[**Phase 2 — kernel mapping at transitional VA + auxv (G-3, C-2, C-6, C-10)**]

- `USER_VDSO_BASE = 0x4002_0000`, `USER_VDSO_DATA = 0x4002_1000` (pre-existing `SIGNAL_TRAMPOLINE` at `0x4001_0000` untouched).
- Implement `xcore::vdso::install(uspace, proc)` (mapping only; no `set_default_restorer` yet).
- Wire into `load_app` after `unmap_user_areas` and before `map_elf`.
- Widen `kernel_elf_parser` auxv to `AUXV_LEN = 18`. Update `map_elf` return type. Append `AT_SYSINFO_EHDR`.
- Run `make build` for both arches; run V-IT-5, V-IT-6.
- At end of phase: `SIGNAL_TRAMPOLINE` still mapped at `0x4001_0000`; vDSO mapped at `0x4002_0000`. No conflict.

[**Phase 3 — time data page + fast path (G-4, C-2, C-5, C-14, R-007, R-101)**]

- Add `pub fn axhal::time::timer_frequency() -> u64` re-exporting `axconfig::devices::TIMER_FREQUENCY` (see `arceos/modules/axhal/src/time.rs:15` for the existing re-export style).
- Allocate `static VDSO_DATA: VdsoData` in `xcore::vdso::data`. Switch the data-page mapping in `xcore::vdso::install` from the Phase-2 placeholder (per-process `map_alloc`) to `map_linear(USER_VDSO_DATA, virt_to_phys(&VDSO_DATA), PAGE_SIZE_4K, R|U)`.
- Implement kernel-side `VdsoDataWriter` with the mult/shift derivation; writer disables IRQs around the two `seq.fetch_add(1, Release)` increments.
- Hook `vdso_tick()` into the timer ISR; guard with `if !axhal::cpu::this_cpu_is_bsp() { return; }` (per `arceos/modules/axhal/src/cpu.rs:21`).
- Replace stub time entries in xvdso with seqlock-read fast paths.
- Run V-UT-1, V-UT-2, V-UT-4, V-IT-1, V-IT-2, V-IT-4, V-F-1, V-F-2, V-F-4.

[**Phase 4 — rt_sigreturn migration & SIGNAL_TRAMPOLINE removal (G-5, G-6, C-11, R-102) — ONE COMMIT**]

This phase is one atomic commit. Enumerated diff (every file the executor must edit):

| File                                                  | Edit                                                                                                                                                       |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `xmodules/xsignal/src/api/process.rs`                 | `default_restorer: usize` → `AtomicUsize`. Add `set_default_restorer(&self, addr: usize)` and `pub(crate) fn default_restorer(&self) -> usize`. `new` body wraps the input in `AtomicUsize::new`. |
| `xmodules/xsignal/src/api/thread.rs:104-106`          | `self.proc.default_restorer` (field) → `self.proc.default_restorer()` (method). `restorer = action.restorer.map_or(self.proc.default_restorer(), |f| f as _);` |
| `xmodules/xsignal/src/arch/riscv.rs`                  | Delete the `.global signal_trampoline` asm and `signal_trampoline_address()`. (Note: filename is `riscv.rs`, not `riscv64.rs`.)                            |
| `xmodules/xsignal/src/arch/loongarch64.rs`            | Same — delete trampoline asm + accessor.                                                                                                                    |
| `xmodules/xsignal/src/arch/x86_64.rs`                 | Same — even though x86_64 is not in the root build, keep the per-arch tree symmetric and avoid dead-code skew.                                              |
| `xmodules/xsignal/src/arch/aarch64.rs`                | Same.                                                                                                                                                       |
| `xmodules/xsignal/src/arch/mod.rs`                    | Delete the `signal_trampoline_address()` re-export.                                                                                                         |
| `xcore/src/config.rs:38`                              | Delete `SIGNAL_TRAMPOLINE`. Add `pub const USER_VDSO_BASE: usize = 0x4001_0000;` and `pub const USER_VDSO_DATA: usize = 0x4001_1000;` (or pick distinct slot above the code page if 1 page proves insufficient — confirmed by Phase 1 size-check). Update Phase-2's transitional `0x4002_0000`/`0x4002_1000` consts to the final values. |
| `xcore/src/mm/init.rs:34-42`                          | Delete `map_trampoline` (whole function).                                                                                                                   |
| `xcore/src/mm/init.rs:154`                            | Delete the `map_trampoline(uspace)?;` call site.                                                                                                            |
| `xcore/src/task/proc.rs:215-218`                      | `ProcessSignalManager::new(actions, crate::config::SIGNAL_TRAMPOLINE)` → `ProcessSignalManager::new(actions, 0)` (default until `xcore::vdso::install` fills it on `execve`). |
| `xcore/src/vdso/resolve.rs`                           | New: `pub fn rt_sigreturn_offset() -> usize` parses the embedded ELF's `.dynsym` once via `xmas_elf` and caches the result in `OnceCell`.                     |
| `xcore/src/vdso/install.rs` (or `mod.rs`)             | After mapping, compute `rt_sigreturn_addr = vdso_base + rt_sigreturn_offset()`, call `proc.signal.set_default_restorer(rt_sigreturn_addr.as_usize())`.        |
| `xmodules/xvdso/src/arch/{riscv64,loongarch64}.rs`    | Finalize `__vdso_rt_sigreturn` body: `li a7, 139; ecall` (RV64) / `li $a7, 139; syscall 0` (LA64).                                                          |

Post-phase verification: `git grep SIGNAL_TRAMPOLINE` returns zero hits across `xcore`, `xmodules`, and `src`. Both arches build (`make rv` / `make la`); V-IT-3, V-IT-5 pass.

[**Phase 5 — xtest integration + docs (G-7, G-8)**]

- Add the four C tests under `xtest/c/`.
- Delete `xtest/c/vdso_rdtime_smoke.c` (Phase 0 artifact).
- Run `make tests ARCH=…` and `make run-tests ARCH=…` for both arches; confirm `[PASS]` lines.
- Update `AGENTS.md` Testing section.
- Add `docs/StarryX/vdso.md` (layout, data page, AT_SYSINFO_EHDR, mult/shift derivation).

## Trade-offs `ask reviewer for advice`

- **T-1: How to resolve the rt_sigreturn absolute address.**
  - (a) Parse the embedded ELF at boot in `xcore::vdso` (current lean). *Adv.:* zero coupling between `xmodules/xsignal` and the vDSO; uses `xmas_elf` already in the workspace; cached after first call. *Disadv.:* O(symbols) at boot (negligible — < 10 symbols).
  - (b) Build-time `nm` table baked into the kernel as a generated source file. *Adv.:* zero runtime cost. *Disadv.:* extra build script, brittle if symbol names change.
  - **Lean: (a)**. Costs almost nothing at boot, keeps `xsignal` clean (TR-1).

- **T-2: Auxv array widening vs slice.**
  - (a) Widen `[AuxvEntry; 17] → [AuxvEntry; 18]` plus exported `pub const AUXV_LEN`. *Adv.:* one-line bump for future entries; no allocation. *Disadv.:* still array, still capped.
  - (b) Move to `&[AuxvEntry]`. *Adv.:* fully future-proof. *Disadv.:* touches the vendored crate's tests/examples.
  - **Lean: (a) with the const**. (TR-2)

- **T-3: Seqlock writer placement.**
  - (a) Boot-CPU timer ISR (lean). *Adv.:* simplest; matches Linux. *Disadv.:* IRQ context constraints (already satisfied — fixed-size struct, no alloc). Requires SMP guard.
  - (b) Kernel thread. *Adv.:* relaxed context. *Disadv.:* extra latency.
  - **Lean: (a) + boot-CPU guard** (TR-3).

- **T-4: vDSO build invocation + workspace placement.**
  - (a) `xmodules/xvdso/build.rs` self-builds; crate is workspace member. *Adv.:* one-step `make build`. *Disadv.:* `cargo --workspace` and `make clippy` break for `*-unknown-none` cdylib.
  - (b) Top-level `make vdso-blob` + workspace exclude. *Adv.:* zero workspace pollution; matches `apps`/page-table-multiarch precedent. *Disadv.:* `make build` must depend on `vdso-blob`.
  - **Lean: (b)**. (TR-4)

## Validation `test design`

[**Unit Tests**]

- V-UT-1: `xcore::vdso::data` — round-trip a `VdsoData` through the seqlock writer; assert reader sees consistent snapshots across 1,000,000 iterations under a stress thread.
- V-UT-2: `xvdso::time::compute_monotonic` — pure function, asserts `(delta * mult) >> shift + base` matches reference for representative `f_hz ∈ {10 MHz, 100 MHz, 1 GHz}`.
- V-UT-3: `xcore::vdso::image()` — `image().len() <= 2 * PAGE_SIZE_4K`; first 4 bytes are `\x7fELF`.
- V-UT-4: Layout test — `assert_eq!(size_of::<xvdso_data::VdsoData>(), <kernel-mirror layout>); offset_of!(VdsoData, wall_sec) % 8 == 0; offset_of!(VdsoData, mono_ns) % 8 == 0`.
- V-UT-5: (replaced) — see V-UT-5'.
- V-UT-5': Verdef sanity, host-side — `scripts/check-vdso-verdef.sh $BLOB` runs `llvm-readelf -V` and `grep`s for `LINUX_2.6` plus the three required symbol bindings (`__vdso_clock_gettime`, `__vdso_gettimeofday`, `__vdso_rt_sigreturn`). Invoked at the tail of `make vdso-blob`, so the build fails before any `make build` / `make run-tests` step if the Verdef block is missing or malformed.
- V-UT-6: `xcore::vdso::resolve` — given a known-good ELF, returns the correct symbol offset; given an ELF without `__vdso_rt_sigreturn`, returns an error.

[**Integration Tests**]

- V-IT-1: Run `clock_gettime02` (the `src/init.sh`-listed case). PASS on both arches under `make run-tests`.
- V-IT-2: Run the new `xtest/c/vdso_clock_monotonic` C test. PASS on both arches.
- V-IT-3: Run `kill06`, `kill11`, `signal02`, `signal03`, `signal04`, `signal05`, `tkill01` (the `src/init.sh`-listed signal cases) plus the new `xtest/c/vdso_rt_sigreturn`. PASS on both arches — exercises the new vDSO `rt_sigreturn` path on every signal delivery.
- V-IT-4: Run new `xtest/c/vdso_gettimeofday` and `xtest/c/vdso_clock_getres`. PASS.
- V-IT-5: Boot smoke — `make rv` and `make la` reach userspace shell.
- V-IT-6: A C test prints auxv and asserts `AT_SYSINFO_EHDR` present, value within `[USER_VDSO_BASE, USER_VDSO_BASE + image_len)`.

[**Failure / Robustness Validation**]

- V-F-1: Unsupported clock id (`CLOCK_PROCESS_CPUTIME_ID`) returns the syscall result unchanged.
- V-F-2: Seqlock contention — userspace test calls `clock_gettime` in a tight loop while the kernel is under heavy timer pressure (drives `vdso_tick` rapidly via a `#[cfg(feature = "vdso-stress")]` debug feature); zero monotonicity violations.
- V-F-3: vDSO disabled (debug-only feature) — time LTP cases still pass via syscall fallback (proves additivity).
- V-F-4: SMP correctness — kernel-only test pins `vdso_tick` invocations onto two cores in a stress harness; assert no torn read on a userspace reader. (Currently runs only when SMP > 1; vf2 default `SMP=2`.)

[**Edge Case Validation**]

- V-E-1: `clock_gettime(CLOCK_MONOTONIC, NULL)` returns `EFAULT` (covered by `clock_gettime02`).
- V-E-2: `__vdso_rt_sigreturn` invoked outside signal context — kernel's existing `sys_rt_sigreturn` validation handles it; no new behavior.
- V-E-3: First execve on init — vDSO mapping must succeed on a fresh address space; covered by V-IT-5.
- V-E-4: Two processes simultaneously execute V-IT-2 (no shared mutable state in vDSO; data page is R-only to user).

[**Acceptance Mapping**]

| Goal / Constraint | Validation                                    |
| ----------------- | --------------------------------------------- |
| G-1               | V-UT-3, V-UT-5'                               |
| G-2               | V-UT-3, V-UT-4                                |
| G-3               | V-IT-5, V-IT-6                                |
| G-4               | V-IT-1, V-IT-2, V-IT-4, V-F-1                 |
| G-5               | V-IT-3, V-UT-6                                |
| G-6               | V-IT-3, V-IT-5 (no `SIGNAL_TRAMPOLINE` left)  |
| G-7               | V-IT-2, V-IT-3, V-IT-4                        |
| G-8               | V-IT-1, V-IT-3, V-IT-5                        |
| C-1               | V-UT-3                                        |
| C-2               | V-UT-4                                        |
| C-5               | V-UT-1, V-F-2, V-F-4                          |
| C-6               | V-IT-6 (auxv carries AT_SYSINFO_EHDR)         |
| C-9               | Phase 0 spike (gating)                        |
| C-10              | V-IT-5, V-E-3                                 |
| C-11              | V-IT-3, V-IT-5                                |
| C-13              | `make clippy` succeeds (V-IT-5 prerequisite)  |
| C-14              | V-F-2, V-F-4 (seqlock under contention proves shared-page semantics) |
| C-15              | V-IT-5 prerequisite — `make rv` / `make la` succeed only if `vdso-blob` ran first |
