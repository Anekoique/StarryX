# `xtest` REVIEW `00`

> Status: Open
> Feature: `xtest`
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

- Decision: Rejected
- Blocking Issues: 4
- Non-Blocking Issues: 8



## Summary

The PLAN is well-organised, internally consistent in its layout / data-shape /
phasing story, and the deletion + skeleton + staging contract is sound. It
fails on one load-bearing assumption: the kernel-side init mechanism. StarryX
does not exec a filesystem-resident `/init.sh` — `src/main.rs` embeds
`src/init.sh` via `include_str!` and runs the *compiled-in* string through
`busybox sh -c`. The rootfs has no `/init.sh` and no `/sbin/init` plumbing
that the bake step can hijack. Consequently every part of the design that
depends on "the existing init dispatches to `/test.sh`" (G-5, T-3, Runtime
step 5, and by extension the whole `make run-tests` boot-smoke) does not work
as written. Until the PLAN replaces this with a concrete mechanism, it cannot
be approved.

Two further blocking concerns: (1) the musl cross-prefix story for first-party
C tests is hand-waved — the contest image's musl toolchain lives at
`/opt/riscv64--musl--bleeding-edge-2020.08-1/...` and is **not** invoked as
`${PREFIX}gcc` like the glibc cross — C-10 / build-c.sh must spell this out;
and (2) a stale "emit OS-COMP markers" claim survived the recent edit and
contradicts the architecture diagram. The remaining findings are scoped fixes
(Docker fail-fast policy, provenance hygiene, run_qemu_tests realism, the
fail-fast/fail-soft asymmetry, an Acceptance Mapping gap, and a minor Phase 1
risk note). On the trade-offs, I support T-1 (vendor) and T-5 (continue);
recommend T-2 (bake-on-top) explicitly with rationale; and T-3 must be
re-grounded once R-001 is resolved.



## Findings

### R-001 `/test.sh` invocation mechanism does not match how StarryX boots

- Severity: CRITICAL
- Section: `[**Goals**]` G-5, `[**Architecture**]` (Guest panel), `[**API Surface**]` `src/test.sh` contract, `## Runtime` Main Flow step 5, `[**Trade-offs**]` T-3
- Problem:
  The PLAN claims `src/test.sh` is "the kernel-side boot script symmetric to
  `src/init.sh`" and that "the user-space init dispatches to `/test.sh` … we
  install it as the script the existing init already invokes." That is not
  how the kernel invokes user-space. `src/main.rs:64` does
  `let init = include_str!("init.sh");` and then runs
  `["/bin/busybox", "sh", "-c", init]` (lines 67–70). The script content is
  **embedded into the kernel binary at compile time**. There is no
  filesystem-resident `/init.sh` in the rootfs, no `/sbin/init` chain, no
  Alpine OpenRC handoff, no kernel parameter pointing at a path. Dropping
  `/test.sh` into the test rootfs and `cp src/test.sh -> mnt/test.sh` will
  produce a file that nothing executes.
- Why it matters:
  G-5 / G-7 / G-9 and the entire Phase 5 boot-smoke are unrealisable as
  drafted. `make run-tests` will boot, run the *unchanged compiled-in*
  `src/init.sh` (which `cd ~ && exec sh`), and never touch the test
  scaffolding. The PRD's outcome ("at least the basic suite plus a handful of
  first-party C smoke tests run end-to-end") fails. Worse, T-3's framing —
  "we control via the bake step — likely by writing/replacing the boot script
  the upstream image already runs" — actively misleads the executor about the
  shape of the fix.
