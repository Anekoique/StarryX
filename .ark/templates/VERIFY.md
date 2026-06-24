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

{{PROJECT_SPEC_COMPLIANCE}}

## Related Feature Spec Compliance

> Auto-seeded from PRD's `[**Related Specs**]`. Empty when none.

{{RELATED_FEATURE_COMPLIANCE}}

## PRD Constraints

> Auto-seeded from PRD's `[**Outcome**]` (and `[**Constraints**]` when present). One bullet per criterion.

{{PRD_CONSTRAINTS}}

## Plan Fidelity

> Auto-seeded from `PLAN.md`'s `## Spec` Goals (`G-N`). PASS when delivered, FAIL when not, N/A when withdrawn (the PLAN explains).

{{PLAN_FIDELITY}}

## SPEC Drift

- [ ] Modified feature SPECs have CHANGELOG entries: PENDING

## Findings

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
