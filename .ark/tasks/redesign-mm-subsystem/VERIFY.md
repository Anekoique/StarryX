# `<feature-name>` VERIFY

> Status: Living document. Maintained by the implementer during EXECUTE → COMMIT.
> Feature: `<feature-name>`
> Target Task: `<task-slug>`
> Tier: `<quick|standard|deep>`
>
> Each checklist item resolves to PASS | FAIL (with explanation) | N/A (with explanation). Findings (`V-NNN`) capture cross-cutting observations with a Resolution. **No verdict line — completion = no `PENDING`.** Deep tier: `/ark:commit` refuses on any `PENDING`. Standard: warns and proceeds.

---

## Project Spec Compliance

> Auto-seeded from `.ark/specs/project/INDEX.md` at `task verify` time, walked recursively. Renders two subsections: `Index integrity` (one PENDING per discovered `INDEX.md` — does it enumerate all on-disk children?) and `Leaf SPECs`.
>
> Honor a rule's actuator tag (`@kind` on its first line): run the check for `tool`/`source-scan`/`test-binding`; judge `judgment` rules yourself.

### Index integrity

- [ ] `INDEX.md` enumerates all children of `specs/project/`: PENDING

### Leaf SPECs

- (none discovered): N/A

## Related Feature Spec Compliance

> Auto-seeded from PRD's `[**Related Specs**]`. Empty when none.

- (none registered): N/A

## PRD Constraints

> Auto-seeded from PRD's `[**Outcome**]` (and `[**Constraints**]` when present). One bullet per criterion.

- (none registered): N/A

## Plan Fidelity

> Auto-seeded from `PLAN.md`'s `## Spec` Goals (`G-N`). PASS when delivered, FAIL when not, N/A when withdrawn (the PLAN explains).

- (none registered): N/A

## SPEC Drift

- [ ] Modified feature SPECs have CHANGELOG entries: PENDING

## Findings

> Migrated from a pre-refactor `VERIFY.md`. The legacy verdict heading was dropped; prior findings preserved below verbatim.


### V-001 Deterministic later-apply/OOM protection injection is absent

- **Severity:** VERIFICATION GAP
- **Resolution:** INCOMPLETE; explicitly deferred by PLAN V-U-5/V-F-3.
- **Detail:** focused guest coverage proves real PROT_NONE, COW, Static, SHM,
  file-fault, and mixed-VMA validation behavior, but no deterministic hook
  forces a later apply or journal-reserve allocation failure and asserts every
  earlier Alloc/Static PTE plus all VMA flags are unchanged.

### V-002 Host xvma unit execution is unavailable

- **Severity:** ENVIRONMENT GAP
- **Resolution:** ENVIRONMENT BLOCKED.
- **Detail:** xhal intentionally rejects the macOS AArch64 host before xvma
  unit tests compile. Supported RISC-V check/build and real guest cases pass.

### V-003 Final-snapshot full OS-COMP did not complete

- **Severity:** VERIFICATION GAP
- **Resolution:** INCOMPLETE; no MM failure observed.
- **Detail:** final run `6a81a4a2-0c4c29b0-f569` hit the host's 4800-second
  deadline during lmbench after six suites passed. The focused MM and complete
  first-party cases profiles pass on the final snapshot; the older 10/10
  OS-COMP run is retained only as pre-final-IPC supplementary evidence.

### V-004 Former unconstrained typed usercopy finding

- **Severity:** HIGH (historical)
- **Resolution:** FIXED.
- **Detail:** public xuspace typed materialization is gone; private audited
  Linux ABI codecs decode fields and zero-fill output buffers.


> Cross-cutting observations that don't map to a single seeded item. Each Finding has a Resolution; `/ark:commit` requires every Resolution to be non-PENDING.

### V-001 `<short title>`

- **Severity:** CRITICAL | HIGH | MEDIUM | LOW
- **Location:** `<file:lines | "cross-file: ...">`
- **Problem:** <what's wrong>
- **Why it matters:** <impact>
- **Recommendation:** <proposed fix>
- **Resolution:** PENDING | FIXED in `<commit-or-section>` | ACCEPTED — `<reason>`

### V-002 `<short title>`

- **Severity:**
- **Location:**
- **Problem:**
- **Why it matters:**
- **Recommendation:**
- **Resolution:**

## Notes

> Free-form. Trade-offs, context for future readers, anything that doesn't fit a Finding.