- Recommendation:
  Pick one concrete mechanism and write it into the Spec, Architecture, and
  Runtime sections:
  (a) **Build-time switch** — make `src/main.rs` choose between two embedded
  scripts based on a feature flag or env var (e.g. `AX_INIT_SCRIPT` env read
  by `build.rs`, defaulting to `init.sh`; `make tests` exports
  `AX_INIT_SCRIPT=test.sh`). The kernel binary built for `make run-tests`
  embeds `src/test.sh`; the kernel for `make run` embeds `src/init.sh`. This
  is the cleanest match for how the project already works and keeps `make
  run` byte-identical (G-8). It does mean the PLAN must update NG-5 ("not
  changing how the kernel is built") and explicitly lift the build flow.
  (b) **Single embedded dispatcher** — change `src/init.sh` once to
  `[ -x /test.sh ] && exec /bin/busybox sh /test.sh; <existing body>`. Then
  the test image only needs `/test.sh`; `make run` is unaffected because
  `disk.img` has no `/test.sh`. This violates the literal reading of G-8
  ("`src/init.sh` is byte-for-byte unchanged") and G-8 must be relaxed.
  (c) Some other concrete plan — but **not** the current "the existing init
  invokes it" hand-wave.
  Whichever option is chosen, T-3's "in-image `/test.sh` location" trade-off
  becomes secondary; the *actual* trade-off is (a) vs (b) and must be added.
  Phase 4's `bake-image.sh` step "install src/test.sh -> /test.sh" stops
  being load-bearing under (a) — under (a) the script is in the kernel ELF
  and the bake step only stages the test tree.



### R-002 Stale "OS-COMP markers" claim contradicts the diagram and the user clarification

- Severity: HIGH
- Section: `[**Architecture**]` "Module decoupling" bullet 2 (line 85)
- Problem:
  Line 85 says the runtime side "only knows how to discover and execute test
  binaries and emit **OS-COMP markers**." The architecture diagram on line 78
  says "(per-test pass/fail; **no contest markers**)" and the user's recent
  edit removed scoring markers from the spec. These two statements directly
  contradict. The runtime contract for `run-all.sh` / `run-c.sh` /
  `run-suite.sh` correctly omits markers, so the bullet is residue.
- Why it matters:
  The executor reading the decoupling bullet may add `#### OS COMP TEST
  GROUP START …` lines back into `run-*.sh`, which is exactly what the user
  asked to remove and what the validation section never tests. Stale spec
  text trains the executor to write contradictory code.
- Recommendation:
  Replace "emit OS-COMP markers" with "emit a stable plain-text pass/fail
  format (e.g. `[PASS] <name>` / `[FAIL] <name> exit=<n>` and per-suite
  group headers)". Cross-check the rest of the PLAN for residue; the rest of
  the doc looks clean per `grep -in 'marker\|contest'`.



### R-003 Musl cross-prefix invocation for `build-c.sh` is unverified and likely wrong

- Severity: HIGH
- Section: `[**Constraints**]` C-10, `## Runtime` Main Flow step 3a, `## Implementation` Phase 2 first bullet
- Problem:
  The PLAN says first-party C tests are built with `${PREFIX}gcc -static`
  using "the musl cross prefix." The existing `xtest/Makefile.sub` shows the
  contest image's musl toolchain is **not** exposed as a standard `${PREFIX}`
  cross prefix — it lives at
  `/opt/riscv64--musl--bleeding-edge-2020.08-1/riscv64-buildroot-linux-musl/sysroot/...`
  and the `Makefile.sub` only ever uses `riscv64-linux-gnu-` (a glibc cross)
  as `PREFIX`. There is no analogous loongarch64 musl cross visible in the
  old plumbing at all (`build-la` only references `loongarch64-linux-gnu-`).
  Linking statically against musl on Alpine therefore needs:
  (i) the exact path/binary name of the musl gcc for both arches,
  (ii) `-static` plus possibly `-specs=` or `--sysroot=` to find musl headers,
  (iii) confirmation that a loongarch64 musl cross even exists in
  `os-contest:20250714`.
  None of this is captured.
- Why it matters:
  C-10 ("link statically against musl") and G-3 / G-9 (C tests run on
  Alpine) ride on this. If we accidentally link against glibc, the binaries
  are unrunnable on Alpine (musl) without LD shims — exactly the trap the
  old `Makefile.sub` papered over with `cp /opt/.../libc.so /lib/ld-musl-…`.
  The PLAN explicitly calls that out as something we don't want to do
  ("without dynamic-loader gymnastics").
- Recommendation:
  Before Phase 2 lands, do a one-time spike inside `os-contest:20250714`:
  `find / -name 'gcc' 2>/dev/null` and `find / -name 'libc.a' 2>/dev/null`
  on both arches. Record the exact compiler binary paths in the PLAN's Spec
  section as named env vars (e.g. `MUSL_CC_RV64`, `MUSL_CC_LA64`) with the
  full invocation needed to produce a static-musl ELF. If a loongarch64
  musl cross is missing, decide explicitly: either (a) add a build step that
  installs one, (b) downgrade C-10 to "static link, libc determined per
  arch" and accept libgcc on Alpine via Alpine's compat packages, or (c)
  drop loongarch64 first-party C tests from G-3/G-9 for this iteration. All
  three are acceptable; silence is not.



### R-004 Phase 1 demolition removes the only working pipeline before Phases 2–5 land

- Severity: MEDIUM
- Section: `## Implementation` Phase 1
- Problem:
  Phase 1 deletes `xtest/Makefile`, `Makefile.sub`, `config/`, and
  `git_testcode.sh` in a single commit, *before* any of Phases 2–5 has
  delivered a working replacement. If Phase 2 / 3 stalls (e.g. R-003 turns
  out to need extra work), the repo has no test pipeline at all for the
  duration. The PRD says the old pipeline "is dead" (init.sh boots straight
  into Alpine sh, LTP blocks commented out), so practically nothing is lost,
  but a reviewer should still flag the asymmetry.
- Why it matters:
  Bisection across Phases 2–5 against the deletion commit becomes confusing
  if a regression appears. It also means a partially-merged branch (e.g.
  Phase 1 + Phase 2 only) leaves the repo in a "no test pipeline at all"
  state.
- Recommendation:
  Either (a) confirm in the PLAN that the old pipeline is fully dead and
  unreferenced (the demolition acceptance test already does that, so just
  promote the assertion to a Spec note), or (b) keep `xtest/Makefile` etc.
  alive until Phase 5's smoke passes, deleting in a final cleanup commit.
  Option (a) is fine given the PRD's framing — just say it explicitly.



### R-005 `xtest/testsuites/UPSTREAM.md` as the only provenance record is thin

- Severity: MEDIUM
- Section: `[**Constraints**]` C-3, `[**Goals**]` G-4, `## Implementation` Phase 3
- Problem:
  Vendoring ~14 third-party test suites with a single top-level
  `UPSTREAM.md` listing one commit hash is below typical OSS provenance
  hygiene. Each upstream suite likely carries its own license (LTP is GPL-2;
  iperf, lua, busybox tests are mixed); committing them verbatim without
  preserving per-suite `LICENSE`/`COPYING` files is a real legal risk for
  redistribution. Compare with `arceos/` which is vendored as a directory
  tree with full per-crate license headers preserved.
- Why it matters:
  The repo's root license trio is "GPL-3.0-or-later OR Apache-2.0 OR
  MulanPSL-2.0" (per AGENTS.md). Adding GPL-2-only LTP sources without
  preserving their LICENSE files muddies that trio at minimum, and may
  outright violate redistribution terms for some suites.
- Recommendation:
  Strengthen C-3 and the Phase 3 import step to: "preserve every upstream
  `LICENSE`, `COPYING`, `NOTICE`, and top-level README inside each
  `xtest/testsuites/<s>/` tree verbatim. `UPSTREAM.md` records URL + commit
  + import date + license SPDX identifier per suite + summary of any local
  patches." Add a V-UT entry that asserts every `xtest/testsuites/<s>/`
  contains at least one of `{LICENSE, COPYING, COPYING.LIB, NOTICE}` or
  the upstream root file equivalent.



### R-006 `make tests` unconditionally invokes Docker; AGENTS.md treats Docker as opt-in

- Severity: MEDIUM
- Section: `[**Constraints**]` C-1, `## Runtime` Main Flow step 2, `## Implementation` Phase 5
- Problem:
  The current top-level Makefile only mentions Docker via the `docker`
  target ("enter contest docker image"); every other build target works
  without it (see AGENTS.md "Build & Run" — no Docker requirement). The PLAN
  makes Docker mandatory for `make tests` and `make run-tests`. That's a
  reasonable choice given how unportable cross toolchains are, but the
  AGENTS.md "Build & Run" surface implies non-Docker is a first-class path,
  and silently enforcing Docker for one new target may surprise contributors.
- Why it matters:
  Contributors on systems without Docker (the existing `vf2` flow doesn't
  need it) will hit a hard error on `make tests`. The CI story is also
  unstated — does GitHub Actions / contest CI already run inside that
  Docker image, or do we need to provision it?
- Recommendation:
  Either:
  (a) Add a one-liner to AGENTS.md "Testing" mentioning that `make tests` /
      `make run-tests` require Docker and document the contest image URL.
  (b) Keep Docker mandatory but add a `XTEST_NO_DOCKER=1` escape hatch for
      environments where the toolchains are already on `$PATH`, with a
      clear "you're on your own" warning. Phase 5's "Acceptance" already
      hints at this with the explicit Docker check.
  Either is fine; just commit to one.



### R-007 `run_qemu_tests` "parameterised macro" reuse claim is optimistic

- Severity: MEDIUM
- Section: `[**Architecture**]` "Top-level Makefile integration" paragraph, `[**Trade-offs**]` T-6, `## Implementation` Phase 5
- Problem:
  `scripts/make/qemu.mk` builds `qemu_args-y` as a single global variable
  (lines 29–67) that is *not* a function — `run_qemu` is a one-liner that
  expands `$(QEMU)` against `$(qemu_args-y)`. There is no current macro
  surface that takes a "disk image" argument. Adding a parameterised
  `run_qemu_tests` requires either:
  (i) overriding `DISK_IMG` before include (which won't work — qemu.mk has
       already used it on line 33 by the time the macro is called),
  (ii) factoring the BLK / NET / accel logic into a macro that takes
       `$(1) = disk image`, which is a non-trivial rewrite, or
  (iii) duplicating the qemu_args block and changing the file path.
  T-6 already concedes "may end up as a sibling macro." Plan should commit
  to which.
- Why it matters:
  If Phase 5 lands as duplicated qemu wiring, future qemu.mk changes need
  two-place edits. Worse, Phase 5's acceptance ("ARCH/BLK/NET/MEM/LOG
  passthrough" via C-5 / V-IT-4) requires the test path to keep parity with
  `run_qemu` — duplication will silently drift.
- Recommendation:
  Change qemu.mk to compute `qemu_args-y` lazily inside a macro that takes
  the disk path as `$(1)`; have both `run_qemu` and a new `run_qemu_tests`
  call it. Add a Spec note: "qemu.mk refactor: extract the disk-image arg
  into a macro parameter; existing callers of `run_qemu` pass `$(DISK_IMG)`
  explicitly." Adjust C-5 to assert parity through the shared macro.



### R-008 G-6 determinism claim hides Docker-induced non-determinism

- Severity: MEDIUM
- Section: `[**Goals**]` G-6, `## Validation` V-IT-3, V-E-4
- Problem:
  G-6 says "same inputs -> same staged tree; image bytes may differ due to
  ext4 timestamps." That is correct for the *staging* step, but the
  *binaries* in the staged tree are produced by a Docker image whose
  toolchain is pinned only by the image tag. Two contributors pulling the
  *same* image tag at different times can land different toolchain binaries
  (Docker tags are mutable). The PLAN says nothing about toolchain
  reproducibility.
- Why it matters:
  V-IT-3 ("`make tests` from top-level produces same image bytes as
  `make -C xtest all`") will pass on one machine and fail on another
  contributor's. If the team ever wants reproducible test artifacts (for
  e.g. comparing runs across branches), this is the seam that breaks.
- Recommendation:
  Either weaken G-6 to "deterministic *staging* given fixed toolchain;
  toolchain pinning is via the contest image tag" and accept binary drift,
  or pin by image digest (`@sha256:...`) instead of tag. The latter is one
  line in the Makefile and removes a real foot-gun. Update V-IT-3 to assert
  *staged tree equality* rather than *image-byte equality*.



### R-009 Build-time fail-fast vs run-time fail-soft asymmetry is not called out as Spec

- Severity: LOW
- Section: `[**Constraints**]` C-8, `## Runtime` Failure Flow items 3 & 5
- Problem:
  Failure Flow item 3 says "Suite build fails -> final exit status non-zero
  so the image isn't baked, but every suite is attempted so contributors
  see all build issues at once." Item 5 says runtime failures "never abort
  the run." This *is* the right design (build-time = developer feedback,
  must catch all errors but block the artifact; run-time = boot must
  finish so the user gets a shell). It's just never elevated to a
  first-class Spec statement, and a future contributor will read C-8
  ("failures don't abort") and assume that applies to build-time too.
