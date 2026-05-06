# `vdso-support` PLAN `00`

> Status: Draft
> Feature: `vdso-support`
> Iteration: `00`
> Owner: Executor
> Depends on:
> - Previous Plan: none
> - Review: none
> - Master Directive: none

---

## Summary

Introduce a Linux-compatible vDSO image (`linux-vdso.so.1`) into StarryX. A new workspace member `xmodules/xvdso/` builds a per-arch position-independent ELF (`riscv64`, `loongarch64`) exporting `__vdso_clock_gettime`, `__vdso_gettimeofday`, `__vdso_clock_getres`, `__vdso_time`, `__vdso_getcpu`, and `__vdso_rt_sigreturn`. The kernel embeds the per-arch ELFs via `include_bytes!`, maps them into every user address space on `execve` together with a kernel-published seqlock-protected time data page, and publishes the code page's ELF header in auxv as `AT_SYSINFO_EHDR`. The signal-return trampoline migrates from the current `SIGNAL_TRAMPOLINE` linear mapping (`xcore/src/config.rs:38`, `xcore/src/mm/init.rs:35`) into the vDSO; `xsignal` reads the trampoline address from a per-process record set at `execve` time.

## Log `None in 00_PLAN`

[**Added**] — n/a (initial iteration)
[**Changed**] — n/a
[**Removed**] — n/a
[**Unresolved**] — n/a
[**Response Matrix**] — n/a

---

## Spec `Core specification`

[**Goals**]

- G-1: A new workspace member `xmodules/xvdso/` builds two per-arch position-independent ELF blobs (`vdso-riscv64.so`, `vdso-loongarch64.so`) using the pinned toolchain (`nightly-2026-03-15`). Built as `cdylib` with a per-arch linker script that places `.text`, `.note`, `.eh_frame_hdr`, `.eh_frame`, and `.dynamic` into a single PT_LOAD segment fitting in 4 KiB. Produced ELFs export the symbols listed under `[**API Surface**]` with the `__vdso_*` and `__kernel_*` aliases that musl/glibc probe for, plus a versioned `LINUX_2.6` `DT_VERDEF`.

