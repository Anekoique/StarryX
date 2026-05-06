
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
