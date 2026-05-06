# `vdso-support` REVIEW `01`

> Status: Closed
> Feature: `vdso-support`
> Iteration: `01`
> Owner: Reviewer
> Target Plan: `01_PLAN.md`
> Review Scope:
>
> - Plan Correctness
> - Spec Alignment
> - Design Soundness
> - Validation Adequacy
> - Trade-off Advice

---

## Verdict

- Decision: Approved
- Blocking Issues: 0
- Non-Blocking Issues: 5



## Summary

Iteration 01 substantively closes every blocking finding from `00_REVIEW.md`. The Response Matrix's "Accepted" rows are backed by concrete spec changes: `ProcessSignalManager.default_restorer` becomes `AtomicUsize` with a setter (R-001 + verified against `xmodules/xsignal/src/api/process.rs:47`); `xmodules/xvdso` and the new shared `xmodules/xvdso-data` are workspace-excluded with a top-level `make vdso-blob` driver (R-002 — Cargo's `exclude` takes precedence over the `xmodules/*` glob, so this works); `pub const AUXV_LEN: usize = 18` removes the literal `17` everywhere (R-003); the LTP case list is now exactly the cases that actually appear in `src/init.sh` (R-004 — verified by grep: `clock_gettime02`, `kill06`, `kill11`, `signal02..05`, `tkill01`); the VA-collision problem is sequenced via a transitional `0x4002_0000` and a single Phase 4 commit (R-005); the Phase 0 spike enumerates per-arch hw bits (`mcounteren.tm`) and ships an artifact (`vdso_rdtime_smoke.c`) with a defined deletion point (R-006). The MEDIUMs are also addressed: mult/shift derivation has an explicit formula and an overflow argument; an explicit `--version-script` carries `LINUX_2.6` Verdef so musl's `_dl_vdso_lookup` can find symbols; `rt_sigreturn` resolution moves to `xcore::vdso::resolve` (parses embedded ELF via the already-present `xmas_elf` dep at `xcore/src/mm/init.rs:18`), keeping `xmodules/xsignal` vDSO-agnostic per the AGENTS.md decoupling rule; the SMP seqlock guard is concrete (`this_cpu_is_bsp()` at `arceos/modules/axhal/src/cpu.rs:21` is the natural fit). VdsoData is centralized in a single `no_std` crate (R-012). Five non-blocking issues remain, none structural: a small misnamed timebase API reference, an under-specified Phase 4 atomic-commit ordering between subsystems, V-UT-5's host-test plumbing within a `*-unknown-none` cdylib crate, the `protect`-after-write pattern's interaction with `unmap_user_areas` for the data page on `execve`, and a missing build-graph link from `make build` to `make vdso-blob` for contributors invoking `cargo build` directly.



## Findings

### R-101 `Timebase frequency source is misnamed — it's not at axhal::time::TIMEBASE_FREQ_HZ`

- Severity: MEDIUM
- Section: `## Spec` Mult/Shift Derivation, Implementation Phase 3
- Problem:
  Plan: "`f_hz` = `axhal::time::TIMEBASE_FREQ_HZ` (already populated by axhal from the dtb `/cpus.timebase-frequency`)". That symbol does not exist. `axhal::time` re-exports `current_ticks`, `epochoffset_nanos`, `nanos_to_ticks`, `ticks_to_nanos` (`arceos/modules/axhal/src/time.rs:15`); the actual frequency source is `axconfig::devices::TIMER_FREQUENCY` (referenced at `arceos/modules/axhal/src/platform/riscv64_qemu_virt/time.rs:3` and the vf2 mirror) and the per-platform `NANOS_PER_TICK` const derived from it. LoongArch's `loongarch64_qemu_virt/time.rs` follows the same pattern. There is no kernel-wide public `TIMEBASE_FREQ_HZ` accessor.
