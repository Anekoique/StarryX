# `{Feature Name}` PLAN `{NN}`

> Status: Draft | Revised | Approved for Implementation
> Feature: `{feature-name}`
> Iteration: `{NN}`
> Owner: Executor
> Depends on:
> - Previous Plan: `{NN-1_PLAN.md | none}`
> - Review: `{NN_REVIEW.md | none}`
> - Master Directive: `{NN_MASTER.md | none}`

---

## Summary
`{A concise summary of the proposal in this round.}`

## Log `{None in 00_PLAN}`

[**Added**]
`{newly added content}`
`{new design / validation / constraint}`



[**Changed**]
`{what changed}`
`{why it changed}`



[**Removed**]
`{what was removed}`
`{why it was removed}`



[**Unresolved**]
`{what remains open}`
`{why it is still unresolved}`



[**Response Matrix**]

| Source | ID | Decision | Resolution |
|--------|----|----------|------------|
| Review | R-001 | Accepted | `{what changed in this plan}` |
| Review | R-002 | Rejected | `{reason for rejecting it}` |
| Master | M-001 | Applied | `{how it was implemented}` |

> Rules:
> - Every prior HIGH / CRITICAL finding must appear here.
> - Every Master directive must appear here.
> - Rejections must include explicit reasoning.

---

## Spec `{Core specification}`

[**Goals**]
`{Clear definition of the "What" and "Why".}`
- G-1: ...
- G-2: ...
- G-3: ...

- NG-1: ...
- NG-2: ...



[**Architecture**]
`{System diagram or component interaction logic.}`



[**Data Structure**]
`{Core types (structs, enums, traits).}`

```rust
struct ...
enum ...
trait ...
```



[**API Surface**]
`{Function signatures and interface semantics.}`

```rust
fn ...
```



[**Constraints**]
`{Limitations and Boundaries of the API or Related Functionality.}`

- C-1: ...
- C-2: ...
- C-3: ...



## Runtime `{runtime logic}`

[**Main Flow**]
1. ...
2. ...
3. ...



[**Failure Flow**]
1. ...
2. ...
3. ...



[**State Transitions**]

- State A -> State B when ...
- State B -> State C when ...



## Implementation `{split task into phases}`

[**Phase 1**]



[**Phase 2**]



[**Phase 3**]



## Trade-offs `{ask reviewer for advice}`
`{Provide detailed possible options and their respective adv. and disadv.}`

- T-1: ...
- T-2: ...
- T-3: ...



## Validation `{test design}`
`{Specific situations require specific analysis. Parts can be none for some situations.}`

[**Unit Tests**]
- V-UT-1: ...
- V-UT-2: ...



[**Integration Tests**]
- V-IT-1: ...
- V-IT-2: ...



[**Failure / Robustness Validation**]
- V-F-1: `{validate failure behavior under ...}`
- V-F-2: `{validate retry / rollback / abort behavior}`
- V-F-3: `{validate crash / timeout / interruption handling}`



[**Edge Case Validation**]
- V-E-1: `{duplicate request}`
- V-E-2: `{empty / max / invalid input}`
- V-E-3: `{concurrency / ordering / boundary condition}`



[**Acceptance Mapping**]

| Goal / Constraint | Validation |
|-------------------|------------|
| G-1 | ... |
| C-1 | ... |
| C-2 | ... |