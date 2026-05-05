# `xtest` REVIEW `01`

> Status: Open
> Feature: `xtest`
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

- Decision: Approved with Revisions
- Blocking Issues: 1
- Non-Blocking Issues: 6



## Summary

PLAN 01 substantively addresses the entire 00_REVIEW finding set. The
Response Matrix accounts for R-001..R-012 and TR-1..TR-6, and each
acceptance is mirrored in the Spec / Architecture / Validation /
Trade-offs sections rather than left as narrative. The build-time-switch
redesign (G-11 / C-12 / T-3') resolves R-001's load-bearing flaw in
direction, and the qemu.mk shared-macro commitment (C-15 + new
`run_qemu_with_disk` macro), license preservation (C-13), digest pin
(C-14), and C-8a/C-8b split all land cleanly. Spec self-containment is
largely respected — the document reads standalone, though one stale
reservation (`C-11`) leaks into the SPEC-bound section.

The remaining blocker is squarely in the same class as the original
R-001: PLAN 01's `[**Build Switch**]` section presents *two* candidate
forms for the kernel-side switch and identifies the wrong one as
primary. `include_str!` is a compiler-builtin macro whose path argument
must be a string-literal token at expansion time; it does not accept a
`const &str` binding under any current Rust toolchain (stable, beta,
nightly-2026-03-15). The "Option_env form" shown first will fail with
`error: argument must be a string literal`. The fallback `cfg!`-feature
form is the only one that compiles. PLAN 01 must commit to the feature
form (or an equivalently literal-only mechanism such as two
`include_str!` arms behind `#[cfg]`) and stop describing the broken form
as the preferred path. Once that one section is rewritten, the plan is
executable as drafted.

The remaining six findings are non-blocking: a dead `C-11` reservation,
a phase-mid PLAN-edit mechanism that needs a sentence of clarification,
a G-10 mapping that still cites Phase deliverables instead of a V-*
entry, a "test-only flags" hedge in V-IT-7 that softens C-15, a C-14 row
that bottoms out in inspection rather than a runnable check, and a
non-goal (NG-7 macOS exclusion) the user may want flagged before
approval. None block execution.

On trade-offs I confirm T-1, T-2, T-4, T-5, T-6 as drafted. T-3' must
move its preferred-form pointer onto the only form that compiles.



## Findings

### R-001 `include_str!(CONST)` does not compile; pick the feature form unambiguously

- Severity: HIGH
- Section: `[**Build Switch**]` (the "Option_env form" code block);
  `[**Constraints**]` C-12; `## Implementation` Phase 0 last sub-bullet
  ("Decide which form … by trying both")
- Problem:
  `[**Build Switch**]` shows two forms and frames them as alternatives
  "both produce identical runtime behaviour." The first form is:
  ```rust
  const INIT_SCRIPT: &str = match option_env!("AX_INIT_SCRIPT") {
      Some(s) => s,
      None => "init.sh",
  };
  let init = include_str!(INIT_SCRIPT);
  ```
  This does not compile. `include_str!` is a built-in procedural macro
  that inspects its argument **at macro-expansion time**, before name
  resolution reaches the `const`. Its argument grammar accepts only a
  string-literal token (or a literal-producing macro such as
  `concat!`/`env!`/`option_env!` whose evaluation also happens at
  expansion time and yields a literal). A `const &str` binding is a
  *named* `&'static str`, not a literal, and the compiler rejects it with
  `error: argument must be a string literal`. This is not a
  toolchain-version quirk; it is a long-standing constraint of the
  `include_str!` macro and applies on the pinned `nightly-2026-03-15`.
  C-12 inherits the ambiguity ("via `option_env!` (or, fallback, an
  `init-test` cargo feature; Phase 0 picks the form)"), and Phase 0's
  acceptance bullet ("decide which form to use by trying both in a
  one-line `src/main.rs` test") implies the executor should burn time
  confirming a known-broken form.
- Why it matters:
  This is the same class of issue as R-001 in REVIEW 00 — a load-bearing
  mechanism documented in the Spec that does not work as written. Phase
  0 is scheduled to "pick" a form by experiment, but the `option_env!`
  form is guaranteed to fail. If the executor follows the PLAN literally
  they will spend a Phase 0 session re-discovering this and then have to
  write a Spec-amendment commit before Phase 1 can land. More
  structurally, PLAN 01's Spec is what gets promoted to
  `specs/features/xtest/SPEC.md` on archive; shipping a SPEC that
  describes a broken-then-discarded form is bad provenance.
- Recommendation:
  In the next PLAN iteration:
  1. Delete the `option_env!` + `const` + `include_str!(CONST)` block
     from `[**Build Switch**]`. Keep only the literal-arm form:
     ```rust
     #[cfg(feature = "init-test")]
     let init = include_str!("test.sh");
     #[cfg(not(feature = "init-test"))]
     let init = include_str!("init.sh");
     ```
     Both arms are string literals, so both expand cleanly. `make tests`
     / `make run-tests` add `--features init-test` to the cargo
     invocation; `make run` does not.
  2. Rewrite C-12 to drop the `option_env!` mention. Replace
     `AX_INIT_SCRIPT` env-var framing with a feature-flag framing
     consistently across G-11, C-12, the Architecture diagram (which
     still prints `AX_INIT_SCRIPT=test.sh`), Phase 0, Phase 1, Phase 5,
     Failure Flow item 8, V-UT-8, V-IT-8, and V-F-6.
  3. Delete the "Decide which form" sub-bullet from Phase 0; replace
     with "Confirm `cargo build --features init-test` succeeds with the
     literal `include_str!(\"test.sh\")` arm and that the resulting ELF
     embeds `src/test.sh`."
  4. Note in the `## Log`'s `[**Changed**]` section that the
     `option_env!` form was removed because `include_str!` does not
     accept named bindings.
  Equivalently acceptable: if the executor wants an env-driven switch,
  do the dispatch in `build.rs` (write a generated `init_script.rs`
  containing `pub const INIT: &str = include_str!("…");` with the path
  resolved at build-script time and emitted as a literal). That is a
  heavier refactor and the feature-flag form is simpler; flag it as an
  Option in T-3' if you want to keep both on the table, but pick *one*.



### R-002 `C-11: (reserved …)` leaks dead numbering into the future SPEC

- Severity: LOW
- Section: `[**Constraints**]` C-11
- Problem:
  C-11 reads `(reserved — was old C-11; promoted to C-15.)`. This is
  cross-iteration bookkeeping leaking into the part of PLAN 01 that gets
  promoted verbatim to `specs/features/xtest/SPEC.md` on archive
  (workflow §4 PLAN gate: "`## Spec` must stay self-contained every
  iteration"). A future reader of the feature SPEC will see a phantom
  constraint with no content and wonder what was meant.
- Why it matters:
  Not blocking, but the SPEC will outlive the PLAN. The PLAN's `## Log`
  already records the renumbering history; the Spec section does not
  need to.
- Recommendation:
  Either (a) delete the C-11 line entirely and renumber nothing
  (C-12..C-15 keep their numbers; C-11 is simply absent — the constraint
  list is not required to be dense), or (b) renumber C-12..C-15 down by
  one so the Spec's constraints are 1..14 with no gaps. Option (a) is
  less churn and preserves the Response-Matrix references; preferred.



### R-003 Phase 0 records into the PLAN; clarify the edit mechanism

- Severity: MEDIUM
- Section: `## Implementation` Phase 0 (third bullet "Record results
  into the PLAN's `[**API Surface**]`"), Phase 0 acceptance line
- Problem:
  Phase 0 instructs the executor to record `MUSL_CC_RV64`,
  `MUSL_CC_LA64`, and the `DOCKER_IMAGE` digest into the PLAN's
  `[**API Surface**]` section *before any code commit*. By the time
  Phase 0 runs, PLAN 01 is already Approved and execution is underway.
  The workflow allows in-flight Spec amendments ("if implementation
  reveals design gaps, update the latest PLAN's `## Spec` section to
  reflect reality" — workflow §4 EXECUTE), but PLAN 01 does not say
  which mechanism to use: amend the current iteration in place, append
  to `## Log` under a new heading, or open a new PLAN iteration.
- Why it matters:
  Mid-execute Spec edits are a known seam — without a written rule the
  executor may either silently rewrite the Approved Spec (losing the
  audit trail of "what was Approved" vs "what was actually built") or
  open a new PLAN iteration (which forces another REVIEW pass for what
  is purely factual data capture). Either is defensible; the PLAN should
  pick one.
- Recommendation:
  Add a one-line rule at the bottom of Phase 0: "Phase 0 results are
  appended to the existing PLAN's `## Log` under a new `[**Phase 0
  Results**]` heading, and the placeholder strings in `[**API
  Surface**]` are replaced in place. No new PLAN iteration is required —
  Phase 0 produces only factual capture, not design change."



### R-004 G-10 has no V-* entry; either add one or weaken testability

- Severity: LOW
- Section: `[**Validation]` Acceptance Mapping row for G-10;
  `[**Goals]` G-10
- Problem:
  Acceptance Mapping cites `Phase 6 deliverables; VERIFY checklist` for
  G-10. Per workflow §4 PLAN gate ("Every Goal mapped to ≥1
  Validation"), Phase deliverables and the VERIFY checklist itself are
  not Validation entries — they live under `## Implementation` and
  `VERIFY.md` respectively, not under `## Validation`. REVIEW 00's R-010
  made exactly this correction for G-2/C-2/C-3 (resolved with
  V-UT-5/6/7). G-10 is the same shape and slipped through.
- Why it matters:
  Symmetry with R-010's resolution; minor trace-table hygiene.
- Recommendation:
  Add a one-line V-UT entry, e.g. "V-UT-9: `grep -F 'make tests'
  AGENTS.md` and `grep -F 'make run-tests' AGENTS.md` both return
  non-empty after Phase 6; `xtest/README.md` exists and is non-empty."
  Map G-10 to V-UT-9.



### R-005 V-IT-7 hedge ("any test-only flags") softens C-15

- Severity: LOW
- Section: `[**Validation]` V-IT-7
- Problem:
  V-IT-7 asserts that `make -n run` and `make -n run-tests` "differ only
  in the `-drive file=` argument (and any test-only flags)." The
  parenthetical "(and any test-only flags)" is open-ended. C-15 is
  precisely the no-drift constraint; allowing unspecified test-only
  flags into the command line undercuts it.
- Why it matters:
  In practice the qemu.mk refactor either parameterises just the disk
  image or it parameterises more; whichever it is should be enumerated,
  not hedged.
- Recommendation:
  Either (a) drop the hedge and assert exact equivalence except for the
  `-drive file=` argument, or (b) enumerate the allowed test-only flags
  up front (e.g. extra `-monitor`, longer `-serial` capture). I'd take
  (a) and let any test-only flag arrive via the same plumbing
  (`BLK`/`NET`/`MEM`/`LOG`) C-5 already promises.



### R-006 C-14 row in Acceptance Mapping bottoms out in "inspection of"

- Severity: LOW
- Section: `[**Validation]` Acceptance Mapping row for C-14
- Problem:
  The C-14 row reads `inspection of xtest/Makefile DOCKER_IMAGE
  post-Phase-0; V-IT-1 reproducibility`. "Inspection" without a runnable
  check is the same anti-pattern R-010 flagged in REVIEW 00. V-IT-1 only
  asserts the build succeeds, not that the digest pin is in effect.
- Why it matters:
  Borderline — a `grep` on `xtest/Makefile` is mechanical, but it is not
  written down as a V-* entry, so VERIFY.md will have nothing to tick.
- Recommendation:
  Add `V-UT-10: grep -E
  'docker\.educg\.net/cg/os-contest@sha256:[a-f0-9]{64}' xtest/Makefile
  returns one match; the same regex on a tag-form mention returns
  none.` Map C-14 to V-UT-10 + V-IT-1.



### R-007 NG-7 excludes macOS-native; flag for user confirmation

- Severity: LOW
- Section: `[**Goals]` NG-7
- Problem:
  NG-7 explicitly excludes a macOS-native path and requires macOS
  contributors to use Docker. This is a deliberate Phase 0 / R-006
  outcome, but the user noted in design conversation that many StarryX
  contributors are on macOS. PLAN 01 commits to Docker-only without an
  escape hatch (R-006 explicitly rejected `XTEST_NO_DOCKER`).
- Why it matters:
  Not blocking — Docker on macOS works fine — but the user may want this
  reflected as a known DX cost rather than buried in NG-7.
- Recommendation:
  No spec change needed. Surface this in the verdict summary so the user
  has a chance to push back before approving (already done in the
  Summary above). If the user is fine with Docker-only on macOS, leave
  NG-7 as is.



## Trade-off Advice

### TR-1 Vendoring upstream test suites

- Related Plan Item: `T-1`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer Option A (vendor) — confirmed
- Advice:
  Keep T-1 as drafted. R-005 in REVIEW 00 → C-13 in PLAN 01 raises the
  provenance bar appropriately.
- Rationale:
  Same as REVIEW 00. Hermetic builds beat upstream agility for a contest
  test rig.
- Required Action:
  None.



### TR-2 Bake on top of upstream rootfs vs. build from scratch

- Related Plan Item: `T-2`
- Topic: Performance vs Simplicity
- Reviewer Position: Prefer Option Y (bake on top) — confirmed
- Advice:
  T-2 promoted to "Confirmed" in PLAN 01. Accept.
- Rationale:
  Same as REVIEW 00.
- Required Action:
  None.



### TR-3 Build-time switch (Option A) vs init.sh dispatcher (Option B)

- Related Plan Item: `T-3'`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer Option A (build-time switch) — confirmed in
  *direction*, blocked on *form*
- Advice:
  The Option A direction is right (keeps `make run` provably
  byte-identical per G-8). The form picked inside Option A is wrong —
  see R-001. Once the form switches from `option_env!` to `--features
  init-test`, T-3' is approved.
- Rationale:
  Build-time switching matches the project's existing AX_*-driven
  feature flag idiom. The feature-flag form does it without bumping
  into the `include_str!` literal-only constraint.
- Required Action:
  Rewrite T-3''s "Chosen" line to specify the cargo-feature form, and
  drop the env-var phrasing.



### TR-4 Per-suite contract — Makefile vs. BUILD.sh vs. uniform script

- Related Plan Item: `T-4`
- Topic: Flexibility vs Safety
- Reviewer Position: Prefer the chosen dual approach — confirmed
- Advice:
  T-4 unchanged in PLAN 01; warn-if-both acceptance landed via V-E-3.
  Accept.
- Rationale:
  Same as REVIEW 00.
- Required Action:
  None.



### TR-5 Failure semantics in run-*.sh — abort vs. continue

- Related Plan Item: `T-5`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer continue — confirmed
- Advice:
  C-8a/C-8b split landed. Accept.
- Rationale:
  Same as REVIEW 00.
- Required Action:
  None.



### TR-6 Where run_qemu_tests lives

- Related Plan Item: `T-6`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Prefer Option B (refactor qemu.mk) — confirmed
- Advice:
  C-15 + `run_qemu_with_disk` macro contract landed. Accept. R-005
  above asks the executor to tighten V-IT-7's hedge so the no-drift
  assertion bites.
- Rationale:
  Same as REVIEW 00.
- Required Action:
  Tighten V-IT-7 per R-005.