- G-2: The root crate `include_bytes!`-es the correct per-arch vDSO blob via `cfg(target_arch=…)` selection. `xcore::vdso` exposes `image()` returning a `&'static [u8]` and `len()` returning the blob length. No build artifact lives in the repo; the blob is produced by `xmodules/xvdso/build.rs` (or a top-level `make vdso-blob` step invoked by the root crate's build script) and dropped under `target/` for `include_bytes!` to pick up.

- G-3: On `execve` (`xcore::mm::init::load_app`), the kernel maps three contiguous regions inside the user address space:
  1. **vDSO data page** (R, 1 page) — kernel-owned `VdsoData` struct, seqlock-protected.
  2. **vDSO code page(s)** (R-X, 1–2 pages) — backed by the embedded ELF blob, mapped via `map_alloc` + `write` (same pattern as ELF segments).
  3. The `AT_SYSINFO_EHDR` auxv entry is set to the code page's base address.

- G-4: `__vdso_clock_gettime(clock_id, *timespec)` and `__vdso_gettimeofday(*timeval, *tz)` serve `CLOCK_REALTIME`, `CLOCK_MONOTONIC`, and `CLOCK_MONOTONIC_RAW` entirely from the data page using `rdtime` (RV64) / `rdtime.d` (LA64), without trapping. Unsupported clocks (`CLOCK_PROCESS_CPUTIME_ID`, `CLOCK_THREAD_CPUTIME_ID`, etc.) fall through to a syscall via `ecall` / `syscall` instructions inside the vDSO code, returning the syscall's result unchanged. `__vdso_clock_getres` returns `1 ns` for the supported clocks (matching the current syscall behavior at `xapi/src/sys/time.rs:71`), syscall fallback otherwise. `__vdso_time` returns `data.wall_sec` directly. `__vdso_getcpu` reads CPU id from a per-CPU stash in the data page (initially always returns `0`; multi-CPU correctness deferred to a follow-up but the ABI shape is established).

- G-5: `__vdso_rt_sigreturn` (and `__vdso_sigreturn` on arches where the legacy ABI applies — RISC-V uses only `rt_sigreturn`; LoongArch likewise) executes the trap instruction with `nr=rt_sigreturn`. `xsignal::arch::signal_trampoline_address()` is replaced by `xsignal::arch::rt_sigreturn_offset()` returning a `usize` offset *within* the vDSO image; the kernel resolves the per-process absolute address by adding the per-process vDSO base. `sigaction` consumers stop reading `SIGNAL_TRAMPOLINE`.

- G-6: `SIGNAL_TRAMPOLINE` (`xcore/src/config.rs:38`), the `map_trampoline` function (`xcore/src/mm/init.rs:35`), `xsignal::arch::signal_trampoline_address()`, and the assembly that backs it are all removed. No call-site remains in `xcore`/`xapi`.

- G-7: Test rootfs gains `xtest/c/vdso_clock_monotonic.c`, a first-party C test that calls `clock_gettime(CLOCK_MONOTONIC, &t)` 1,000,000 times and verifies (a) every sample is ≥ the previous sample, (b) total elapsed real time is in a sane range (>0, <60 s on the QEMU image), (c) the program exits 0. It is built by the existing xtest pipeline per `specs/features/redesign-xtest/SPEC.md` (one `.c` → one statically-linked ELF; picked up by `run-c.sh`).

- G-8: All existing LTP cases listed in `src/init.sh` that touch covered surface — `clock_gettime0[1-3]`, `gettimeofday*`, `kill*`, `sigaction*`, `rt_sigreturn*`, `signal*` — pass on both `make rv` and `make la`. `make build ARCH={riscv64,loongarch64}` succeeds; both kernels boot to userspace.

- NG-1: vDSO ASLR / per-process randomized base. v1 maps the vDSO at a fixed VA per-process (`USER_VDSO_BASE = 0x4001_0000`, reusing the slot freed by removing `SIGNAL_TRAMPOLINE`). Randomization is a follow-up.
- NG-2: `getcpu` correctness on SMP. The ABI is in place; the data page exposes a `cpu` field; v1 returns `0`. Real per-CPU mapping is a follow-up.
- NG-3: vDSO build outside the pinned toolchain. The vDSO crate uses the same toolchain as the kernel; no separate cross-compiler.
- NG-4: User-readable HW counter on platforms beyond the two QEMU configs. The plan validates `rdtime`/`rdtime.d` only on `riscv64-qemu-virt`, `loongarch64-qemu-virt`, and `riscv64-visionfive2`. Other platforms (none currently shipped) would re-validate.
- NG-5: x86_64 / aarch64 vDSO. Out of scope; the root build does not cover those arches.
- NG-6: Compatibility with statically-linked binaries that ignore auxv. Statically-linked binaries that bypass the dynamic loader and never read `AT_SYSINFO_EHDR` continue to syscall — the vDSO is purely additive.

[**Architecture**]

```
xmodules/xvdso/                    (NEW workspace member)
├── Cargo.toml                     cdylib, no_std, panic=abort
├── build.rs                       per-arch link with linker scripts
├── src/
│   ├── lib.rs                     #![no_std] entry; re-exports per-arch
│   ├── data.rs                    VdsoData layout (mirrors xcore::vdso::data)
│   ├── time.rs                    seqlock read; clock dispatch
│   ├── arch/
│   │   ├── riscv64.rs             rdtime; ecall fallback; rt_sigreturn asm
│   │   └── loongarch64.rs         rdtime.d; syscall fallback; rt_sigreturn asm
│   └── exports.rs                 #[no_mangle] __vdso_* / __kernel_*
├── linker/
│   ├── vdso-riscv64.lds
│   └── vdso-loongarch64.lds
└── README.md

xcore/src/vdso/                    (NEW module inside xcore)
├── mod.rs                         pub fn image(), pub fn install(uspace, base)
├── data.rs                        struct VdsoData (kernel mirror), seqlock writer
├── blob.rs                        include_bytes!{vdso-<arch>.so}
└── tick.rs                        fn vdso_tick() — called from timer ISR to refresh data page

xcore/src/mm/init.rs               EDIT: load_app maps vDSO + data page; auxv += AT_SYSINFO_EHDR
xcore/src/config.rs                EDIT: remove SIGNAL_TRAMPOLINE; add USER_VDSO_BASE, USER_VDSO_DATA
xmodules/xsignal/src/arch/*.rs     EDIT: signal_trampoline_address() → rt_sigreturn_offset()
xcore/src/task/proc.rs             EDIT: stop referencing SIGNAL_TRAMPOLINE; per-process vdso_base
arceos/crates/kernel_elf_parser    EDIT: AuxvEntry array length grows from 17 → 18 (or accept slice)

src/main.rs                        unchanged
xtest/c/vdso_clock_monotonic.c     NEW first-party C test
```

End-to-end flow:

```
boot:
  axhal::time init  ─────►  vdso_tick() seeds VdsoData{ wall_sec, mono_ns_offset, mult, shift, seq=2 }
  timer_irq         ─────►  vdso_tick() bumps seq, writes new monotonic offset, bumps seq

execve:
  load_app:
    map ELF segments
    map heap
    map ustack
    ── NEW ────────────
    map vDSO data page (R)        @ USER_VDSO_DATA  → backed by kernel page holding VdsoData
    map vDSO code  page(s) (R-X)  @ USER_VDSO_BASE  → backed by alloc; write blob bytes
    auxv[AT_SYSINFO_EHDR] = USER_VDSO_BASE
    auxv[AT_BASE]       unchanged
    proc.vdso_base = USER_VDSO_BASE
    proc.rt_sigreturn = USER_VDSO_BASE + xsignal::arch::rt_sigreturn_offset()

userspace:
  glibc/musl ld.so reads AT_SYSINFO_EHDR → resolves __vdso_clock_gettime
  app calls clock_gettime(CLOCK_MONOTONIC, &ts)
    └─ libc → __vdso_clock_gettime (no trap)
              └─ seqlock-read VdsoData
                  └─ now = (rdtime() * mult) >> shift + mono_ns_offset
              return 0

signal:
  kernel delivery (xsignal::handle):
    user_frame.pretcode = proc.rt_sigreturn        // was SIGNAL_TRAMPOLINE
    set PC = handler
  user handler returns to pretcode → __vdso_rt_sigreturn → ecall #rt_sigreturn → kernel restores
```

[**Data Structure**]

```rust
// xmodules/xvdso/src/data.rs  AND  xcore/src/vdso/data.rs (kernel mirror; layout MUST match)
#[repr(C)]
pub struct VdsoData {
    /// Seqlock counter. Even = stable; odd = writer in progress. Reader retries on odd or mismatch.
    pub seq: AtomicU32,
    pub _pad0: u32,

    /// Wall-clock seconds at the point captured by `mono_cycles_at_capture`.
    pub wall_sec: u64,
    /// Wall-clock nanoseconds (sub-second) at capture.
    pub wall_nsec: u32,
    pub _pad1: u32,

    /// Monotonic nanoseconds at capture. `now_ns = mono_ns + ((rdtime - cycles_at_capture) * mult) >> shift`.
    pub mono_ns: u64,
    pub mono_cycles_at_capture: u64,
    pub mult: u32,
    pub shift: u32,

    /// Per-CPU id stash (v1: always 0 on UP; G-getcpu follow-up will mark this per-CPU).
    pub cpu: u32,
    pub _pad2: u32,
}

// xcore/src/vdso/mod.rs
pub fn image() -> &'static [u8];               // include_bytes! result
pub fn install(uspace: &mut AddrSpace) -> AxResult<VirtAddr>;  // returns vdso base
pub fn rt_sigreturn_address(vdso_base: VirtAddr) -> VirtAddr;

// xcore/src/task/proc.rs additions to XProcess (or its TaskExt)
pub struct VdsoBinding {
    pub base: VirtAddr,                        // vDSO code page base
    pub rt_sigreturn: VirtAddr,                // base + rt_sigreturn_offset
}

// xmodules/xsignal/src/arch/{riscv64,loongarch64}.rs (replacing signal_trampoline_address)
pub fn rt_sigreturn_offset() -> usize;         // offset into vDSO code page
```

[**API Surface**]

```rust
// vDSO exports (in xmodules/xvdso/src/exports.rs; #[no_mangle], with linker-script aliases)
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_clock_gettime(clock_id: i32, ts: *mut Timespec) -> i32;
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_gettimeofday(tv: *mut Timeval, tz: *mut c_void) -> i32;
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_clock_getres(clock_id: i32, res: *mut Timespec) -> i32;
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_time(tloc: *mut i64) -> i64;
#[unsafe(no_mangle)] pub unsafe extern "C" fn __vdso_getcpu(cpu: *mut u32, node: *mut u32, tcache: *mut c_void) -> i32;
#[unsafe(naked)]    pub unsafe extern "C" fn __vdso_rt_sigreturn();
// Aliases (provided via linker script `PROVIDE`):
//   __kernel_clock_gettime  = __vdso_clock_gettime;
//   __kernel_gettimeofday   = __vdso_gettimeofday;
//   __kernel_rt_sigreturn   = __vdso_rt_sigreturn;
//   __kernel_clock_getres   = __vdso_clock_getres;
//   __kernel_getcpu         = __vdso_getcpu;

// xcore::vdso public API
pub fn image() -> &'static [u8];
pub fn install(uspace: &mut AddrSpace) -> AxResult<VdsoBinding>;
pub fn data_writer() -> &'static VdsoDataWriter;     // called from timer ISR

// xcore::vdso::VdsoDataWriter
impl VdsoDataWriter {
    pub fn refresh(&self, mono_cycles: u64, mono_ns: u64, wall_sec: u64, wall_nsec: u32);
}
```

[**Constraints**]

- C-1: vDSO blob size ≤ 8 KiB (2 pages) on both arches; CI fails the build if exceeded.
- C-2: `VdsoData` is exactly one page (4 KiB); kernel mirror and userspace view share an identical `#[repr(C)]` layout — enforced by a `const _: () = assert!(size_of::<VdsoData>() <= PAGE_SIZE_4K);` on both sides.
- C-3: vDSO code is `no_std`, `panic = "abort"`, no allocation, no global mutable state outside the data page. No stack frame > 256 bytes (no large local arrays).
- C-4: vDSO uses only registers and the data page; it never reads thread-local storage or anything outside `[USER_VDSO_BASE, USER_VDSO_BASE + image_size)` ∪ `[USER_VDSO_DATA, USER_VDSO_DATA + 4 KiB)`.
- C-5: Seqlock invariant — `seq` even ⇒ readers see a consistent snapshot; writer increments to odd, writes, increments to even. Reader retries on odd or on mismatch between pre-read and post-read seq. Single-writer (timer ISR on CPU 0); multi-reader.
- C-6: Auxv array growth (currently `[AuxvEntry; 17]` in `arceos/crates/kernel_elf_parser`) must accommodate `AT_SYSINFO_EHDR`. Either widen the array (preferred — minimum churn) or accept a slice; either way, no caller outside `kernel_elf_parser` constructs the array directly.
- C-7: vDSO code must be position-independent (`-fpic` equivalent — Rust default for `cdylib`) so the kernel can map it at any 4 KiB-aligned address. v1 keeps a fixed base (NG-1) but the constraint enables future ASLR.
- C-8: vDSO image must not depend on the kernel's runtime symbol table; it links statically against its own minimal `core` and is stripped of all relocations except R_*_RELATIVE.
- C-9: User-mode `rdtime` (RV64) and `rdtime.d` (LA64) must be trap-free in U-mode for our shipped platform configs. Validated as a Phase 0 spike before the data-page integration lands; if either arch traps, that arch's vDSO falls back to a syscall path for time entries (G-4's "no trap" goal weakens to "no trap on supported arch") and the alternative is recorded in a follow-up task.
- C-10: vDSO is mapped in *every* user address space, including the very first one (init) and every `execve` after. `unmap_user_areas()` must not unmap the vDSO data page (or must remap it as part of post-`execve` setup before returning to user mode).
- C-11: Removing `SIGNAL_TRAMPOLINE` is atomic with adding the vDSO `rt_sigreturn` symbol — no commit may leave `xsignal` reaching for a deleted symbol.
- C-12: vDSO build does not require Docker. `cargo build -p xvdso --target <arch>-unknown-none -Z build-std=core` is enough; this keeps `make build` working in environments without Docker (the xtest pipeline still uses Docker for rootfs baking, unchanged).

