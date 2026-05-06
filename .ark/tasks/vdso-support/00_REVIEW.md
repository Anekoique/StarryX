# `vdso-support` REVIEW `00`

> Status: Open
> Feature: `vdso-support`
> Iteration: `00`
> Owner: Reviewer
> Target Plan: `00_PLAN.md`
> Review Scope:
>
> - Plan Correctness
> - Spec Alignment
> - Design Soundness
> - Validation Adequacy
> - Trade-off Advice

---

## Verdict

- Decision: Approved with Revisions
- Blocking Issues: 6
- Non-Blocking Issues: 6



## Summary

The plan establishes a coherent end-to-end vDSO design (per-arch `cdylib`, kernel-side mapping, seqlock data page, auxv `AT_SYSINFO_EHDR`, `rt_sigreturn` migration) that aligns with PRD outcomes and respects the `xmodules` reuse contract for the user-side blob. Goals map cleanly to validations and the four trade-offs are real, with sensible leans. However, several design seams under-specify how the existing kernel state actually flips: `ProcessSignalManager` already takes the restorer address by value at construction, so a "per-process vDSO base" path needs explicit threading through `XProcess::new` / `execve`; the current `ProcessSignalManager::new` signature and the `unmap_user_areas() → map_trampoline()` sequence in `xcore/src/mm/init.rs:154-157` are not addressed by the plan as written. The plan also takes a workspace-membership shortcut (`xmodules/xvdso`) that will fight with the root crate's host-target build, mis-cites several LTP test names that are not actually in `src/init.sh`, and leaves the C-9 user-mode `rdtime`/`rdtime.d` spike as a phase-0 "if it traps, downgrade" without specifying what the artifact looks like in either branch. None of the issues are fatal to the design, but six (R-001 .. R-006) need concrete answers in `01_PLAN.md` before EXECUTE.



## Findings

### R-001 `ProcessSignalManager.default_restorer is constructed once per process, not per execve`

- Severity: CRITICAL
- Section: `## Spec` G-5 / G-6, Architecture "execve" block, Implementation Phase 4
- Problem:
  `xmodules/xsignal/src/api/process.rs:33-58` defines `ProcessSignalManager { default_restorer: usize, ... }` with `default_restorer` set at `ProcessSignalManager::new(actions, default_restorer)`. The kernel's only construction site is `xcore/src/task/proc.rs:215-218`, which passes `crate::config::SIGNAL_TRAMPOLINE` (the fixed const). The plan says "`xsignal` reads the trampoline address from a per-process record set at `execve` time" and "sigaction writes the vDSO `rt_sigreturn` symbol address into the user signal frame's `pretcode`". Neither claim is supported by the current type: `default_restorer` is captured by value at process creation (well before `load_app` returns the vDSO base), and `XProcess::signal` is `Arc<ProcessSignalManager>` — interior mutability is not provided for `default_restorer`. The plan does not say whether to (a) make `default_restorer` an `AtomicUsize` / `RwLock<usize>`, (b) add a setter on `ProcessSignalManager`, (c) defer construction of the manager until after vDSO mapping, or (d) keep a separate `XProcess.vdso.rt_sigreturn` and stop using `ProcessSignalManager.default_restorer` entirely.
- Why it matters:
  Without picking one of these, Phase 4 cannot land. Each option has different blast radius — (a)/(b) touches the `xmodules/xsignal` API (used by the trait-decoupled signal core), (c) reorders `XProcess::new` vs `load_app`, (d) leaves dead state in the manager. The PRD outcome "`sigaction` writes the vDSO `rt_sigreturn` symbol address into the user signal frame's `pretcode`" implies the address must be live-readable on every signal delivery, including `execve` after fork — which requires either per-process re-creation or a writable field.
