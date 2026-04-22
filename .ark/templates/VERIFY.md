# `{Feature Name}` VERIFY

> Status: Living document. Maintained by the implementer during EXECUTE → COMMIT.
> Feature: `{feature-name}`
> Target Task: `{task-slug}`
> Tier: `{quick|standard|deep}`
>
> The implementer fills the auto-seeded checklist sections as compliance is
> confirmed (PASS) or violated (FAIL with explanation) or judged irrelevant
> (N/A with explanation). The Findings section captures opinions and
> judgment calls — issues the implementer notices that don't map to a
> single seeded checklist item. **No verdict line: completion = every item
> resolved (no `PENDING`).** `/ark:commit` refuses on deep tier when any
> entry is still `PENDING`; on standard tier it warns and proceeds.

---

## Project Spec Compliance

> Auto-seeded from `.ark/specs/project/INDEX.md` at `task verify` time. One
> bullet per registered SPEC. Each SPEC's rules are stated in its own
> document; this checklist tracks compliance, not the rules themselves.

{{PROJECT_SPEC_COMPLIANCE}}

## Related Feature Spec Compliance

> Auto-seeded from PRD's `[**Related Specs**]` block. Empty when the PRD
> has no related-spec entries.

{{RELATED_FEATURE_COMPLIANCE}}

## PRD Constraints

> Auto-seeded from PRD's `[**Outcome**]` and `[**Constraints**]` (when present).
> One bullet per observable success criterion or named constraint.

{{PRD_CONSTRAINTS}}

## Plan Fidelity

> Auto-seeded from the latest `NN_PLAN.md`'s `## Spec` Goals (`G-N` entries).
> One bullet per Goal — the implementer marks it PASS when the implementation
> delivers the stated G, FAIL when not, or N/A when the goal was withdrawn
> mid-flight (the PLAN's Log explains the withdrawal).

{{PLAN_FIDELITY}}

## SPEC Drift

> Fixed-content. The implementer marks PASS once any modified feature SPEC
> (deep tier) carries a `[**CHANGELOG**]` entry recording the change.

- [ ] Modified feature SPECs have CHANGELOG entries: PENDING

## Findings

> Open-ended. Each finding is something the implementer notices during
> EXECUTE → COMMIT that doesn't map to a single seeded checklist item:
> cross-file redundancy, abstraction concerns, design trade-offs that need
> explicit acknowledgement, anything that PASSes the rules but feels off.
>
> Each finding has a Resolution. `/ark:commit` requires every Resolution to
> be non-PENDING.

### V-001 `{short title}`

- **Severity:** CRITICAL | HIGH | MEDIUM | LOW
- **Location:** `{file:lines or "cross-file: ..."}`
- **Problem:** `{what's wrong}`
- **Why it matters:** `{impact}`
- **Recommendation:** `{proposed fix}`
- **Resolution:** PENDING | FIXED in `<commit-or-section>` | ACCEPTED — `<reason>`

### V-002 `{short title}`

- **Severity:** ...
- **Location:** ...
- **Problem:** ...
- **Why it matters:** ...
- **Recommendation:** ...
- **Resolution:** ...

## Notes

> Free-form. Trade-offs, context for future readers, anything that doesn't fit
> a finding.