## Runtime `runtime logic`

[**Main Flow — clock_gettime fast path**]

1. App calls `libc::clock_gettime(CLOCK_MONOTONIC, &ts)`.
2. libc dispatches to `__vdso_clock_gettime` (resolved at startup via `AT_SYSINFO_EHDR`).
3. vDSO body:
   a. Load `seq` from `VdsoData` via `LDAR`/`fence`-equivalent acquire.
   b. If `seq & 1`, `pause`/`yield` and retry.
   c. Read `wall_sec`, `mono_ns`, `mono_cycles_at_capture`, `mult`, `shift`.
   d. Read `seq` again; if changed or odd, retry.
   e. `delta = rdtime() − mono_cycles_at_capture`.
   f. `delta_ns = (delta * mult) >> shift`.
   g. `now_ns = mono_ns + delta_ns` (or `wall_sec*1e9 + wall_nsec + delta_ns` for `CLOCK_REALTIME`).
   h. Write `ts->tv_sec = now_ns / 1e9; ts->tv_nsec = now_ns % 1e9`.
4. Return `0`. No trap.

[**Main Flow — rt_sigreturn**]

1. Kernel delivers signal: writes user frame, sets `pretcode = vdso_base + rt_sigreturn_offset`, jumps to handler.
2. User handler returns; PC lands on `__vdso_rt_sigreturn`.
3. vDSO body: `li a7, NR_rt_sigreturn; ecall` (RV64) / `li $a7, NR_rt_sigreturn; syscall 0` (LA64).
4. Kernel `sys_rt_sigreturn` restores ucontext.

