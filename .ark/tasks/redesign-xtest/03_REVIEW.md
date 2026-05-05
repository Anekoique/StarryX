# `xtest` REVIEW `03`

> Status: Closed
> Feature: `xtest`
> Iteration: `03`
> Owner: Reviewer
> Target Plan: `03_PLAN.md`
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
- Blocking Issues: `0`
- Non-Blocking Issues: `2`



## Summary

PLAN 03 closes REVIEW 02's HIGH R-001 and the four LOWs cleanly. The `ROOT_FEATURES` variable plus the one-line extension to `cargo_build` is the minimal honest seam — verified against `scripts/make/cargo.mk` line 24 (extension is syntactically valid GNU Make and produces a well-formed `cargo build … --features "axfeat/…" --features "init-test"` invocation; cargo unions repeated `--features` flags) and against `scripts/make/build.mk` line 23 (`_cargo_build` calls `$(call cargo_build,$(APP),$(AX_FEAT))`; Make globals carry `ROOT_FEATURES` through with no scoping/unexport interference). The marker convention (G-12 + C-16) replaces the brittle "first non-blank line" idiom with a mechanically-checkable `# id: starry-{init,test}` comment, with V-UT-11 asserting both presence-uniqueness and cross-absence. G-8 + G-12 are jointly the new contract and PLAN 03 calls this out explicitly; V-IT-6a is correctly rewritten to assert exactly the one-line diff. Reality checks against `Cargo.toml` (root `[package] name = "starry"`, existing `[features] lwext4_rs = ["axfeat/lwext4_rs"]` — no conflict with `init-test = []`), `src/main.rs` line 64 (matches PLAN 03's quoted line verbatim), and `Makefile` line 99 (`.PHONY: all defconfig oldconfig build disasm run justrun debug clippy fmt \`) all pass. Acceptance Mapping covers G-1..G-12 and C-1..C-10, C-12..C-16 with V-* rows. Spec section reads standalone; no normative reference to "PLAN 02" or "previous iteration." T-3'' reads cleanly and rejects the alternatives on defensible grounds. Two LOW non-blocking observations remain (clippy parity and a hardenable wording in V-UT-11) — neither is worth burning the iteration cap on.



## Findings

### R-001 `cargo_clippy_root does not honour ROOT_FEATURES`

- Severity: LOW
- Section: `[**API Surface**]` (cargo.mk extension) and NG-5
- Problem:
  PLAN 03's seam extends `cargo_build` (line 23-25 of `scripts/make/cargo.mk`) but leaves the sibling macro `cargo_clippy_root` (line 29-31) unchanged. If a developer runs `make clippy ROOT_FEATURES=init-test`, clippy will be invoked without the `init-test` feature. NG-5 names "one extension to `cargo.mk`'s `cargo_build` macro" as the entire seam, making this an explicit (not accidental) scope decision.
- Why it matters:
  Practically: `init-test = []` is a no-op feature that only gates which `include_str!` arm compiles; both arms compile under either setting and there is no clippy-visible behavioural divergence. So this is not a correctness issue today. But: anyone later promoting `init-test` to gate non-trivial code (a second `cfg(feature="init-test")` arm somewhere) will silently bypass clippy on the test-build path. The PLAN doesn't note this future pitfall.
- Recommendation:
  Non-blocking. Either (a) add a one-sentence note to NG-5 that `cargo_clippy_root` is intentionally not extended because `init-test` is documentation-only (no `cfg`-gated logic beyond the `include_str!` switch), and that broadening `init-test` later requires extending `cargo_clippy_root` symmetrically; or (b) extend `cargo_clippy_root` in the same Phase 1 commit for parity (one extra line). Either is fine; I prefer (a) as cheaper and equally honest.



### R-002 `V-UT-11's "neither marker appears in the other file" is technically achievable but not assertion-ready`

- Severity: LOW
- Section: `[**Validation**] V-UT-11`
- Problem:
  V-UT-11 says "neither marker appears in the other file." Mechanically the assertion is `! grep -F 'starry-test' src/init.sh && ! grep -F 'starry-init' src/test.sh`. This works as long as `src/test.sh`'s body never legitimately mentions the string `starry-init` (e.g., in a comment about why the marker convention exists). Today `src/test.sh` is a fresh script with no such mention, so the assertion passes. But the PLAN doesn't tell a future maintainer "do not add prose like `# unlike starry-init, this script ...`," which would silently break V-UT-11.
- Why it matters:
  V-UT-11 is the long-term load-bearing assertion for C-16 marker uniqueness. A future drive-by edit to `src/test.sh` could regress it without anyone noticing the rule.
- Recommendation:
  Non-blocking. Add one sentence to C-16 (or to V-UT-11): "Neither script's body should otherwise mention the other script's marker string in prose; treat the marker comments as the only place those strings appear in `src/{init,test}.sh`." This makes the contract explicit so V-UT-11 doesn't rest on accident.



## Trade-off Advice

### TR-1 `Vendor upstream test suites`

- Related Plan Item: `T-1`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Confirm
- Advice:
  Keep T-1 as written.
- Rationale:
  Vendoring under `xtest/testsuites/` was confirmed across iterations 00..02; PLAN 03 doesn't disturb it. C-13 (per-suite license preservation) and `UPSTREAM.md` (URL + commit + import date + per-suite SPDX + per-suite local-patches summary) handle the provenance hardening that vendoring otherwise weakens.
- Required Action:
  Adopt as written.



### TR-2 `Bake on top of upstream rootfs`

- Related Plan Item: `T-2`
- Topic: Performance vs Simplicity
- Reviewer Position: Confirm
- Advice:
  Keep T-2 as written.
- Rationale:
  Reuses Alpine + busybox + musl exactly as today; the per-build multi-MB image copy is negligible on modern disks. No change needed.
- Required Action:
  Adopt as written.



### TR-3 `Build-time switch — cargo-feature form`

- Related Plan Item: `T-3'`
- Topic: Flexibility vs Safety
- Reviewer Position: Confirm
- Advice:
  Keep T-3' as written.
- Rationale:
  T-3' correctly captures that `include_str!`'s literal-only argument grammar forces the cargo-feature form (verified directly: `include_str!` requires a string-literal token; the only way to swap which file gets embedded is `#[cfg]` selection of two `let` bindings, each with its own literal). No alternative survives that constraint, so the trade-off table is honest about there being effectively one viable option.
- Required Action:
  Adopt as written.



### TR-3'' `Make-side passthrough — new ROOT_FEATURES variable`

- Related Plan Item: `T-3''`
- Topic: Flexibility vs Safety
- Reviewer Position: Confirm
- Advice:
  Keep T-3'' as written.
- Rationale:
  Verified against `scripts/make/cargo.mk` line 24 and `scripts/make/build.mk` line 23. The `$(if $(strip $(ROOT_FEATURES)),--features "$(strip $(ROOT_FEATURES))",)` expansion is well-formed in both branches: empty when unset (cargo command unchanged), `--features "init-test"` appended when set (cargo unions repeated `--features` flags, so `--features "axfeat/foo bar" --features "init-test"` is equivalent to `--features "axfeat/foo bar init-test"`). The rejection of the two alternatives is sound: special-casing inside `features.mk` would entangle `init-test` with the `axfeat/`-prefix machinery (the original bug); a generic `EXTRA_CARGO_ARGS` would weaken the contract. Minimal seam, correct semantics, future-extensible to other root-crate features.
- Required Action:
  Adopt as written.



### TR-4 `Per-suite contract — Makefile preferred, BUILD.sh fallback`

- Related Plan Item: `T-4`
- Topic: Flexibility vs Safety
- Reviewer Position: Confirm
- Advice:
  Keep T-4 as written.
- Rationale:
  Dual contract with explicit `[WARN]` on both-present (V-E-3) is the right balance for a vendored heterogeneous suite collection.
- Required Action:
  Adopt as written.



### TR-5 `Run-time failure semantics — continue on failure`

- Related Plan Item: `T-5`
- Topic: Flexibility vs Safety
- Reviewer Position: Confirm
- Advice:
  Keep T-5 as written.
- Rationale:
  C-8b + `lib/timeout.sh` together give full per-run reporting without losing later regressions to early aborts.
- Required Action:
  Adopt as written.



### TR-6 `run_qemu_tests via shared run_qemu_with_disk macro in qemu.mk`

- Related Plan Item: `T-6`
- Topic: Compatibility vs Clean Design
- Reviewer Position: Confirm
- Advice:
  Keep T-6 as written.
- Rationale:
  C-15 + V-IT-7 (the two `make -n` outputs differ only in the `-drive file=` argument) prevent drift cleanly. Phase 5's "byte-identical `make -n run` before/after refactor" check catches accidental flag changes.
- Required Action:
  Adopt as written.