- Why it matters:
  Spec drift risk; minor.
- Recommendation:
  Split C-8 into two: C-8a "build-time: any error blocks image baking but
  build-suites attempts every suite before exiting non-zero" and C-8b
  "run-time: a failing test never aborts the run." Reference C-8a from
  Phase 3 acceptance and C-8b from V-F-*.



### R-010 Acceptance Mapping is missing entries and treats Phase-acceptance as if it were Validation

- Severity: LOW
- Section: `[**Validation]]` Acceptance Mapping table
- Problem:
  Walking the table:
  - G-1..G-9: present.
  - C-1..C-10: C-10 has a row ("V-IT-2 file inspection; V-IT-4 runs on
    Alpine") — that's fine.
  - But several rows cite "Phase N acceptance check" rather than a V-*
    entry: G-2 (Phase 1), C-2 (Phase 3), C-3 (Phase 3), C-1 (Phase 5).
    Those Phase-acceptance bullets are **inside the Implementation
    section**, not the Validation section, so they don't count under the
    workflow rule "every Goal mapped to >=1 Validation." VERIFY.md will
    have nothing to check off for them.
  - Also: G-2 ("old pipeline deleted, nothing references them") needs a
    V-* item — a `git grep` assertion is trivially testable as a unit test.
- Why it matters:
  Workflow rule per `.ark/workflow.md` §4 PLAN gate: "Every Goal mapped to
  >=1 Validation." Phase acceptance bullets aren't part of the Validation
  section, so technically G-2 / C-2 / C-3 fail the gate.
- Recommendation:
  Add explicit V-UT or V-E entries:
  - V-UT-5: `git grep -L 'Makefile.sub|busybox-config-|git_testcode'`
    returns empty in the repo (G-2).
  - V-UT-6: `git status --porcelain xtest/build/` is empty after `make -C
    xtest all` (C-2).
  - V-UT-7: every `xtest/testsuites/<s>/` has a license file and the
    `UPSTREAM.md` row matches (C-3 + R-005).
  - V-IT-7 / V-F-5 already cover C-1 — fine.



### R-011 Per-arch image path needs to be exposed explicitly

- Severity: LOW
- Section: `[**Data Structure**]` build/ tree, `## Validation` V-E-5
- Problem:
  V-E-5 asserts that `make tests ARCH=riscv64` followed by `make tests
  ARCH=loongarch64` both succeed without one wiping the other's
  `xtest/build/<arch>/`. Good. What it doesn't pin: the path the top-level
  Makefile passes to qemu. The Data Structure tree shows
  `xtest/build/<arch>/tests-rootfs-<arch>.img`, which is correct, but it's
  worth elevating to API-surface text so Phase 5 doesn't drift.
- Why it matters:
  Avoids a "where is my image?" surprise during Phase 5 when wiring the
  qemu macro.
- Recommendation:
  Add a one-line API surface entry: "`make tests` writes to
  `$(ROOT_DIR)/xtest/build/$(ARCH)/tests-rootfs-$(ARCH).img`; that path is
  exported as `TESTS_ROOTFS_IMG` for `run_qemu_tests`." Already implied by
  C-4 but worth being explicit.



### R-012 V-IT-6 is the wrong shape for verifying G-8

- Severity: LOW
- Section: `[**Goals**]` G-8, `## Validation` V-IT-6
- Problem:
  V-IT-6 says "`make run` behaves identically before and after the change —
  same disk image used, same init.sh, same boot output up to the user
  shell." "Same boot output" is hard to assert mechanically (timestamps,
  PCI scan order). What G-8 actually cares about is bit-identical
  `src/init.sh` and a kernel ELF that still embeds the original init
  script.
