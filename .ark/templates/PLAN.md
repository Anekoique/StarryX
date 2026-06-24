# `<feature-name>` PLAN

> Status: Draft | Approved for Implementation
> Feature: `<feature-name>`
> Owner: Executor

---

## Summary

<one paragraph: what this PLAN proposes>

> Deep tier: REVIEW findings are folded into this PLAN in place before EXECUTE — there is no iteration history to track here.

---

## Spec

> This section is the durable design record. On deep-tier commit, it is copied **verbatim** into `specs/features/<slug>/SPEC.md`. Keep it tight: the SPEC is what future readers consult to understand what was built, not why each step happened. Why-explanations belong in `## Trade-offs`. Implementation steps belong in `## Implementation`. The Spec is the contract.

[**Goals**]

> One line per bullet, ≤80 chars, verb-led, capability-oriented (the *what*, not the *how*). Soft cap: 5. If you have more goals, you are listing implementation steps — promote them to Constraints or drop them.
>
> Good: `G-1: ark context prints a JSON snapshot of git + tasks + specs.`
> Bad:  `G-1: Two flags control output: --scope {session|phase} and --for {design|...} ...`  ← that's a Constraint.

- G-1:
- G-2:
- G-3:

[**Non-goals**]

> Only list when a reasonable reader would assume the item is in scope. Skip blanket exclusions of features nobody requested. Soft cap: 3.

- NG-1:

[**Architecture**]

> Module / file layout with a one-line note per file. Prefer a tree or diagram; avoid prose narration.

```
<directory tree or component diagram>
```

[**Data Structure**]

> Public types only. Field names + types + a one-line comment when meaning is non-obvious.

```rust
struct ...
enum ...
trait ...
```

[**API Surface**]

> Public function signatures + one-line semantics. No bodies.

```rust
fn ...
```

[**Constraints**]

> Invariants the implementation must hold, each a two-line bullet. Line 1 is the actuator tag `- C-N: @<kind>[: <arg>]` — `tool`, `source-scan` (`<pattern> @ <glob>`), `test-binding` (a test id), or `judgment`; the arg names a real test or command, never a `V-*` label. Line 2 is one declarative sentence (≤120 chars). The *why* belongs in Trade-offs, not here.
>
> Good:
> - C-1: @test-binding: <your_test_fn_name>
> ark context emits exactly one stdout write per invocation.
>
> Bad (elaboration is the *how*, belongs in Implementation): `ark context emits one stdout write: JSON via a pre-rendered string + newline, text via a single Display write. No interspersed debug prints.`

- C-1: @judgment
<constraint>
- C-2: @judgment
<constraint>

---

## Runtime

[**Main Flow**]

1.
2.

[**Failure Flow**]

1.
2.

[**State Transitions**]

- State A → State B when …

---

## Implementation

[**Phase 1**]

[**Phase 2**]

[**Phase 3**]

---

## Trade-offs

- T-1: <option A vs option B; adv. / disadv.>
- T-2:

---

## Validation

[**Unit Tests**]

- V-UT-1:

[**Integration Tests**]

- V-IT-1:

[**Failure / Robustness**]

- V-F-1: <failure / retry / rollback / crash / timeout>

[**Edge Cases**]

- V-E-1: <duplicate / empty / max / invalid input / concurrency / boundary>

[**Acceptance Mapping**]

| Goal / Constraint | Validation |
|-------------------|------------|
| G-1 | … |
| C-1 | … |