[**Failure Flow**]

1. Unsupported clock id → vDSO falls through to syscall-emit instructions inside the vDSO code (`ecall` / `syscall 0`) and returns the kernel's result unchanged.
2. `rdtime` traps on a platform we didn't anticipate → kernel observes the trap, but vDSO has already failed its contract on that arch. C-9's spike catches this before integration; if it slips through, the failure is contained to time entries (signal path uses syscall regardless).
3. Seqlock writer pre-empted (single-writer in timer ISR — should not happen under normal scheduling) → readers spin on `seq & 1` indefinitely. Mitigation: the writer disables interrupts around the two `seq` updates; the window is ≤ 20 instructions.
4. Auxv array overflow on widen → caught at compile time by `assert_eq!(auxv.len(), N)` in `app_stack_region`.

[**State Transitions**]

- Boot → `xcore::vdso::init()` registers the data page, runs the C-9 spike, seeds initial `VdsoData` from `axhal::time`, installs the timer-tick refresher.
- `execve` → `xcore::vdso::install(uspace)` maps both pages into the address space, returns `VdsoBinding`, stored in `XProcess`.
- Timer ISR → `data_writer().refresh(...)` updates `VdsoData` under seqlock.
- Signal delivery → reads `XProcess.vdso.rt_sigreturn` for `pretcode`.
- Process exit → no special unmap; `AddrSpace::drop` releases the alloc-backed pages normally.

