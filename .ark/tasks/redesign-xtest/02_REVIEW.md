# `xtest` REVIEW `02`

> Status: Open
> Feature: `xtest`
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

- Decision: Approved with Revisions
- Blocking Issues: 1
- Non-Blocking Issues: 4



## Summary

PLAN 02 cleanly resolves all six findings from REVIEW 01 in the body of the
Spec, not merely in the Response Matrix: the `option_env!` block is gone,
`[**Build Switch**]` shows only the literal two-arm `#[cfg(feature = "init-test")]`
form, C-12 / G-11 / Architecture / API Surface / Phase 0 / V-* are
consistently re-stated around the cargo-feature mechanism, C-11 has been
deleted (constraint list now jumps C-10 → C-12), Phase 0's in-place edit
mechanism is explicit, V-IT-7's hedge is dropped, V-UT-9 covers G-10,
V-UT-10 covers C-14, and NG-7 reads cleanly. Spec self-containment holds —
no "see PLAN 01" leaks. The two-arm `#[cfg]`-on-`let` idiom is valid Rust
on stable and on the pinned `nightly-2026-03-15`; both arms bind a fresh
`init` and exactly one survives `cfg`-stripping, so there is no
"uninitialised binding" or "unused variable" warning either way.

The remaining blocker is mechanical, not architectural: the **passthrough
path PLAN 02 assumes for `--features init-test` does not exist in the
current build plumbing**. PLAN 02 says "we just need the test path to add
`init-test` to the existing list (FEATURES)." That is wrong on this
codebase. `Makefile`'s `FEATURES` is split by `scripts/make/features.mk`,
which then prepends `axfeat/` to *every* entry (line 27: `AX_FEAT :=
$(strip $(addprefix axfeat/,$(ax_feat)))`). `cargo.mk`'s `cargo_build`
macro consumes only `$(AX_FEAT)` (line 24). Adding `init-test` to
`FEATURES` produces `axfeat/init-test`, a feature on the wrong crate, and
`cargo build` fails. PLAN 02 must commit to a concrete Make-side seam for
a *root-crate* feature (separate `ROOT_FEATURES` variable, or
`features.mk` split, or extra cargo-args plumbing) before Phase 1's
acceptance line is mechanically checkable. NG-5 forbids treating this as
out-of-scope; the Make plumbing for the new feature *is* the single new
feature.

The four non-blocking issues are: V-UT-8's "first non-blank line"
assertion is a false-positive risk if both scripts share a shebang or
SPDX header; the Phase 1 acceptance line still hedges "or whatever the
project's existing feature flag passthrough syntax is" — directly
downstream of R-001; the `tests-rootfs-<arch>.img` path is duplicated in
Data Structure and API Surface without cross-reference; and the Phase 1
`.PHONY` instruction is ambiguous about extending the existing line vs
adding a new one.

On trade-offs I confirm T-1, T-2, T-3', T-4, T-5, T-6 as drafted. Approve
once R-001 lands a concrete Makefile diff for the cargo-feature
passthrough.



## Findings

### R-001 `--features init-test` has no passthrough into the existing cargo invocation

- Severity: HIGH
- Section: `[**Build Switch**]` final paragraph; `[**API Surface**]`
  Cargo features block; `## Implementation` Phase 1 bullet "Confirm the
  feature passthrough in `scripts/make/cargo.mk` / `scripts/make/build.mk`";
  `## Implementation` Phase 1 acceptance line; `## Implementation` Phase
  5 promotion bullet