- Why it matters:
  Without a sharper assertion, R-001's recommendation (a) — embedding
  `init.sh` vs `test.sh` based on env — could subtly change the kernel ELF
  for `make run` if not done carefully (e.g. unconditional `build.rs`
  changes). A diffable check catches that.
- Recommendation:
  Replace V-IT-6 with two assertions:
  - V-IT-6a: `git diff` on `src/init.sh` is empty across the task branch.
  - V-IT-6b: `make build ARCH=riscv64` produces a kernel binary whose
    embedded init-script string equals the contents of `src/init.sh`
    (e.g. `strings starry_*.elf | grep -F "$(head -1 src/init.sh)"`).
  These are mechanically checkable and survive R-001's resolution under
  either option (a) or (b).



## Trade-off Advice

### TR-1 Vendoring upstream test suites

- Related Plan Item: `T-1`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer Option A (vendor)
- Advice:
  Keep the vendor decision. The user already directed it; it's the right
  call for an offline-buildable contest kernel. The drawbacks the PLAN
  lists (repo size, manual upstream sync) are real but small versus the
  alternative (fetch-on-build dies the moment the upstream repo moves or
  contest mirrors block GitHub).
- Rationale:
  Hermetic builds beat upstream agility for a test rig that needs to work
  inside `os-contest:20250714` with no network. The repo-size cost is a
  one-time hit; submodule fragility (especially around `.git` files inside
  Docker bind-mounts and CI clones) would bite repeatedly.