- Recommendation:
  Pick exactly one option in `01_PLAN.md` and spell it out. Preferred: extend `xsignal::api::ProcessSignalManager` with `set_default_restorer(&self, addr: usize)` backed by `AtomicUsize` (immutable in spirit; only the load happens on the hot path), and have `xcore::vdso::install` call it after the vDSO is mapped. Document that the default value pre-`execve` is `0` / unused (process is not yet runnable in user mode). Update G-5 prose accordingly.



### R-002 `xmodules/xvdso as a workspace member will fight the kernel host/cross build`

- Severity: HIGH
- Section: `## Spec` Architecture (file tree), Implementation Phase 1, Trade-off T-4
- Problem:
  The root `Cargo.toml:3-9` declares `members = ["xmodules/*", ...]`. Adding `xmodules/xvdso/` as a member makes `cargo build --workspace` (and any tool that walks the workspace, e.g. clippy, rust-analyzer) attempt to compile the vDSO crate against whatever target/profile the workspace command is invoked with — including the host triple for unit tests, `make clippy`, and the `kernel_elf_parser` host tests already present at `arceos/crates/kernel_elf_parser/tests/`. The plan asserts the vDSO is `no_std`, `panic = "abort"`, `cdylib`, built with `--target <arch>-unknown-none -Z build-std=core`. A `cdylib` for `*-unknown-none` cannot link into the host workspace for tests; conversely, `cargo build` from the kernel side will not pick the right `--target` automatically. T-4 acknowledges "recursive cargo invocation — needs CARGO_TARGET_DIR discipline" but does not address workspace membership.
- Why it matters:
  AGENTS.md "Common Pitfalls" calls out exactly this class of foot-gun ("Assuming x86_64/aarch64 still build from the root — they don't"). If `xmodules/xvdso` is a normal member, `make clippy` will break for any contributor, and `cargo test -p kernel_elf_parser` may fail because cargo refuses to resolve the workspace. The current `xmodules/*` glob captures the new crate automatically — there is no opt-out without an explicit `exclude`.
- Recommendation:
  Either (a) place the vDSO crate **outside** the workspace and add it to `exclude = [...]` in `Cargo.toml` (precedent: `apps`, the page-table subtrees), with a top-level `make vdso-blob` driving its build via an explicit `--manifest-path`; or (b) keep it under `xmodules/` but list it in `exclude`. Document the choice and update T-4's option (b) to cite the workspace exclusion concretely. Lean: (a) — keeps the `xmodules/*` glob's "reusable kernel components" semantics intact.



### R-003 `Auxv array literal width is hard-coded in two return types — change set is more than "one line"`