- Why it matters:
  The executor will look for `axhal::time::TIMEBASE_FREQ_HZ`, not find it, and either (a) invent a new public accessor in `axhal::time` (extra cross-cutting change not budgeted in any phase), (b) reach into `axconfig::devices` directly from `xcore::vdso` (couples the kernel's vdso module to `axconfig` — fine, but should be stated), or (c) reverse-derive `f_hz = NANOS_PER_SEC / NANOS_PER_TICK` (lossy when `NANOS_PER_TICK` rounds). Option (a) is the most correct but adds churn; option (b) is the lightest. The plan should pick one.
- Recommendation:
  In `## Spec` Mult/Shift Derivation, replace `axhal::time::TIMEBASE_FREQ_HZ` with the actual surface. Two viable phrasings: (i) "expose `pub const fn timer_frequency() -> u64` in `arceos/modules/axhal/src/time.rs` re-exporting `axconfig::devices::TIMER_FREQUENCY`; the new accessor is part of Phase 3's diff." or (ii) "import `axconfig::devices::TIMER_FREQUENCY` directly in `xcore::vdso::tick`; this is permissible since `xcore` already depends on `axconfig` for other constants." Pick one and update the file-list in the Architecture block accordingly so the executor doesn't have to guess.



### R-102 `Phase 4's "one atomic commit" understates the cross-crate ordering between xsignal and xcore`

- Severity: MEDIUM
- Section: Implementation Phase 4
- Problem:
  Phase 4's bullet list mixes (a) the `ProcessSignalManager.default_restorer: usize → AtomicUsize` API change in `xmodules/xsignal/src/api/process.rs`, (b) `XProcess::new` calling `ProcessSignalManager::new(actions, 0)` in `xcore/src/task/proc.rs:215-218`, (c) the `xcore::vdso::install` call to `set_default_restorer`, (d) the `signal_trampoline` asm + `signal_trampoline_address()` deletion across all four arches in `xmodules/xsignal/src/arch/`, and (e) the `SIGNAL_TRAMPOLINE`/`map_trampoline` deletion + `USER_VDSO_BASE` shift to `0x4001_0000`. That's a single commit touching `xmodules/xsignal`, `xcore`, plus possibly `xmodules/xsignal/src/api/thread.rs:106` (which currently reads `self.proc.default_restorer` as a `usize` field — this changes to a method call after R-001's `AtomicUsize` migration). The plan does not list `thread.rs:106` in Phase 4's diff, but the field-to-method rename forces an edit there.

  Additionally: the asm-side `signal_trampoline` is also defined in `xmodules/xsignal/src/arch/x86_64.rs:11` and `aarch64.rs:10`, neither of which builds in the current root tree (per AGENTS.md "Common Pitfalls": "Assuming x86_64/aarch64 still build from the root — they don't"). The plan only mentions the `riscv64`/`loongarch64` files; the x86/aarch64 files should either be deleted in the same commit (they currently build the same trampoline asm) or left in place as dead code with a comment. Pick one.
- Why it matters:
  C-11 is the strongest atomicity claim in the spec ("one commit may not leave xsignal reaching for a deleted symbol"). If Phase 4's diff list omits `thread.rs:106` or the `x86_64`/`aarch64` arch files, the commit either fails to build the workspace (member crate `xmodules/xsignal` won't compile if `default_restorer` is a method but `thread.rs` still reads it as a field) or leaves a dangling asm symbol whose absence breaks the symmetric arch tree.
- Recommendation:
  Expand Phase 4's diff bullet list to enumerate every file that consumes `default_restorer`. From the current tree: `xmodules/xsignal/src/api/process.rs:47,51,56`, `xmodules/xsignal/src/api/thread.rs:106`, `xmodules/xsignal/src/arch/{riscv,loongarch64,x86_64,aarch64}.rs`, `xcore/src/mm/init.rs:35-39` (the `map_trampoline` body that calls `xsignal::arch::signal_trampoline_address()`), and `xcore/src/config.rs:38`. State whether `arch/x86_64.rs` and `arch/aarch64.rs` lose their `signal_trampoline` asm in this commit too (recommended: yes — keeps the per-arch tree symmetric and avoids dead-code skew, even though the root build does not exercise them).



### R-103 `V-UT-5 (LINUX_2.6 Verdef host test) plumbing inside a *-unknown-none cdylib crate is not specified`

- Severity: MEDIUM
- Section: `## Validation` V-UT-5, Implementation Phase 1
- Problem:
  V-UT-5 says: "host test runs `llvm-readelf -V` on the produced blob; asserts `LINUX_2.6` Verdef block exists and binds `__vdso_clock_gettime`, `__vdso_gettimeofday`, `__vdso_rt_sigreturn`. Runs as part of `cargo test` for the `xvdso` crate via `[[test]] required-features` host-side wrapper." But `xvdso`'s only build target is `*-unknown-none` (Phase 1: `cargo build --target riscv64imac-unknown-none-elf --release`); a `cdylib` crate compiled for `*-unknown-none` does not produce a host test harness, and `[[test]] required-features` is not the right knob (it gates on Cargo features, not target triples). Adding host tests typically means a sibling crate (e.g. `xmodules/xvdso/tests/verdef.rs` integration test) that runs after a build artifact is on disk — but integration tests are also compiled for the crate's target, so they'd hit the same `*-unknown-none` problem.
- Why it matters:
  R-008 (now closed) was specifically about the Verdef being asserted but not specified; V-UT-5 is the asserter. If V-UT-5 silently doesn't run (because the host test never compiles for `*-unknown-none`), the Verdef regression sneaks back in. Phase 1 ends "V-UT-3 + V-UT-5 green here" but V-UT-5 has no clear runner.
- Recommendation:
  Either (i) move V-UT-5 out of `xvdso` and into a host-side helper script the top-level `make vdso-blob` step invokes after the blob is built (`scripts/check-vdso-verdef.sh` running `llvm-readelf -V $(BLOB) | grep -q 'LINUX_2.6'`); or (ii) put the assertion in the kernel-side `xcore::vdso::resolve` startup path — same ELF parser already runs to find `rt_sigreturn`, so checking for the `LINUX_2.6` Verdef there is one extra `verify_verdef()` call. Either is fine; (i) catches the regression at build time, (ii) at boot. Prefer (i) so a broken vDSO doesn't reach `make run-tests`.



### R-104 `Data-page mapping interaction with unmap_user_areas + write+protect ordering on R-only data page`

- Severity: MEDIUM
- Section: `## Spec` G-3, C-10, Architecture pseudocode
- Problem:
  C-10: "The mapping function uses `map_alloc(... R|U)` then `uspace.write(image_bytes)` then `protect(... R|X|U)` — same trick `map_elf` already uses for read-only `.rodata`." This is the standard pattern (`AddrSpace::protect` exists at `arceos/modules/axmm/src/aspace.rs:433` — verified). Two unspecified details:
  1. The **data page** (`USER_VDSO_DATA`) is described as "R-only to user, W-able to kernel via the kernel mirror". The plan does not say whether the user-side mapping is `map_alloc(... R|U)` (and the kernel mirror is the alloc-backing kernel virtual alias of the same physical page) or `map_linear` (the data page is a kernel-owned static allocated once and shared across processes by linear-mapping its phys addr into every uspace). Linux uses the latter — one global `vvar` page mapped into every user AS — which is also far cheaper (no per-process page allocation). The plan's `map_alloc` phrasing implies a per-process copy of `VdsoData`, which would defeat the seqlock contract: the timer ISR cannot update N copies under the same seqlock.
  2. On `execve`, `unmap_user_areas` runs first (per the rewritten pseudocode). Does it unmap the previously-installed vDSO mappings? If yes (vDSO is in user range), Phase 2 needs to re-establish them after `unmap_user_areas` on every `execve` (which the pseudocode shows). If `unmap_user_areas` skips the vDSO range (some kernels treat vDSO as a "vvar"-style permanent mapping), the plan should say so. C-10 currently asserts mapping happens "after `unmap_user_areas`" which implies the former — fine, but the data page must therefore be a *shared* phys page across all processes for the seqlock contract to hold.
- Why it matters:
  If each process gets its own copy of the data page, the boot-CPU timer ISR's single writer (C-5) can only refresh one of them; the others go stale. V-F-2 (seqlock under contention) and V-IT-2 (monotonic) would silently fail because user code reads a stale page.
- Recommendation:
  In `## Spec` G-3 / C-10: state explicitly that the data page is a single kernel-allocated physical page (a `'static` `VdsoData` in `xcore::vdso::data`), and the user-side mapping is by phys-addr (e.g. `map_linear(USER_VDSO_DATA, phys_of(&VDSO_DATA), 4096, R|U)` or whatever the project's preferred API is). Only the *code* page is alloc-backed-and-copied per process. Update the Architecture pseudocode's "map vDSO data page" line to note "shared phys page across all processes". Add a constraint asserting this: "C-14: VdsoData is a single global instance; the data-page mapping is by-phys-addr, not by-alloc."



### R-105 `make build → make vdso-blob dependency is not captured for cargo-direct contributors`

- Severity: MEDIUM
- Section: Implementation Phase 1, C-12
- Problem:
  C-12: "`make vdso-blob ARCH=riscv64` runs cargo build … Output: `target/vdso/<arch>/release/libxvdso.so`. The kernel's `build.rs` `include_bytes!`-es from there." Phase 1 adds the `vdso-blob` Make target. The plan does not state whether `make build` (the existing kernel-build entry) gains a dependency on `vdso-blob`, nor what happens when a contributor runs `cargo build --manifest-path xcore/Cargo.toml` (or `make clippy`, which doesn't currently invoke `make vdso-blob`) — the kernel's `build.rs` `include_bytes!` will fail with "file not found" if the blob hasn't been built yet. Two viable resolutions:
  - (a) The kernel's `build.rs` shells out to `cargo build --manifest-path xmodules/xvdso/Cargo.toml --target … --release` itself when the blob is missing. Removes the Make-side coupling but reintroduces recursive cargo (the very thing T-4 ruled out).
  - (b) `make build` and `make clippy` gain a `vdso-blob` prerequisite; cargo-direct invocation is documented as unsupported with a `build.rs` panic message that points to `make vdso-blob`.
- Why it matters:
  AGENTS.md "After writing code" says "make fmt and make clippy both targets you touched". `make clippy` is invoked frequently. If it doesn't have the `vdso-blob` prerequisite, every clippy invocation after a fresh `cargo clean` will fail until the user runs `make vdso-blob` manually. T-4 picked option (b) ("Top-level `make vdso-blob` + workspace exclude") to *avoid* recursive cargo, so the Makefile must carry the dependency.
- Recommendation:
  In `## Spec` Implementation Phase 1, add: "`make build`, `make clippy`, `make rv`, `make la`, `make vf2` all gain `vdso-blob` as a prerequisite (in `scripts/make/build.mk` or the top-level Makefile). The kernel's `build.rs` emits `cargo:rerun-if-changed=target/vdso/<arch>/release/libxvdso.so` so an out-of-date blob retriggers a rebuild." Document in the new `docs/StarryX/vdso.md` (Phase 5 deliverable) that direct `cargo build` from a workspace root requires `make vdso-blob` first, with a `build.rs` `panic!()` carrying that message if the blob is missing.



## Trade-off Advice

(All four trade-offs from `00_REVIEW.md` were Applied as advised; no new trade-off questions emerge in iteration 01. The plan's restated T-1..T-4 leans match TR-1..TR-4 from the prior review. No further trade-off advice needed.)