- Problem:
  PLAN 02 claims `make tests` / `make run-tests` "pass `--features
  init-test` to the kernel cargo build (via the existing
  `scripts/make/cargo.mk` `features` plumbing — see Implementation
  Phase 1)." Phase 1 in turn says "we just need the test path to add
  `init-test` to the existing list (FEATURES)." Tracing the actual
  pipeline:
  - `Makefile` line 17: `FEATURES ?= fp_simd,lwext4_rs`.
  - `scripts/make/features.mk` line 7: splits `FEATURES` into a
    space-list (override).
  - `scripts/make/features.mk` line 25: appends that list into
    `ax_feat`.
  - `scripts/make/features.mk` line 27: `AX_FEAT := $(strip $(addprefix
    axfeat/,$(ax_feat)))`. **Every entry in `FEATURES` gets the
    `axfeat/` prefix.**
  - `scripts/make/cargo.mk` line 24: `cargo build … --features "$(strip
    $(2))"`, where `$(2)` is `$(AX_FEAT)` (per `build.mk` line 23
    `_cargo_build`). There is no path for a root-crate `starry`
    feature.
  Adding `init-test` to `FEATURES` therefore produces
  `axfeat/init-test`, which is a feature on the wrong crate (`axfeat`,
  not `starry`). `axfeat` does not declare it, so `cargo build` fails
  with `error: package … does not have feature axfeat/init-test`. The
  PLAN's "we just need to add `init-test` to the existing list" line
  is mechanically wrong on this codebase.
  Phase 1's acceptance line repeats the error: "`make build
  ARCH=riscv64 FEATURES=init-test` (or whatever the project's existing
  feature flag passthrough syntax is) produces a kernel ELF whose
  embedded string equals `src/test.sh`'s." Under the current plumbing
  that command will not build at all.
- Why it matters:
  This is a load-bearing mechanism — every subsequent phase
  (Phase 1's V-IT-8 dry run, Phase 5's `cargo build --features
  init-test`, V-UT-8, V-IT-8, V-F-6) depends on it working. The PLAN
  presents it as a one-liner inside an existing list, so the executor
  will not reach for a Makefile refactor and will spend a Phase 1
  session re-discovering the prefix. NG-5 ("not changing how the
  kernel is built beyond the single new `init-test` cargo feature")
  forbids treating this as out-of-scope; the Make plumbing for the new
  feature *is* the single new feature.
- Recommendation:
  Pick one and write it into [**Build Switch**] / [**API Surface**] /
  Phase 1:
  (a) Add a new top-level Make variable, e.g. `ROOT_FEATURES ?=`,
      separate from `FEATURES`. `cargo.mk`'s `cargo_build` macro
      extends its `--features` argument with `$(ROOT_FEATURES)` (no
      `axfeat/` prefix). Phase 1 sets `ROOT_FEATURES := init-test` on
      the `tests` / `run-tests` Make path. Cleanest seam; preserves
      the existing `axfeat/`-prefixing behaviour for everything else.
  (b) Special-case `init-test` inside `features.mk`: split the
      incoming `FEATURES` list into "axfeat features" and "root
      features" before prefixing. More magical; harder to extend.
  (c) Pass `--features init-test` outside the `FEATURES` plumbing
      entirely — e.g. a new `EXTRA_CARGO_ARGS` variable that
      `cargo.mk`'s `cargo_build` appends verbatim. Most flexible;
      least specific.
  Whichever option is chosen, update Phase 1's acceptance line so the
  command form matches (e.g. for option (a): `make build
  ARCH=riscv64 ROOT_FEATURES=init-test`), and update the Phase 1
  bullet from "we just need the test path to add `init-test` to the
  existing list" to a one-line description of the chosen mechanism
  with the specific files touched. My preference: option (a) — minimal
  surface area, no behavioural change for any existing caller, easy
  for the executor to land in one Phase 1 commit.



### R-002 V-UT-8 "first non-blank line" assertion is fragile against shared shebang/preamble

- Severity: LOW
- Section: `[**Validation]]` V-UT-8; `[**Validation]]` V-IT-8
- Problem:
  V-UT-8 says: "`cargo build` (no feature) produces a kernel ELF that
  contains the first non-blank line of `src/init.sh` (via `strings`);
  `cargo build --features init-test` produces a kernel ELF that
  contains the first non-blank line of `src/test.sh` and **not** that
  of `src/init.sh`." V-IT-8 uses the same `awk 'NF{print;exit}'`
  idiom. If both scripts start with `#!/bin/sh` (likely for POSIX
  scripts) or both start with the same `# SPDX-License-Identifier: …`
  header, the "first non-blank line" check is satisfied for both ELFs
  and the test gives a false positive.
- Why it matters:
  V-UT-8 / V-IT-8 are the load-bearing assertions that the build
  switch actually selected the right script. If they pass under a
  misconfiguration (e.g. both `#[cfg]` arms accidentally embed the
  same script), G-11 / C-12 are unverified.
- Recommendation:
  Use a sentinel guaranteed unique per script. Either:
  (a) require each script to have a marker comment like `# id:
      starry-init` / `# id: starry-test` near the top; assert the
      ELF contains the matching marker and not the other.
  (b) compute `sha256sum` of each script and assert the corresponding
      hash bytes appear (longer; fiddly).
  Option (a) is one comment line per script and a one-line `grep`.
  Apply the same fix to V-IT-8 and to the Phase 1 acceptance line.



### R-003 Phase 1 acceptance line uses a placeholder Make-variable form

- Severity: LOW
- Section: `## Implementation` Phase 1 acceptance line
  ("`make build ARCH=riscv64 FEATURES=init-test` (or whatever the
  project's existing feature flag passthrough syntax is)")
- Problem:
  The parenthetical hedge "(or whatever the project's existing
  feature flag passthrough syntax is)" is the same anti-pattern
  REVIEW 01 R-001 told us not to do — defer the choice to the
  executor at Phase-1 time. Once R-001 in this review picks a
  concrete passthrough variable, Phase 1's acceptance should name it.
- Why it matters:
  Acceptance lines should be mechanically checkable; "or whatever"
  isn't. Directly downstream of R-001 — fixed when R-001 lands.
