# `vdso-support` PLAN `01`

> Status: Revised
> Feature: `vdso-support`
> Iteration: `01`
> Owner: Executor
> Depends on:
> - Previous Plan: `00_PLAN.md`
> - Review: `00_REVIEW.md`
> - Master Directive: none

---

## Summary

Iteration 01 addresses every blocking finding (R-001 .. R-006) and the load-bearing MEDIUMs (R-007, R-008, R-009, R-010) and one trimming LOW (R-013) from `00_REVIEW.md`. The macro-design is unchanged: a per-arch `cdylib` vDSO produced from `xmodules/xvdso/` (now a workspace-excluded crate built via a top-level `make vdso-blob`), embedded into the kernel as a per-arch byte blob, mapped into every user address space alongside a kernel-published seqlock-protected time data page, with `AT_SYSINFO_EHDR` set in auxv. Signal-return migrates from `SIGNAL_TRAMPOLINE` into the vDSO. The micro-design now spells out: how `ProcessSignalManager.default_restorer` becomes mutable per-`execve`, how the VA collision between `SIGNAL_TRAMPOLINE` and `USER_VDSO_BASE` is sequenced atomically inside one phase, how `mult/shift` are derived, what the linker/version script must produce for musl/glibc to actually call the symbols, and how SMP affects seqlock writers.

## Log

[**Added**]

- Phase 0 spike artifact (`xtest/c/vdso_rdtime_smoke.c`) and per-arch hw-bit list (R-006).
- `pub const AUXV_LEN: usize = 18;` exported by `kernel_elf_parser`; both `auxv_vector` and `map_elf` use it (R-003 / TR-2).
- `xsignal::api::ProcessSignalManager::set_default_restorer(&self, addr: usize)` backed by `AtomicUsize`, plus a `default_restorer: AtomicUsize` field replacing the current `usize` (R-001 / TR-1).
- `xcore::vdso::rt_sigreturn_address(&blob, base) -> VirtAddr`, computed at install time by parsing the embedded ELF's `.dynsym` via the existing `xmas_elf` dependency. `xsignal` is no longer involved in offset resolution (TR-1).
- Workspace exclusion: `xmodules/xvdso` is added to `Cargo.toml`'s `exclude` list, not `members` (R-002 / TR-4).
- `linker/vdso-version.lds` (cpp-free version script with the `LINUX_2.6` `VERSION { ... }` block) and a build-rs `--version-script` flag (R-008).
- `mult/shift derivation` subsection in `## Spec` (R-007).
- SMP guard: only the boot CPU's timer ISR calls `vdso_tick()` (R-010 / TR-3).
- `__vdso_getcpu` is dropped from G-4; auxv exposure remains but the symbol returns `ENOSYS` (R-013).
- New first-party C tests: `vdso_gettimeofday.c`, `vdso_clock_getres.c`, `vdso_rt_sigreturn.c`, `vdso_rdtime_smoke.c` (R-004).
- `USER_VDSO_BASE` retains the value `0x4001_0000`, but the cleanup is sequenced into a single Phase 4 commit so the address never aliases (R-005).

[**Changed**]

- `## Spec` is restated in full (deep-tier rule). Sections are not deltas.
- T-1 leans flipped: kernel-side ELF parse at install time wins (TR-1).
- T-2 lean unchanged but tightened with `AUXV_LEN` const (TR-2).
- T-3 lean unchanged but qualified with SMP guard (TR-3).
- T-4 lean flipped: top-level `make vdso-blob` + workspace exclusion wins (TR-4).
- V-IT-1..V-IT-3 LTP case lists corrected to match `src/init.sh` actually-present cases (R-004).
- Phase ordering revised: Phase 2 maps the vDSO at a *transitional* base `0x4002_0000`; Phase 4 is one atomic commit that (a) deletes `SIGNAL_TRAMPOLINE` + `map_trampoline`, (b) shifts `USER_VDSO_BASE` to `0x4001_0000`, (c) wires `set_default_restorer`. No transitional commit ever has both `SIGNAL_TRAMPOLINE` and a vDSO mapping fighting for the same VA (R-005).

