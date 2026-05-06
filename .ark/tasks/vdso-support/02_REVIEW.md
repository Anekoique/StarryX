# `vdso-support` REVIEW `02`

> Status: Closed
> Feature: `vdso-support`
> Iteration: `02`
> Owner: Reviewer
> Target Plan: `02_PLAN.md`
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
- Non-Blocking Issues: 3



## Summary

Iteration 02 substantively closes every MEDIUM raised in `01_REVIEW.md` (R-101..R-105). Spot-checks confirm the Spec/Implementation deltas are not just Response-Matrix bookkeeping: `axconfig::devices::TIMER_FREQUENCY` is the real surface (verified at `arceos/modules/axhal/src/platform/riscv64_qemu_virt/time.rs:3`) and the new thin re-export `axhal::time::timer_frequency()` is sequenced into Phase 3's diff (R-101 resolved). Phase 4's diff list now enumerates every consumer of `default_restorer` including `xmodules/xsignal/src/api/thread.rs:104-106` (verified the line currently reads `self.proc.default_restorer` as a field — the field-to-method rename is mandatory under the `AtomicUsize` migration), all four `xmodules/xsignal/src/arch/{riscv,loongarch64,x86_64,aarch64}.rs` files (filenames verified — note the existing tree uses `riscv.rs`, not `riscv64.rs`, and the plan correctly calls this out), `xcore/src/mm/init.rs:34-42` plus the `:154` call site (function exists at the cited range; call site is actually `:155`, off by one — see R-201), and `xcore/src/task/proc.rs:215-218` (R-102 resolved). V-UT-5 moves out of the `*-unknown-none` cdylib's test harness into a host-side `scripts/check-vdso-verdef.sh` invoked at the tail of `make vdso-blob` (R-103 resolved). C-14 codifies the data page as a single kernel-resident `'static VdsoData` mapped via `AddrSpace::map_linear` (verified at `arceos/modules/axmm/src/aspace.rs:153`), shared by phys-addr across every process — without this the boot-CPU single-writer seqlock would be silently incorrect (R-104 resolved). C-15 wires `vdso-blob` as a Make prerequisite of `build`, `clippy`, `rv`, `la`, `vf2` and adds a `build.rs` panic for the cargo-direct path (R-105 resolved). The SMP guard `axhal::cpu::this_cpu_is_bsp()` is verified at `arceos/modules/axhal/src/cpu.rs:21`; `AddrSpace::protect` is verified at `aspace.rs:433`. Three new non-blocking MEDIUMs surface from the deltas, none structural: a one-line citation drift on the `map_trampoline` call site, an unstated kernel-linker-script constraint that the `'static VDSO_DATA` introduced by C-14 must land in a kernel region whose `virt_to_phys` is well-defined and whose page is alignable to 4 KiB, and an under-specified Phase 4 commit-size assertion (the enumerated diff now spans 14 file edits across two crates and the kernel linker script may also be touched). Phase 4 atomicity remains realistic — the diff is mechanical and non-overlapping with Phase 3 — but is large enough that the executor should pre-build it on a branch and verify `make rv` / `make la` both green before squashing.

R-101..R-105 are all demonstrably resolved by content changes in `## Spec` and `## Implementation`, not just by Response Matrix entries. The loop terminates here.



## Findings

### R-201 `map_trampoline call-site line number drifts by one`

- Severity: LOW
- Section: `## Log` Phase 4 enumerated diff; `## Implementation` Phase 4 table
- Problem:
  Plan cites `xcore/src/mm/init.rs:34-42 + 154` for the `map_trampoline` function and its single call site. Verified: the function body lives at lines 34-42 (correct); the call site `map_trampoline(uspace)?;` is at line 155, not 154 (line 154 is `uspace.unmap_user_areas()?;`). Off by one.
- Why it matters:
  Negligible impact — the executor will see both lines in the same `if !init { ... }` block and delete the right one. But the deep-tier review bar prefers exact citations because Phase 4 is the "one atomic commit" with the strongest atomicity claim (C-11), and `git grep` for `map_trampoline` is the cheap fallback if the line drifts.
- Recommendation:
  Either change `mm/init.rs:34-42 + 154` → `mm/init.rs:34-42 + 155`, or replace the line numbers with a `git grep -n map_trampoline` call-out. Not a re-iteration trigger.



### R-202 `Kernel linker / virt_to_phys constraint introduced by C-14 is not stated`