## Implementation `split task into phases`

[**Phase 0 — Spike: validate user-mode counter access (C-9)**]

- Boot a trivial kernel test (`make rv` and `make la`) that drops to U-mode and executes `rdtime` / `rdtime.d`, then a second `ecall`/`syscall` to report the result.
- If both succeed without trap, lock the design as written.
- If either traps, downgrade that arch to "vDSO time entries call syscalls" (still sets `AT_SYSINFO_EHDR`, still owns `rt_sigreturn`); record in `## Log` of next iteration.

[**Phase 1 — vDSO crate skeleton + blob pipeline (G-1, G-2, C-1, C-3, C-7, C-8, C-12)**]

- Create `xmodules/xvdso/` workspace member.
- Add per-arch `linker/vdso-{rv,la}.lds` modeled on Linux's `arch/{riscv,loongarch}/kernel/vdso.lds.S`.
- `Cargo.toml`: `[lib] crate-type = ["cdylib"]`, `[profile.release] panic = "abort"`, `lto = true`, `opt-level = 3`, `strip = true`.
- `build.rs`: pass `-C link-arg=-Tlinker/vdso-<arch>.lds`, `-C relocation-model=pic`, `-C link-arg=-soname=linux-vdso.so.1`, `-C link-arg=--build-id=none`, `-C link-arg=--no-eh-frame-hdr`-but-yes-`.eh_frame_hdr` (to enable userspace unwind).
- Stub all entries to syscall fallback first (proves the build pipeline before any seqlock work).
- Add a workspace-level `xmodules/xvdso/blobs/` *not* checked in; root crate's `build.rs` invokes `cargo build -p xvdso --target <arch>-unknown-none --release` for the active `target_arch`, then `include_bytes!` the resulting `.so`.
- Add a CI-style size assert: `assert!(image().len() <= 2 * PAGE_SIZE_4K)`.