- Required Action:
  Keep T-1 as is. Strengthen provenance per R-005 (add license preservation
  + per-suite license SPDX in `UPSTREAM.md`). No further comparison needed.



### TR-2 Bake on top of upstream rootfs vs. build a new rootfs from scratch

- Related Plan Item: `T-2`
- Topic: Performance vs Simplicity
- Reviewer Position: Prefer Option Y (bake on top)
- Advice:
  Confirm bake-on-top. The PRD says "we bake everything into a *copy* of
  the upstream Alpine `rootfs-$ARCH.img`" — that wording is unambiguous;
  the user's "we need a new rootfs" reads as "a new derived image," not
  "build Alpine from scratch."
- Rationale:
  Building Alpine from scratch costs us the entire Alpine package set
  (busybox, ash, the dynamic linker, /etc/profile, /etc/passwd, /dev/null
  population) for zero new capability. The Alpine image is tiny and
  battle-tested in `make run` already; replacing it would mean re-vendoring
  everything that `qemu_rootfs` currently downloads. The bake step's
  "every test rootfs build re-copies a multi-MB image" cost is a non-issue
  on modern disks (the upstream image is ~30–60 MB; rsync + ext4 mkfs
  isn't on the hot path). Option X expands Phase 4 from "copy + rsync +
  install" to "build a userland from packages" — a separate, weeks-long
  subproject.
- Required Action:
  Update T-2's "Chosen" line from "(assumed)" to "Confirmed: bake on top
  per reviewer guidance." Drop the "Reviewer: confirm…" prompt.



### TR-3 In-image `/test.sh` location

- Related Plan Item: `T-3`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Need More Justification — depends on R-001
- Advice:
  T-3 as drafted answers the wrong question ("where in the image does
  `/test.sh` live") because R-001 establishes that no in-image init reads
  `/test.sh` at all. After R-001 is resolved, replace T-3 with a trade-off
  about *how the kernel selects which embedded script to run*:
  - Option A: build-time switch via env (R-001 recommendation (a)).
    Adv: keeps `make run` byte-identical, makes the test/normal path a
    pure build-time choice. Disadv: `build.rs` complexity, two kernel
    binaries.
  - Option B: dispatcher in `src/init.sh` (R-001 recommendation (b)).
    Adv: one kernel binary, one init flow. Disadv: violates "init.sh
    byte-for-byte unchanged"; tiny runtime cost on `make run` to check
    `[ -x /test.sh ]`.
  My preference is **Option A** — the build-time switch is cleaner and
  matches how the project already uses env-driven `axconfig`/feature
  flags (per AGENTS.md "AX_* env vars"). A single-line `build.rs` reading
  `AX_INIT_SCRIPT` (defaulting to `init.sh`) keeps the `include_str!`
  idiom intact.
- Rationale:
  Option A leaves `make run` provably identical (G-8) and isolates all
  test-rig changes inside the test build. Option B couples the two paths
  through a script the kernel must always execute.
- Required Action:
  In the next PLAN iteration, delete the current T-3 and replace with the
  A vs B trade-off above. Pick A unless there's a concrete obstacle.



### TR-4 Per-suite contract — `Makefile` vs. `BUILD.sh` vs. uniform script

- Related Plan Item: `T-4`
- Topic: Flexibility vs Safety
- Reviewer Position: Prefer the chosen dual approach
- Advice:
  Keep the dual contract (Makefile preferred, BUILD.sh fallback,
  copy-only as last resort). It's the only realistic choice given the
  upstream suites' heterogeneity (busybox/ltp ship Makefiles, lua needs
  a configure dance, basic is mostly a script bundle).
- Rationale:
  A uniform script would require us to patch each upstream suite into
  shape, multiplying the per-suite local-patch surface that C-3 already
  warns about. Keeping the contract pluggable matches what each upstream
  already exposes.
- Required Action:
  Add one acceptance: `xtest/scripts/build/build-suites.sh` warns (not
  errors) when a suite has both `Makefile` and `BUILD.sh` — the dispatch
  rule should be deterministic and visible.



### TR-5 Failure semantics in `run-*.sh` — abort vs. continue

- Related Plan Item: `T-5`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer continue (the chosen option)
- Advice:
  Keep continue-on-failure (C-8). Mandatory for a regression rig — one
  hung mq_open01 must not eat the rest of the suite.
- Rationale:
  The contest / CI use case wants the full report each run.
  `lib/timeout.sh` covers the hang case adequately. Aborting would bury
  later regressions behind earlier ones.
- Required Action:
  Per R-009, split C-8 into C-8a (build-time fail-fast at suite boundary)
  and C-8b (run-time fail-soft). T-5 itself doesn't need to change.



### TR-6 Where `run_qemu_tests` lives

- Related Plan Item: `T-6`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer Option B (refactor qemu.mk to take the disk arg)
- Advice:
  Take the refactor cost upfront. Per R-007, qemu.mk's current structure
  doesn't naturally accept a parameterised macro; if we duplicate, the
  two paths drift. A small refactor that makes the macro itself take
  `$(1) = disk image` (or sets `DISK_IMG` lazily inside the macro) costs
  ~10 lines and makes `run_qemu_tests` a one-liner.
- Rationale:
  C-5 ("`make tests` accepts the same ARCH/BLK/NET/MEM/LOG overrides as
  `make rv` / `make la`") is implausible to maintain across two duplicated
  qemu_args blocks; a single shared macro is the only durable answer.
- Required Action:
  Phase 5's first bullet should call out the qemu.mk refactor explicitly:
  "extract disk-image arg from `$(DISK_IMG)` global into a macro parameter;
  `run_qemu` continues to use `$(DISK_IMG)`, `run_qemu_tests` passes the
  test image path." Add a Validation that both `make rv` and `make
  run-tests ARCH=riscv64` produce a sensible `qemu-system-riscv64` command
  (use `make -n run` / `make -n run-tests` snapshot diffing).