- Severity: MEDIUM
- Section: `## Spec` C-14, Implementation Phase 3
- Problem:
  C-14 says "`VdsoData` lives as a single `'static` instance in `xcore::vdso::data` (kernel side); all user-space mappings of `USER_VDSO_DATA` resolve to the **same physical page** via `map_linear`. The `VdsoDataWriter` writes through the kernel-virtual alias of that page". The implementation step in Phase 3 reads:

  ```
  Switch the data-page mapping in xcore::vdso::install from the Phase-2 placeholder (per-process map_alloc) to map_linear(USER_VDSO_DATA, virt_to_phys(&VDSO_DATA), PAGE_SIZE_4K, R|U).
  ```

  Two unstated preconditions:
  1. `&VDSO_DATA` must lie in a kernel virtual region for which `virt_to_phys()` is meaningful — i.e., the kernel-image-linear region (not, e.g., a per-CPU section, an early-boot identity-mapped scratch region, or a region remapped after boot). For StarryX's kernel layout this is normally true of `'static` BSS, but the plan does not state which `static` storage class / linker section the symbol must land in. If a future kernel relocation moves `VDSO_DATA` into a region where `virt_to_phys` is not defined, the mapping silently aliases the wrong page.
  2. `map_linear` requires page alignment on both the virt and phys side (verified at `arceos/modules/axmm/src/aspace.rs:161-164` — "address not aligned" `ax_err!`). `VdsoData`'s `#[repr(C, align(8))]` gives 8-byte alignment, not 4 KiB. A `static VDSO_DATA: VdsoData` will therefore generally **not** be 4 KiB aligned, and `map_linear(...)` will return `InvalidInput` at boot.
- Why it matters:
  (1) is a subtle long-term hazard; (2) is an immediate Phase 3 build/boot failure — the executor will write the obvious `static VDSO_DATA: VdsoData = ...` and discover it traps in `map_linear`'s align check on first `execve`. The fix is mechanical (`#[repr(C, align(4096))]` on a wrapper struct, or `#[link_section = ".bss.vdso_data"]` + a linker-script `. = ALIGN(4096);` directive), but it must be specified.
- Recommendation:
  Add a sub-bullet to C-14: "The `'static` holding `VdsoData` must be 4 KiB-aligned (e.g., wrap in `#[repr(C, align(4096))] struct VdsoDataPage(VdsoData);` so the page-aligned alignment requirement of `AddrSpace::map_linear` is met). The wrapper lives in `xcore::vdso::data`, not in `xmodules/xvdso-data` (the cross-crate data-layout struct stays 8-aligned to keep the user-side vDSO's load slim)." Optionally also add: "`xcore::vdso::data` may need a `#[link_section]` directive if the default kernel linker script does not place 4 KiB-aligned BSS without prompting" — but this is verifiable cheaply at Phase 3 implementation time, so leaving it as a "verify at impl" note is fine. Mention this in the Phase 3 bullet list so the executor doesn't trip on it.



### R-203 `Phase 4 "one commit" — diff size is now 14 file edits across two crates; commit-discipline note is missing`

- Severity: LOW
- Section: `## Implementation` Phase 4
- Problem:
  After R-102's expansion, Phase 4's enumerated diff is:
  - 7 files in `xmodules/xsignal` (process.rs, thread.rs, arch/{riscv,loongarch64,x86_64,aarch64}.rs, arch/mod.rs)
  - 5 files in `xcore` (config.rs, mm/init.rs, task/proc.rs, vdso/resolve.rs, vdso/install.rs)
  - 2 files in `xmodules/xvdso` (arch/{riscv64,loongarch64}.rs)

  That's 14 files in one atomic commit. The plan asserts atomicity (C-11) and verification (`git grep SIGNAL_TRAMPOLINE` returns zero hits, both arches build). What's missing: a pre-commit recipe that lets the executor stage the 14 edits, run `make rv` and `make la` once, and only *then* squash to one commit. Without that, an executor will reasonably commit-as-they-go and end up needing a `git rebase -i` to squash, which AGENTS.md disallows in interactive mode.
- Why it matters:
  C-11 is the strongest atomicity claim in the spec. If the executor uses `git rebase -i` and it is blocked, they may resort to a less safe alternative (force-push, hand-fixup) and accidentally introduce an intermediate commit that has either a deleted symbol still referenced or both `SIGNAL_TRAMPOLINE` and `USER_VDSO_BASE` claiming `0x4001_0000`. The whole point of the transitional `0x4002_0000` in Phase 2 is to make Phase 4 a single atomic transition.
- Recommendation:
  Add to Phase 4: "Recipe: stage all 14 file edits without committing, run `make build ARCH=riscv64 && make build ARCH=loongarch64 && make rv && make la`, only commit once both arches are green. Use `git commit -a` (not `git commit --amend` and not `git rebase -i`) to land the single commit. If a partial mistake forces a redo, `git stash && git checkout` the affected files and start over from the staged set." This is a Phase-implementation-discipline note, not a spec change.



## Trade-off Advice

(All four trade-offs from `00_REVIEW.md` were Applied as advised in iteration 01 and remain unchanged in iteration 02. No new trade-off questions emerge in iteration 02. T-1..T-4 leans match TR-1..TR-4 from the prior review. No further trade-off advice needed.)