[**Phase 2 — kernel mapping + auxv (G-3, C-2, C-6, C-10)**]

- Add `xcore::vdso::install(uspace)` that maps the two pages (using the existing `map_alloc` + `write` pattern from `xcore/src/mm/init.rs:55`).
- Wire it into `load_app` after the heap mapping; store the resulting `VdsoBinding` in `XProcess`.
- Widen `kernel_elf_parser::AuxvEntry` array from 17 to 18 entries; `app_stack_region` accepts the new length transparently. Update `map_elf` return type accordingly.
- Append `AT_SYSINFO_EHDR = vdso_base.as_usize()` to the auxv array.
- Verify with a one-shot user program (existing `LD_SHOW_AUXV=1 /bin/true` style) that the entry appears.

[**Phase 3 — time data page + fast path (G-4, C-2, C-5)**]

- Mirror `VdsoData` in `xcore::vdso::data` (kernel side) and `xvdso::data` (user side); compile-time layout assert on both.
- Implement `VdsoDataWriter::refresh` with `Acquire`/`Release` ordering on `seq`.
- Hook `axhal` timer ISR (already present) to call `vdso_tick()` once per tick.
- Replace stub `__vdso_clock_gettime` / `__vdso_gettimeofday` / `__vdso_clock_getres` / `__vdso_time` with seqlock-read fast paths.
- Add `__vdso_getcpu` returning `data.cpu` (always 0 in v1).
- Run `clock_gettime0[1-3]` LTP cases.

[**Phase 4 — rt_sigreturn migration (G-5, G-6, C-11)**]

- Add `__vdso_rt_sigreturn` (naked asm: `li a7, 139 ; ecall` on RV64; `li $a7, 139 ; syscall 0` on LA64 — the syscall numbers come from `linux_raw_sys`/`syscalls`).
- Add `xsignal::arch::rt_sigreturn_offset()` (returns the offset of the symbol from the vDSO base; computed at link time and exposed via a small generated table).
- Update `xsignal` signal-frame writers to read `XProcess.vdso.rt_sigreturn` instead of `xcore::config::SIGNAL_TRAMPOLINE`.
- Delete `xcore::config::SIGNAL_TRAMPOLINE`, `xcore::mm::init::map_trampoline`, and `xsignal::arch::signal_trampoline_address()` and its assembly.
- Confirm `kill*`, `sigaction*`, `rt_sigreturn*` LTP cases pass on both arches.