- Severity: HIGH
- Section: `## Spec` G-3, Constraint C-6, Implementation Phase 2
- Problem:
  `arceos/crates/kernel_elf_parser/src/info.rs:145` declares `pub fn auxv_vector(&self, pagesz: usize) -> [AuxvEntry; 17]` and the literal array on line 146-164 has exactly 17 entries (PLAN's claim "currently `[AuxvEntry; 17]`" is correct). But `xcore/src/mm/init.rs:44` *also* hard-codes the literal: `pub fn map_elf(...) -> AxResult<(VirtAddr, [AuxvEntry; 17])>`. Plan Phase 2 says "Widen ... AuxvEntry array from 17 to 18 entries; `app_stack_region` accepts the new length transparently. Update `map_elf` return type accordingly." That part is correct, but Phase 2 also says "no caller outside `kernel_elf_parser` constructs the array directly" (C-6) — `map_elf` does not *construct* the array, but it does name its size in the return type, so any caller of `map_elf` has the literal `17` baked into its signature too. The plan should explicitly enumerate the touched signatures or move to a `pub const AUXV_LEN: usize = 18;` exported by `kernel_elf_parser` so callers can write `[AuxvEntry; AUXV_LEN]` and never see the literal.
- Why it matters:
  Without an exported constant, every future auxv addition (e.g. `AT_RANDOM` already exists, future `AT_HWCAP2`, `AT_MINSIGSTKSZ`) churns three call sites. Trade-off T-2 leans (a) "for v1, with a comment explaining (b) is the eventual direction" — that comment should ride with an exported constant so the next bump is one line.
- Recommendation:
  Either: export `pub const AUXV_LEN: usize` from `kernel_elf_parser` and use it in both `auxv_vector`'s return type and `xcore::mm::init::map_elf`'s return type; or commit to T-2 option (b) now and accept a slice everywhere. Either way, restate C-6 to enumerate every signature widened (file:line list) so the executor can grep for the change set.



### R-004 `Cited LTP test names do not all exist in src/init.sh`

- Severity: HIGH
- Section: `## Spec` G-8, Validation V-IT-1 .. V-IT-3
- Problem:
  `grep -nE 'clock_gettime|gettimeofday|rt_sigreturn|sigaction[0-9]|kill[0-9]|signal[0-9]' src/init.sh` shows only `clock_gettime02`, `kill06`, `kill11`, `signal02..signal05`, and `tkill01`. The PLAN cites `clock_gettime0[1-3]`, `gettimeofday01/02`, `kill01..kill12`, `sigaction01..sigaction03`, `rt_sigreturn01..rt_sigreturn02`, and `signal01..signal06`. Most of these are not actually invoked by the upstream Alpine LTP run that `src/init.sh` drives — they would have to be added (or the validation re-pointed at the cases that are actually present, plus the new first-party C tests).
- Why it matters:
  G-8 reads "all existing LTP cases listed in `src/init.sh` ... pass on both `make rv` and `make la`" — if the cases listed in V-IT-1..V-IT-3 are not in `init.sh`, the goal text claims more coverage than the validation can deliver. This will leave Phase-5 "PASS green" undefined for the missing cases and may hide regressions in `gettimeofday`, `sigaction`, and `rt_sigreturn`.
- Recommendation:
  In `01_PLAN.md`, replace the V-IT-1..V-IT-3 case lists with the actual `src/init.sh` set (`clock_gettime02`, `kill06`, `kill11`, `signal02..signal05`, `tkill01`), and add to V-UT/V-IT a first-party C test per missing surface (`xtest/c/vdso_gettimeofday.c`, `xtest/c/vdso_clock_getres.c`, `xtest/c/vdso_rt_sigreturn.c`) following the same staging contract as `vdso_clock_monotonic.c`. Alternatively, propose adding the missing LTP cases to `src/init.sh` as part of this task's scope and call out that this changes the LTP runtime budget on `make rv`/`make la`.



### R-005 `C-10 (vDSO mapped on every execve) does not address unmap_user_areas + map_trampoline replacement order`

- Severity: HIGH
- Section: `## Spec` Architecture "execve" block, C-10, C-11, Implementation Phase 2 / Phase 4
- Problem:
  `xcore/src/mm/init.rs:153-157`:
  ```
  if !init {
      uspace.unmap_user_areas()?;
      map_trampoline(uspace)?;
      axhal::arch::flush_tlb(None);
  }
  ```
  The plan says vDSO install runs "after the heap mapping" inside `load_app` (Phase 2), and Phase 4 deletes `map_trampoline`. But the existing code uses `map_trampoline` *before* `map_elf` runs, immediately after `unmap_user_areas`. The plan's "execve" pseudocode shows `map ELF segments → map heap → map ustack → map vDSO data + code` in sequence, omitting the `unmap_user_areas → map_trampoline` step entirely. Two questions go unanswered:
  1. Should `xcore::vdso::install` move to the spot currently occupied by `map_trampoline` (between `unmap_user_areas` and `map_elf`), or after the heap?
  2. The plan deletes `map_trampoline` (G-6) but Phase 2 lands the vDSO mapping *before* Phase 4 deletes `map_trampoline`. Does Phase 2 leave both code paths mapped (vDSO + dead trampoline) until Phase 4? If so, are `USER_VDSO_BASE = 0x4001_0000` and `SIGNAL_TRAMPOLINE = 0x4001_0000` (same address per `xcore/src/config.rs:38`) supposed to alias? They cannot both be mapped simultaneously to different physical pages — `map_alloc(... 0x4001_0000)` will conflict with `map_linear(... 0x4001_0000)`.
- Why it matters:
  C-11 says "Removing `SIGNAL_TRAMPOLINE` is atomic with adding the vDSO `rt_sigreturn` symbol — no commit may leave `xsignal` reaching for a deleted symbol." The same atomicity applies to the *address*: Phase 2 + Phase 4 cannot be separate commits if they both want `0x4001_0000`. The plan needs to either (a) collapse Phase 2's vDSO base addition with Phase 4's trampoline removal into one phase, or (b) pick a different `USER_VDSO_BASE` for the transitional commits.
- Recommendation:
  In `01_PLAN.md`: (1) Re-write the Architecture pseudocode to show the explicit `unmap_user_areas → install_vdso → map_elf → ...` order. (2) Either merge Phase 2+4 into a single phase commit, or use a transitional `USER_VDSO_BASE` (e.g. `0x4002_0000`) for Phase 2 and shift it to `0x4001_0000` in Phase 4 alongside `SIGNAL_TRAMPOLINE` removal. (3) State explicitly which mapping function (`map_alloc` + `write` vs a new `map_alloc_with_data`) is used; the existing `map_alloc` + `uspace.write` pattern requires the destination to be writable transiently — flag whether that contradicts the R-X protection bits.



### R-006 `Phase 0 spike's "downgrade" branch is under-specified`

- Severity: HIGH
- Section: `## Spec` C-9, Implementation Phase 0, Failure Flow
- Problem:
  C-9 says "User-mode `rdtime` (RV64) and `rdtime.d` (LA64) must be trap-free in U-mode for our shipped platform configs. Validated as a Phase 0 spike before the data-page integration lands; if either arch traps, that arch's vDSO falls back to a syscall path for time entries". Phase 0's "if either traps, downgrade that arch to ‘vDSO time entries call syscalls'" is hand-wavy: the data structure section only describes a counter-based seqlock (`mono_cycles_at_capture`, `mult`, `shift`) — it does not describe what `__vdso_clock_gettime` does in the syscall-fallback branch. Does it call the same syscall the kernel libc would? Then what is the win? More importantly, on RV64, `rdtime` reads `time` CSR which is delegated to S-mode by the SBI; whether U-mode access traps depends on `STIMECMP` / `Sstc` extension and the `mcounteren` setting, which is a per-platform property (`riscv64-qemu-virt` allows it in default OpenSBI; `riscv64-visionfive2` may not). LoongArch `rdtime.d` reads `stable counter` and behaves similarly. The "spike" needs to enumerate which CSRs/extensions are required and list the QEMU command-line/firmware bit that enables them.
- Why it matters:
  If C-9 fails for `riscv64-visionfive2`, the entire performance argument collapses for that platform but G-3..G-8 still need to hold. NG-4 says "Other platforms (none currently shipped) would re-validate" — but vf2 IS currently shipped (`make vf2` exists, AGENTS.md §"Build & Run"). The plan does not state whether vf2 is in the v1 scope or not.
- Recommendation:
  In `01_PLAN.md`: (1) Enumerate the required hw bits per arch (`mcounteren.tm` for RV, `CSR.MISC.RPCNTL` / `cpucfg` for LA), with the OpenSBI / firmware setting that makes them readable. (2) Specify the spike artifact: a `xtest/c/vdso_rdtime_smoke.c` (one-liner that prints `rdtime`) is sufficient and lives in the repo, not just in the plan. (3) Decide vf2 scope explicitly: either include vf2 in the spike or move it to NG. (4) Specify the syscall-fallback branch's code path: is it `ecall #clock_gettime` directly inside the vDSO (no perf win, only ABI parity) or does it longjmp out to libc? Document and cite which musl/glibc paths actually use the latter.



### R-007 `mult/shift derivation is unspecified for the time fast path`

- Severity: MEDIUM
- Section: `## Spec` G-4, Data Structure (`VdsoData.mult`, `VdsoData.shift`), Runtime "Main Flow — clock_gettime fast path"
- Problem:
  The data layout exposes `mult: u32, shift: u32` and the fast path computes `delta_ns = (delta * mult) >> shift`. The plan does not say where `mult`/`shift` come from. The kernel's existing `axhal::time::ticks_to_nanos` (vendored at `arceos/modules/axhal/src/time.rs`) and per-arch `current_ticks` use `NANOS_PER_TICK` (a pre-computed nanosecond-per-tick constant). The vDSO needs the *inverse* multiplier (cycles → ns) computed once at boot and frozen. On RV64, the timebase frequency is reported by the dtb (`/cpus.timebase-frequency`); on LoongArch, by `cpucfg`. If the freq is e.g. 10 MHz, `mult = ((10**9 << shift) / 10**6)` for some `shift` chosen so `mult` fits in u32 with at least ~32 bits of precision. The plan does not describe the seeding step, the precision/rollover bound, or what happens for a `delta` large enough to overflow `delta * mult`.
- Why it matters:
  Without an explicit derivation, the executor will guess. If `mult * delta` overflows u64 for, say, a 1-second `delta`, monotonicity goes negative — exactly what V-IT-4 is supposed to catch but only after the bug is shipped. Linux's vDSO uses a per-clock `clock_mode` enum and a `MULT_SHIFT` recomputation tied to `clocksource_register_khz`; the plan should at least sketch the equivalent.
- Recommendation:
  In `01_PLAN.md` add a "mult/shift derivation" subsection: state the source of the timebase frequency on each arch (dtb on RV, `cpucfg` on LA), the chosen `shift` (Linux uses 24 by convention for ~16 MHz–4 GHz), the maximum representable delta before overflow, and the period at which `vdso_tick()` re-captures `mono_cycles_at_capture` to keep `delta` bounded. Add a unit test for the derivation in V-UT.



### R-008 `Linker-script soundness for a Linux-shaped vDSO is asserted but not specified`

- Severity: MEDIUM
- Section: `## Spec` G-1, Implementation Phase 1
- Problem:
  G-1 promises the produced ELF has "a versioned `LINUX_2.6` `DT_VERDEF`" and the tree shows `linker/vdso-{rv,la}.lds` "modeled on Linux's `arch/{riscv,loongarch}/kernel/vdso.lds.S`". That model in Linux is non-trivial: it relies on a `version-script` (`vdso.lds.S` runs `cpp` first to expand `VERSIONS { LINUX_2.6 { ... } }`), the build emits a separate `vdso.so.dbg`, then `objcopy --strip-debug` and `--rename-section`. Rust `cdylib` on its own does not produce a `DT_VERDEF` — `rustc` will not run cpp on the linker script and will not synthesize version definitions. The plan's `build.rs` flag list (`-C link-arg=-Tlinker/vdso-<arch>.lds`, `-C link-arg=-soname=linux-vdso.so.1`, `-C link-arg=--build-id=none`) does not include `--version-script=...` or any equivalent. Without `LINUX_2.6` versioning, musl's `vdso.c` (`_dl_vdso_lookup`) will not resolve symbols at all on RV/LA, because both libcs probe versioned aliases.
- Why it matters:
  If the version definition is missing, `__vdso_clock_gettime` is invisible to the dynamic loader and the entire fast path is dead code. V-IT-6 (auxv visible) passes but V-IT-1 (libc actually uses it) silently regresses to the syscall path — the test passes but doesn't measure what it claims.
- Recommendation:
  In `01_PLAN.md`'s Phase 1, add an explicit `--version-script=linker/vdso-version.lds` build-rs flag and include the version-script contents inline (or as a separate `linker/vdso-version.lds` file). Add a V-UT case that runs `llvm-readelf -V` on the produced blob and asserts a `LINUX_2.6` `Verdef` entry exists and binds `__vdso_*` symbols.



### R-009 `rt_sigreturn_offset() is not a const-evaluable function in normal Rust`

- Severity: MEDIUM
- Section: `## Spec` G-5 / Data Structure, Trade-off T-1
- Problem:
  Data Structure: "`pub fn rt_sigreturn_offset() -> usize`". G-5: "returning a `usize` offset *within* the vDSO image". T-1 leans (a): "Generate a `vdso_offsets.rs` from a build-time `nm` pass, included by both `xsignal` and `xcore`." Two consistency gaps:
  1. The function lives in `xmodules/xsignal/src/arch/{riscv64,loongarch64}.rs`, but xsignal does not own the vDSO image — it cannot run `nm` on a blob it doesn't see. AGENTS.md forbids `xmodules/*` from depending on `xcore`/`xapi`; if `xsignal` `include!`s an offsets file generated by the *root* `build.rs`, it now depends on the root crate's build artifact directory (an env var like `OUT_DIR` from a different crate is not addressable).
  2. The natural owner of the offset is `xcore::vdso` (which already owns the blob via `include_bytes!`). Then `xsignal` should *receive* the offset, not produce it.
- Why it matters:
  If `xsignal::arch::rt_sigreturn_offset()` is the API, it must either (a) parse the embedded ELF at runtime to find the symbol (slow but isolated), (b) be const-included from a path that's part of the xsignal crate (which forces the offset table to be staged into `xmodules/xsignal/src/arch/generated_offsets.rs` by the root or vdso build script), or (c) be removed in favor of a kernel-side resolution (`xcore::vdso::rt_sigreturn_address(base)`).
- Recommendation:
  Move the API to `xcore::vdso::rt_sigreturn_address(vdso_base) -> VirtAddr`, computed at vDSO install time by parsing the embedded ELF's symbol table once per boot (cached). Drop `xsignal::arch::rt_sigreturn_offset()` from the plan; have `xsignal`'s signal-frame writer take the per-process `default_restorer` (R-001's `set_default_restorer` API) — `xsignal` itself stays vDSO-agnostic, preserving the AGENTS.md decoupling.



### R-010 `Seqlock writer's "disable interrupts around the two seq updates" assumes single-CPU`

- Severity: MEDIUM
- Section: `## Spec` C-5, Failure Flow item 3, T-3
- Problem:
  C-5 promises "Single-writer (timer ISR on CPU 0); multi-reader" — but `make rv` runs `SMP=1` by default and `make vf2` runs `SMP=2` (per AGENTS.md). On SMP, "timer ISR on CPU 0" is not automatic: each CPU has its own timer interrupt. The plan does not say whether (a) only CPU 0's tick calls `vdso_tick`, or (b) all CPUs do but coordinate via a lock. Failure Flow item 3 says "the writer disables interrupts around the two `seq` updates" — that is necessary but not sufficient on SMP (it prevents preemption on the local CPU but not concurrent writers on another).
- Why it matters:
  V-F-2 ("Seqlock under contention") uses N reader threads but a single writer; on SMP without a chosen lead CPU, two ISRs can race and both fail to publish a consistent snapshot. This is the same bug Linux solved with `tk_core.lock` and `vdso_data->seq` increments under the timekeeper seqlock.
- Recommendation:
  In `01_PLAN.md` C-5, add a sub-clause: "On SMP, only the lead CPU's timer ISR (selected at boot, e.g. boot CPU) calls `vdso_tick()`; other CPUs skip the call." Or: "Concurrent writers serialize via a kernel `SpinNoIrq` taken before the two `seq` increments." Pick one; the latter is closer to Linux. Update T-3 to mention the SMP wrinkle.



### R-011 `VdsoData layout asserts size_of <= PAGE_SIZE_4K but seqlock requires alignment guarantees`

- Severity: LOW
- Section: `## Spec` C-2, Data Structure
- Problem:
  C-2: "`const _: () = assert!(size_of::<VdsoData>() <= PAGE_SIZE_4K);`". The struct is `#[repr(C)]` with `AtomicU32 seq` first, then mixed `u64`/`u32` fields with manual `_pad` filler. The kernel mirror and the userspace view "share an identical `#[repr(C)]` layout" but neither side is alignment-asserted, and `AtomicU32` has only 4-byte alignment by default — fine for `seq`, but the `wall_sec: u64` after a `u32` pad needs 8-byte alignment for atomic-friendly reads on RV64 (LR/SC on RV requires natural alignment; misaligned `LD` on RV64 traps unless `Zicclsm` is implemented).
- Why it matters:
  If a future field reordering breaks 8-byte alignment for `wall_sec`/`mono_ns`, RV64 will trap and the vDSO will silently fall back to the syscall path (or worse, segfault user mode).
- Recommendation:
  Add `#[repr(C, align(8))]` to `VdsoData`, plus a layout test that asserts `offset_of!(VdsoData, wall_sec) % 8 == 0` on both kernel and user sides.



### R-012 `_pad fields make the kernel/user mirror invariant fragile`

- Severity: LOW
- Section: `## Spec` Data Structure
- Problem:
  `_pad0: u32, _pad1: u32, _pad2: u32` in `VdsoData` (and the duplicated definition in `xcore::vdso::data`). Two copies of an identical layout with hand-rolled padding is an established source of drift. The plan's V-UT-4 "field offsets match" is reactive, not preventive.
- Recommendation:
  Either (a) factor `VdsoData` into a single shared no_std crate (`xmodules/xvdso-data` or similar) used by both sides, or (b) drop manual padding in favor of `#[repr(C, align(8))]` on the struct and let the compiler insert padding. (a) is preferred for self-documentation.



### R-013 `__vdso_getcpu's tcache argument is typed wrong for the Linux ABI`

- Severity: LOW
- Section: `## Spec` API Surface
- Problem:
  Plan: `pub unsafe extern "C" fn __vdso_getcpu(cpu: *mut u32, node: *mut u32, tcache: *mut c_void) -> i32;`. Linux kernel's `__vdso_getcpu` signature is `int __vdso_getcpu(unsigned *cpu, unsigned *node, struct getcpu_cache *tcache)` where `struct getcpu_cache` is documented as deprecated; passing it as `*mut c_void` works at the ABI level but loses the type contract. musl's `arch/riscv64/syscall_arch.h` (used by glibc-via-musl on Alpine) does not call `__vdso_getcpu` at all — only glibc does, via `sched_getcpu`. If the rootfs is musl-only (per redesign-xtest SPEC NG-3), `__vdso_getcpu` is dead weight.
- Recommendation:
  Either drop `__vdso_getcpu` from G-4 (musl rootfs doesn't call it) and document it as a follow-up, or keep it but cite the exact musl/glibc call site. Update NG-2 accordingly.



## Trade-off Advice

### TR-1 `Where to publish the rt_sigreturn offset`

- Related Plan Item: `T-1`
- Topic: Compatibility vs Clean Design (xmodules decoupling)
- Reviewer Position: Need More Justification — both options as written break the AGENTS.md decoupling rule (see R-009)
- Advice:
  Reject both T-1 options. Move the resolution to `xcore::vdso::rt_sigreturn_address(vdso_base)`, computed once per `execve` by parsing the embedded ELF's `.dynsym` (xmas_elf already in the workspace at `xcore/src/mm/init.rs:18`). Pass the resolved absolute address to `ProcessSignalManager` via a new `set_default_restorer` API (R-001).
- Rationale:
  The proposed (a) generates a file that must be `include!`-d from `xmodules/xsignal`, creating a build-graph dependency from a reusable component to a kernel-specific build artifact — exactly the coupling AGENTS.md forbids ("component crates in `xmodules/*` must stay reusable — do not pull `xcore`/`xapi` into them"). (b) "fixed offset in the linker script" is fragile and equally a contract leak between vdso build and signal arch asm. The clean design parses the ELF in the kernel where the blob already lives, costs O(symbols) on each `execve` (cached if needed), and keeps `xmodules/xsignal` vDSO-agnostic.
- Required Action:
  Adopt the kernel-side resolution in `01_PLAN.md`; rewrite T-1 to compare "parse ELF at install time" vs "build-time `nm` table baked into the kernel side". Pick parse-at-install for v1.



### TR-2 `Auxv array widening vs. moving to slice`

- Related Plan Item: `T-2`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer Option A (widen) — but with the `pub const AUXV_LEN` from R-003
- Advice:
  Adopt T-2 (a) "widen 17 → 18", but introduce `pub const AUXV_LEN: usize = 18;` in `kernel_elf_parser` and rewrite both `auxv_vector` and `xcore::mm::init::map_elf` return types in terms of it. Add a comment block citing T-2's eventual move to (b).
- Rationale:
  (b) is correct long-term but touches the vendored ArceOS crate's tests (`arceos/crates/kernel_elf_parser/tests/test_*.rs`) and README/examples, expanding scope into an unrelated subtree. The constant-based (a+) gives 90% of (b)'s benefit (one-line bumps for future `AT_*` additions) at zero scope cost.
- Required Action:
  Update `01_PLAN.md` Phase 2 to call out the `AUXV_LEN` export and the two return-type updates. No further auxv work.



### TR-3 `Seqlock writer placement`

- Related Plan Item: `T-3`
- Topic: Performance vs Simplicity (with SMP correctness)
- Reviewer Position: Prefer Option A (timer ISR), but require the SMP guard from R-010
- Advice:
  Keep ISR-driven publication. Add an explicit "lead CPU only" guard so secondary CPUs' timer ticks skip `vdso_tick()`. Document it in C-5.
- Rationale:
  (a) is simpler and matches Linux's placement. (b) (kernel thread) introduces wake-up latency on a path where stale-by-one-tick reads are already acceptable for the seqlock contract. The SMP wrinkle is real but local — a single `if cpu_id() != BOOT_CPU { return; }` at the top of `vdso_tick` solves it.
- Required Action:
  Update C-5 / T-3 to include the SMP guard. Add V-F-4 "concurrent writers on SMP" guard test (kernel-only) — pin `vdso_tick` to two cores in a stress harness, assert no torn read.



### TR-4 `vDSO build invocation`

- Related Plan Item: `T-4`
- Topic: Performance vs Simplicity (build-system)
- Reviewer Position: Prefer Option B (top-level `make vdso-blob`) — and tighten with R-002 (workspace exclusion)
- Advice:
  Adopt T-4 (b). Combine with R-002's recommendation to *exclude* `xmodules/xvdso` from the root workspace. The top-level Makefile gains a `vdso-blob` target that runs `cargo build --manifest-path xmodules/xvdso/Cargo.toml --target $(ARCH)-unknown-none -Z build-std=core --release`, drops the `.so` at a known path, and the kernel's `build.rs` `include_bytes!`-es from there.
- Rationale:
  (a) (build.rs self-builds) plus a workspace-member crate would cause `make clippy` and `cargo test` from the workspace root to recurse into `xmodules/xvdso` with the host triple, which cannot succeed for a `*-unknown-none` `cdylib`. (b) plus exclusion sidesteps both problems and matches the established `apps`/`page_table_multiarch` exclusion precedent.
- Required Action:
  Update `01_PLAN.md` Phase 1 + T-4 to specify: workspace `exclude` entry, top-level `vdso-blob` target, `--manifest-path` invocation, output path, and the kernel-side `include_bytes!` literal. Add `make build` to the validation set explicitly to confirm the new target wires in.