[**Removed**]

- The `xsignal::arch::rt_sigreturn_offset()` API (R-009 / TR-1). Replaced by the kernel-side `xcore::vdso::rt_sigreturn_address`.
- `__vdso_getcpu` from the must-implement list (R-013). Symbol still exported (it's cheap and may help glibc-built guests in future) but currently returns `-ENOSYS`; `cpu` field stays in `VdsoData` for the eventual SMP follow-up.

[**Unresolved**]

- vDSO ASLR (NG-1, unchanged from 00).
- Per-CPU `getcpu` correctness (NG-2, unchanged).

[**Response Matrix**]

| Source | ID    | Decision  | Resolution                                                                                                                                                                                          |
| ------ | ----- | --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Review | R-001 | Accepted  | Add `set_default_restorer(&self, addr)` on `ProcessSignalManager`; field becomes `AtomicUsize`. `xcore::vdso::install` calls it after mapping. See G-5 + Implementation Phase 4.                    |
| Review | R-002 | Accepted  | `xmodules/xvdso` added to workspace `exclude`, not `members`. Built via top-level `make vdso-blob` invoking `cargo --manifest-path`. See Phase 1.                                                   |
| Review | R-003 | Accepted  | `pub const AUXV_LEN: usize = 18;` exported from `kernel_elf_parser`; `auxv_vector` and `map_elf` use it. C-6 restated to enumerate touched signatures.                                              |
| Review | R-004 | Accepted  | V-IT-1..V-IT-3 case lists corrected to actually-present `src/init.sh` cases. New first-party C tests fill the gaps for `gettimeofday`, `clock_getres`, `rt_sigreturn`. See Validation.              |
| Review | R-005 | Accepted  | Phase 2 uses transitional `USER_VDSO_BASE = 0x4002_0000`; Phase 4 is a single commit that deletes `SIGNAL_TRAMPOLINE` and shifts vDSO to `0x4001_0000`. Architecture pseudocode rewritten.          |
| Review | R-006 | Accepted  | Phase 0 spike specifies (a) the required hw bits per arch, (b) the artifact (`xtest/c/vdso_rdtime_smoke.c`), (c) `riscv64-visionfive2` in scope, (d) the syscall fallback path emits `ecall`/`syscall` directly inside the vDSO. |
| Review | R-007 | Accepted  | Added "mult/shift derivation" subsection.                                                                                                                                                            |
| Review | R-008 | Accepted  | Added `linker/vdso-version.lds` with `LINUX_2.6` block; `--version-script` build-rs flag; V-UT-5 asserts `Verdef` with `llvm-readelf -V`.                                                            |
| Review | R-009 | Accepted  | Resolution moves to `xcore::vdso::rt_sigreturn_address`; `xsignal` API change removed.                                                                                                              |
| Review | R-010 | Accepted  | C-5 amended: only boot CPU's timer ISR calls `vdso_tick()`. V-F-4 added.                                                                                                                            |
| Review | R-011 | Accepted  | `#[repr(C, align(8))]` on `VdsoData`; layout test added.                                                                                                                                              |
| Review | R-012 | Accepted  | `VdsoData` factored into a single `xmodules/xvdso-data` no_std crate shared by both kernel and user side; manual `_pad` fields dropped in favor of compiler-inserted padding under `align(8)`.        |
| Review | R-013 | Accepted  | `__vdso_getcpu` removed from G-4 must-list (musl Alpine doesn't call it); symbol exported but returns `-ENOSYS`. NG-2 unchanged.                                                                    |
| Review | TR-1  | Applied   | Kernel-side parse-at-install adopted; T-1 rewritten.                                                                                                                                                  |
| Review | TR-2  | Applied   | Widen + `AUXV_LEN` const; T-2 rewritten.                                                                                                                                                              |
| Review | TR-3  | Applied   | Boot-CPU guard added; T-3 rewritten.                                                                                                                                                                  |
| Review | TR-4  | Applied   | Top-level `make vdso-blob` + workspace exclude; T-4 rewritten.                                                                                                                                        |

---

## Spec `Core specification`

[**Goals**]

- G-1: A new crate `xmodules/xvdso/` (excluded from the root workspace) builds two per-arch position-independent ELF blobs (`vdso-riscv64.so`, `vdso-loongarch64.so`) using the pinned toolchain (`nightly-2026-03-15`). Built as `cdylib` with a per-arch linker script that places `.text`, `.note.linux`, `.eh_frame_hdr`, `.eh_frame`, `.dynamic`, and `.dynsym`/`.dynstr`/`.gnu.version*` into a single PT_LOAD segment fitting in 8 KiB. Produced ELFs export the symbols listed under `[**API Surface**]` with the `__vdso_*` and `__kernel_*` aliases, plus a versioned `LINUX_2.6` `Verdef` (built via an explicit `--version-script linker/vdso-version.lds`).

- G-2: A separate `xmodules/xvdso-data/` crate (also excluded; pure `no_std`, no `cdylib`) defines `VdsoData` once. Both `xvdso` (user side) and `xcore::vdso` (kernel side) depend on it via a path dependency. The kernel root crate `include_bytes!`-es the correct per-arch vDSO blob via `cfg(target_arch=…)` selection from a known path under `target/vdso/`, written by `make vdso-blob`. `xcore::vdso::image()` returns `&'static [u8]`.

- G-3: On `execve` (`xcore::mm::init::load_app`), the kernel maps three regions inside the user address space, in this order, **after** `unmap_user_areas` has run on non-init paths:
  1. **vDSO data page** at `USER_VDSO_DATA` (R-only to user, W-able to kernel via the kernel mirror): one 4 KiB page holding `VdsoData`.
  2. **vDSO code page(s)** at `USER_VDSO_BASE` (R-X to user): 1–2 4 KiB pages backed by alloc, written from `image()` while still W-able, then re-protected to R-X via `protect`.
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
  map vDSO data page (R)        @ USER_VDSO_DATA
  map vDSO code  page(s) (R-X)  @ USER_VDSO_BASE
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

- **RV64**: `f_hz` = `axhal::time::TIMEBASE_FREQ_HZ` (already populated by axhal from the dtb `/cpus.timebase-frequency`).
- **LA64**: `f_hz` = `axhal::time::TIMEBASE_FREQ_HZ` (axhal reads from `cpucfg`).

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
- C-10: vDSO mapping runs **after** `unmap_user_areas`, before `map_elf`, on every `execve`. Init path also maps the vDSO. The mapping function uses `map_alloc(... R|U)` then `uspace.write(image_bytes)` then `protect(... R|X|U)` — same trick `map_elf` already uses for read-only `.rodata`.
- C-11: `SIGNAL_TRAMPOLINE` removal and vDSO `rt_sigreturn` wiring land in **one** Phase 4 commit. This commit also shifts `USER_VDSO_BASE` from the transitional `0x4002_0000` (used in Phase 2) to the final `0x4001_0000`. No commit ever has both mappings claiming the same VA.
- C-12: vDSO build does not require Docker. `make vdso-blob ARCH=riscv64` runs `cargo build -Z build-std=core --manifest-path xmodules/xvdso/Cargo.toml --target riscv64imac-unknown-none-elf --release`; same for LA. Output: `target/vdso/<arch>/release/libxvdso.so`. The kernel's `build.rs` `include_bytes!`-es from there.
- C-13: `xmodules/xvdso` and `xmodules/xvdso-data` are listed in the root `Cargo.toml`'s `exclude = [...]`. They are **not** members; `cargo --workspace`, `cargo test --workspace`, and `make clippy` do not see them. The `xmodules/*` glob in `members` is preserved.

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
- Add Makefile target `vdso-blob` that runs cargo with `--manifest-path` for each arch, drops output at `target/vdso/<arch>/release/libxvdso.so`.
- Stub all entries to syscall-fallback (no seqlock yet).
- Kernel-side `xcore::vdso::blob` `include_bytes!`-es per `cfg(target_arch)`.
- Kernel-side compile-time size assert.
- V-UT-3 + V-UT-5 (size + Verdef) green here.

[**Phase 2 — kernel mapping at transitional VA + auxv (G-3, C-2, C-6, C-10)**]

- `USER_VDSO_BASE = 0x4002_0000`, `USER_VDSO_DATA = 0x4002_1000` (pre-existing `SIGNAL_TRAMPOLINE` at `0x4001_0000` untouched).
- Implement `xcore::vdso::install(uspace, proc)` (mapping only; no `set_default_restorer` yet).
- Wire into `load_app` after `unmap_user_areas` and before `map_elf`.
- Widen `kernel_elf_parser` auxv to `AUXV_LEN = 18`. Update `map_elf` return type. Append `AT_SYSINFO_EHDR`.
- Run `make build` for both arches; run V-IT-5, V-IT-6.
- At end of phase: `SIGNAL_TRAMPOLINE` still mapped at `0x4001_0000`; vDSO mapped at `0x4002_0000`. No conflict.

[**Phase 3 — time data page + fast path (G-4, C-2, C-5, R-007)**]

- Implement kernel-side `VdsoDataWriter` with the mult/shift derivation.
- Hook `vdso_tick()` into the boot-CPU timer ISR with `if cpu_id() != BOOT_CPU { return; }`.
- Replace stub time entries in xvdso with seqlock-read fast paths.
- Run V-UT-1, V-UT-2, V-UT-4, V-IT-1, V-IT-2, V-IT-4, V-F-1, V-F-2.

[**Phase 4 — rt_sigreturn migration & SIGNAL_TRAMPOLINE removal (G-5, G-6, C-11) — ONE COMMIT**]

This phase is one atomic commit. The diff:

- `xmodules/xsignal/src/api/process.rs`: `default_restorer: usize` → `AtomicUsize`; add `set_default_restorer`, `default_restorer()` accessor; `new(actions, default_restorer: usize)` builds the atomic.
- `xmodules/xsignal/src/arch/{riscv64,loongarch64}.rs`: delete `signal_trampoline` asm and `signal_trampoline_address()`.
- `xcore::vdso::resolve`: parse `image()` once at boot to find `__vdso_rt_sigreturn` offset; expose via `rt_sigreturn_offset()`.
- `xcore::vdso::install`: after mapping, call `proc.signal.set_default_restorer(rt_sigreturn_address.as_usize())`.
- `xcore::config`: delete `SIGNAL_TRAMPOLINE`, replace with `USER_VDSO_BASE = 0x4001_0000`, `USER_VDSO_DATA = 0x4001_1000`.
- `xcore::mm::init`: delete `map_trampoline` and the call site at line 154.
- `xcore::task::proc`: `ProcessSignalManager::new(actions, 0)` (default until vDSO install fills it).
- vDSO `__vdso_rt_sigreturn` body finalized.
- Run V-IT-3, V-IT-5; rebuild both arches; verify no `SIGNAL_TRAMPOLINE` reference remains via `grep -r SIGNAL_TRAMPOLINE`.

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
- V-UT-5: Verdef sanity — host test runs `llvm-readelf -V` on the produced blob; asserts `LINUX_2.6` Verdef block exists and binds `__vdso_clock_gettime`, `__vdso_gettimeofday`, `__vdso_rt_sigreturn`. Runs as part of `cargo test` for the `xvdso` crate via `[[test]] required-features` host-side wrapper.
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
| G-1               | V-UT-3, V-UT-5                                |
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