[**Phase 5 — xtest integration + docs (G-7, G-8)**]

- Add `xtest/c/vdso_clock_monotonic.c`; verify it lands at `tests-rootfs-$ARCH.img:/root/tests/c/vdso_clock_monotonic` per the xtest spec.
- Run `make tests ARCH=riscv64` and `make tests ARCH=loongarch64`; run `make run-tests ARCH=…` for both; confirm `[PASS] vdso_clock_monotonic` appears.
- Update `AGENTS.md` "Testing" section to mention the vDSO surface and where its tests live.
- Update `docs/StarryX/mm.md` (and possibly add `docs/StarryX/vdso.md`) to document the layout, the data page, and the `AT_SYSINFO_EHDR` contract.

## Trade-offs `ask reviewer for advice`

- **T-1: Where to publish the rt_sigreturn offset.**
  - (a) Generate a `vdso_offsets.rs` from a build-time `nm` pass, included by both `xsignal` and `xcore`. *Adv.:* exact, cheap. *Disadv.:* extra build-time tooling (still toolchain-only — `llvm-nm` ships with rustup).
  - (b) Define the symbol at a fixed offset in the linker script (`rt_sigreturn = 0x100;`). *Adv.:* zero tooling. *Disadv.:* fragile — any inline asm size change risks overrunning.
  - **Lean: (a)**. The build script already exists; one extra `nm` invocation is cheap and self-validating.

- **T-2: Auxv array widening vs. moving to `Vec<AuxvEntry>`.**
  - (a) Widen the fixed array from 17 to 18 entries. *Adv.:* one-line change in `kernel_elf_parser`; no allocation in the boot path. *Disadv.:* still fragile to future entries.
  - (b) Switch to `&[AuxvEntry]` everywhere. *Adv.:* future-proof. *Disadv.:* touches more API surface, including the vendored crate's README/examples.
  - **Lean: (a) for v1**, with a comment explaining (b) is the eventual direction.

- **T-3: Seqlock writer placement.**
  - (a) Inside the timer ISR. *Adv.:* highest update frequency, simplest. *Disadv.:* IRQ context constraints (no allocation, no blocking — fine for our struct).
  - (b) From a kernel thread. *Adv.:* relaxed context. *Disadv.:* extra latency; still requires synchronization with the timer source.
  - **Lean: (a)**. The struct is fixed-size, no allocation; ISR is the canonical Linux placement.

- **T-4: vDSO build invocation.**
  - (a) `xmodules/xvdso/build.rs` self-builds. *Adv.:* single-step `make build`. *Disadv.:* recursive cargo invocation — needs `CARGO_TARGET_DIR` discipline.
  - (b) Top-level `make vdso-blob` step the root crate's `build.rs` calls. *Adv.:* clean separation. *Disadv.:* one more Make target.
  - **Lean: (b)**. Avoids recursive cargo and keeps the kernel crate's build script tiny.

## Validation `test design`

[**Unit Tests**]

- V-UT-1: `xcore::vdso::data` — round-trip a `VdsoData` through the seqlock writer; assert reader sees consistent snapshots across 1,000,000 iterations under a stress thread (`#[cfg(test)]` in xcore tests).
- V-UT-2: `xvdso::time::compute_monotonic` — pure function, given mocked counter + mult/shift, asserts the output matches the reference `(delta * mult) >> shift + base`. Runs on host (the function is `no_std` but `cfg(test)` enables host build).
- V-UT-3: `xcore::vdso::image()` — asserts `image().len() <= 2 * PAGE_SIZE_4K` and the first 4 bytes are `\x7fELF`.
- V-UT-4: Layout assert — `assert_eq!(size_of::<xcore::vdso::data::VdsoData>(), size_of::<xvdso::data::VdsoData>())` and field offsets match.