- Recommendation:
  Replace the hedge with the concrete command form once R-001 is
  resolved. E.g. for option (a): "`make build ARCH=riscv64
  ROOT_FEATURES=init-test` produces a kernel ELF whose embedded
  init-script marker matches `src/test.sh`'s (per V-UT-8 fix in
  R-002)."



### R-004 `tests-rootfs-<arch>.img` path duplicated in Data Structure and API Surface

- Severity: LOW
- Section: `[**Data Structure**]` last lines under `xtest/build/`;
  `[**API Surface**]` `TESTS_ROOTFS_IMG` definition
- Problem:
  Data Structure shows `xtest/build/<arch>/tests-rootfs-<arch>.img`
  as a build-side artifact; API Surface defines
  `TESTS_ROOTFS_IMG := $(ROOT_DIR)/xtest/build/$(ARCH)/tests-rootfs-$(ARCH).img`.
  Both are correct and consistent — no contradiction — but the path
  appears in two places without cross-reference. A future reader
  editing one and not the other introduces drift.
- Why it matters:
  Pure documentation hygiene; not a correctness issue.
- Recommendation:
  Either annotate the Data Structure entry with "(exposed as
  `TESTS_ROOTFS_IMG`; see API Surface)" or drop the path from Data
  Structure and keep it only in API Surface. Either is fine.



### R-005 Phase 1 `.PHONY` edit needs to be explicit about extending vs replacing

- Severity: LOW
- Section: `## Implementation` Phase 1 last bullet
  ("Top-level `Makefile` `.PHONY` line includes `tests run-tests`.")
- Problem:
  The existing top-level `Makefile` already has a `.PHONY:` line at
  line 99 listing many targets. The Phase 1 instruction is correct in
  spirit but ambiguous about whether the executor should *extend* the
  existing line or add a *new* `.PHONY:` line. Adding a second
  `.PHONY:` line is valid GNU make but poor style.
- Why it matters:
  Style; very minor.
- Recommendation:
  Reword to "Extend the existing `.PHONY:` line in the top-level
  `Makefile` to include `tests run-tests`." One word, removes the
  ambiguity.



## Trade-off Advice

### TR-1 Vendoring upstream test suites

- Related Plan Item: `T-1`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer Option A (vendor) — confirmed
- Advice:
  T-1 unchanged from PLAN 01; reviewer position unchanged.
- Rationale:
  Hermetic builds beat upstream agility for an offline-buildable
  contest test rig. C-13 license-preservation hardening retained.
- Required Action:
  None.



### TR-2 Bake on top of upstream rootfs

- Related Plan Item: `T-2`
- Topic: Performance vs Simplicity
- Reviewer Position: Prefer Option Y (bake on top) — confirmed
- Advice:
  T-2 unchanged. Accept.
- Rationale:
  Same as REVIEW 00 / 01.
- Required Action:
  None.



### TR-3 Build-time switch — cargo-feature form

- Related Plan Item: `T-3'`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer Option A (build-time switch, cargo-feature
  form) — confirmed
- Advice:
  T-3' "Chosen" line now names the cargo-feature form explicitly,
  resolving REVIEW 01 R-001. The two-arm `#[cfg(feature = "init-test")]`
  idiom on consecutive `let` bindings is valid Rust on stable and on
  the pinned `nightly-2026-03-15` (a `#[cfg]` attribute on a `let`
  statement is accepted; exactly one arm survives `cfg`-stripping, so
  there is no "uninitialised binding" or "unused variable" warning).
  Approve in form. R-001 in this review applies to the *Make
  passthrough* into the cargo invocation, not to the kernel-side
  switch itself.
- Rationale:
  Confirmed in REVIEW 01.
- Required Action:
  None for the T-3' text; resolve R-001 in this review for the Make
  side.



### TR-4 Per-suite contract — Makefile / BUILD.sh / uniform script

- Related Plan Item: `T-4`
- Topic: Flexibility vs Safety
- Reviewer Position: Prefer dual approach — confirmed
- Advice:
  T-4 unchanged; warn-if-both acceptance retained via V-E-3.
- Rationale:
  Same as REVIEW 00 / 01.
- Required Action:
  None.



### TR-5 Failure semantics in `run-*.sh` — abort vs. continue

- Related Plan Item: `T-5`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer continue — confirmed
- Advice:
  T-5 unchanged; C-8a / C-8b split retained.
- Rationale:
  Same as REVIEW 00 / 01.
- Required Action:
  None.



### TR-6 Where `run_qemu_tests` lives

- Related Plan Item: `T-6`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer Option B (refactor qemu.mk) — confirmed
- Advice:
  T-6 unchanged; C-15 + `run_qemu_with_disk` macro retained; V-IT-7
  hedge dropped per REVIEW 01 R-005 (PLAN 02 honoured this).
- Rationale:
  Same as REVIEW 00 / 01.
- Required Action:
  None.
