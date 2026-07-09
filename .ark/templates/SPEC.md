[**Goals**]

> What the feature does. One line per bullet, ≤80 chars, verb-led, capability-oriented (the user-visible *what*, not the *how*). Soft cap: 5 goals. If you have more, you are listing implementation steps — promote them to Constraints or drop them.
>
> Good: `G-1: ark context prints a JSON snapshot of git + tasks + specs.`
> Bad:  `G-1: Two flags control output: --scope {session|phase} and --for {design|...} (required iff --scope=phase). Clap rejects mismatched combinations.`  ← this is implementation detail; belongs in Constraints.

- G-1:
- G-2:
- G-3:

[**Non-goals**]

> Only list a non-goal when a reasonable reader would assume it is in scope. Skip "no X" bullets where X is far outside the feature's natural reach. Soft cap: 3.
>
> Good: `NG-1: No mutation — read-only command.`
> Bad:  `NG-1: No multi-developer concepts. NG-2: No monorepo aggregation.`  ← nobody asked for those.

- NG-1:

[**Architecture**]

> Show the design, not just where code lives: name the components, what each owns, and how data / control flows between them — put shared state at the top, label the edges. Prefer a fenced-ASCII component diagram (or a layered stack / call graph). A bare file→responsibility tree is the weakest form — if you use one, add the arrows. A short module map may follow.
>
> ```
>            ┌──────── shared Layout (paths) + task.toml ────────┐
>            │                                                   │
>            ▼                                                   ▼
>      ┌───────────┐      ┌───────────┐                    ┌───────────┐
>      │  verify   │      │ spec_x    │  extract `## Spec` │  commit   │
>      │ gate: no  │─ok─▶ │ deep tier │─▶ features INDEX ─▶│ stage +   │
>      │ PENDING   │      │ → SPEC.md │   upsert (leaf→root)│ git commit│
>      └───────────┘      └───────────┘                    └─────┬─────┘
>            ▲                                                   │
>            └──────────── scoped rollback ◀── on failure ───────┘
> ```

```
<component / flow diagram — then optional module map with relationships>
```

[**Data Structure**]

> Public types only. Field names + types + a one-line comment when the meaning is non-obvious. No bodies, no derived methods unless they are part of the API.

```rust
struct ...
enum ...
trait ...
```

[**API Surface**]

> Public function signatures and their one-line semantics. No bodies. If a function's behaviour is captured by its signature + name, omit the comment.

```rust
fn ...
```

[**Constraints**]

> Invariants the implementation must hold, each a two-line bullet. Line 1 is the actuator tag `- C-N: @<kind>[: <arg>]` — `tool`, `source-scan` (`<pattern> @ <glob>`), `test-binding` (a test id), or `judgment`; the arg names a real test or command, never a `V-*` label. Line 2 is one declarative sentence (≤120 chars). The *why* belongs in PLAN's Trade-offs, not here.
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

[**CHANGELOG**]

> Appended only when a later task modifies this SPEC. New SPECs (extracted from a deep-tier PLAN at commit) start with this section empty.

- `<YYYY-MM-DD>` `<task-slug>`: <one line: what changed and why>