[**Integration Tests**]

- V-IT-1: Run `clock_gettime01`, `clock_gettime02`, `clock_gettime03` from the LTP suite under `make run-tests ARCH=riscv64` and `make run-tests ARCH=loongarch64`. All PASS.
- V-IT-2: Run `gettimeofday01`, `gettimeofday02` (LTP). All PASS.
- V-IT-3: Run `kill01..kill12`, `sigaction01..sigaction03`, `rt_sigreturn01..rt_sigreturn02`, `signal01..signal06` (LTP). All PASS — exercises the new vDSO `rt_sigreturn` path on every signal delivery.
- V-IT-4: Run the new `xtest/c/vdso_clock_monotonic` first-party test. Output: `[PASS] vdso_clock_monotonic`.
- V-IT-5: Boot smoke — `make rv` and `make la` reach the userspace shell (ensures no regression in init for the non-test kernel).
- V-IT-6: `LD_SHOW_AUXV=1 /bin/true` (or equivalent) prints `AT_SYSINFO_EHDR` with a non-zero address that lies within `[USER_VDSO_BASE, USER_VDSO_BASE + image_size)`.

[**Failure / Robustness Validation**]

- V-F-1: vDSO called with an unsupported clock id (`CLOCK_PROCESS_CPUTIME_ID`) returns the same value as the syscall path (verified by a second xtest C program comparing two consecutive readings — vDSO should fall through to syscall and produce a sane number).
- V-F-2: Seqlock under contention — a kernel test pins the timer ISR refreshing at high rate and runs N reader threads in userspace via the new C test in a tight loop; no reader observes a torn read (every consecutive sample monotonic).
- V-F-3: vDSO missing — temporarily disable the vDSO mapping (debug-only feature) and re-run the time LTP cases; they MUST still pass via syscall fallback (proves the auxv-driven path is genuinely additive).

[**Edge Case Validation**]

- V-E-1: `clock_gettime(CLOCK_MONOTONIC, NULL)` — vDSO must return `EFAULT` (or call syscall which returns `EFAULT`). Asserted by an existing LTP case.
- V-E-2: `__vdso_rt_sigreturn` invoked outside a signal context — kernel must detect via the syscall path's existing checks; vDSO does not introduce new behavior here. (The existing `sys_rt_sigreturn` validates `current` ucontext.)
- V-E-3: First execve on init — vDSO mapping must succeed on a fresh address space with no prior mappings; covered by V-IT-5 (init shell start).
- V-E-4: Two processes simultaneously read vDSO — confirmed by running V-IT-4 in parallel (`run-c.sh` runs sequentially; an additional fork-based variant runs N copies and asserts no crash).

[**Acceptance Mapping**]

| Goal / Constraint | Validation                                                  |
| ----------------- | ----------------------------------------------------------- |
| G-1 (vdso crate)  | V-UT-3, V-IT-5                                              |
| G-2 (embed blob)  | V-UT-3                                                      |
| G-3 (mapping)     | V-IT-5, V-IT-6                                              |
| G-4 (time fns)    | V-IT-1, V-IT-2, V-IT-4, V-F-1                               |
| G-5 (rt_sigreturn) | V-IT-3                                                     |
| G-6 (cleanup)     | V-IT-3, V-IT-5 (no `SIGNAL_TRAMPOLINE` reference remains)   |
| G-7 (xtest)       | V-IT-4                                                      |
| G-8 (LTP green)   | V-IT-1, V-IT-2, V-IT-3, V-IT-5                              |
| C-1 (size)        | V-UT-3                                                      |
| C-2 (layout)      | V-UT-4                                                      |
| C-5 (seqlock)     | V-UT-1, V-F-2                                               |
| C-6 (auxv grow)   | V-IT-6                                                      |
| C-9 (rdtime)      | Phase 0 spike (gating)                                      |
| C-10 (every map)  | V-IT-5, V-E-3                                               |
| C-11 (atomic)     | V-IT-3, V-IT-5                                              |
