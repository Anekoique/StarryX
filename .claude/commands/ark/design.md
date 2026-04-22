---
description: Start a standard or deep-tier task. Produces PRD → PLAN → (REVIEW loop if --deep) → EXECUTE → VERIFY.
argument-hint: "[--deep] <title>"
---

# `/ark:design $ARGUMENTS`

Create a standard-tier task (default) or deep-tier task (if `--deep` is in arguments).

- **Standard** — feature work with testable scope. Single PLAN, no REVIEW loop, single VERIFY gate.
- **Deep** — architectural or cross-cutting work. Iterated PLAN ↔ REVIEW loop, VERIFY gate, SPEC extracted on archive.

Parse `$ARGUMENTS`:
- If it contains `--deep`, tier = `deep`, title = remainder.
- Otherwise, tier = `standard`, title = `$ARGUMENTS`.

Structural operations (creating task dirs, phase transitions, archive moves, SPEC extraction, index upserts) are handled by the `ark agent` CLI — do not hand-edit `task.toml` or move directories with `mv`, except where this command later explicitly instructs a deep-tier iteration or reopen update (for example, bumping `iteration` and resetting `phase`). Artifact bodies (PRD prose, PLAN sections, REVIEW findings) are yours to write.

## Preconditions

- `.ark/` is initialized.
- For **standard**: change is feature-scoped, testable, doesn't break APIs or architecture. If it does, use `--deep`.
- For **deep**: change is architectural, cross-cutting, or introduces a new subsystem.

## Phase 1 — DESIGN

### 1.1 Pull design-phase context

```bash
ark context --scope phase --for design --format json
```

This bundles git state, current task (if any), project specs, feature specs, and recent archive in one structured snapshot. Read the returned JSON before reading the workflow doc — it tells you what specs to consult.

`.ark/workflow.md` is worth a quick re-read if you haven't seen it recently:

```bash
cat .ark/workflow.md
```

Then read every SPEC referenced under `specs.project` and any `specs.features` rows that look related to the task — they're all listed in the context output.

### 1.3 Brainstorm with the user

**Standard tier** — ask up to **3** clarifying questions focused on what's ambiguous in the title. Examples:
- What's the observable outcome?
- Any constraints you'd call out up front?
- Any existing patterns I should follow?

**Deep tier** — conduct a thorough brainstorm covering:
- Problem framing and non-goals
- Boundaries and constraints (performance, security, compatibility)
- Alternatives considered and why rejected
- Risks and assumptions
- Interaction with existing feature SPECs

Do not proceed until the user confirms direction.

### 1.4 Create the task

Derive slug from title: lowercase, hyphen-separated, ASCII, ≤40 chars.

```bash
# standard tier — opt in to --worktree only if work would collide with in-flight changes on the active branch
ark agent task new --slug <slug> --title "<title>" --tier standard

# deep tier — --worktree is REQUIRED
ark agent task new --slug <slug> --title "<title>" --tier deep --worktree
```

This scaffolds `.ark/tasks/<slug>/` with `PRD.md` + `task.toml` (`phase = design`, `iteration = 0`), registers the slug in `.ark/.state.toml` as this session's focus, and warns to stderr if other active tasks already exist. Refuses if the slug already exists.

**Deep tier MUST use `--worktree`.** After scaffolding, `cd .ark/worktrees/<branch>/` and run all subsequent phase commands (plan / review / execute / verify / archive) from the worktree.

### 1.5 Fill the PRD

Edit `.ark/tasks/<slug>/PRD.md`:
- **What** — one-line description
- **Why** — the reason
- **Outcome** — observable success criteria
- **Related Specs** — feature specs this task touches (list each path with one line on how it interacts)

## Phase 2 — PLAN

### 2.0 Refresh phase context

```bash
ark context --scope phase --for plan --format json
```

This narrows the snapshot to current task + PRD + related feature specs (filtered to those mentioned in the PRD's `[**Related Specs**]`) + project specs.

### 2.1 Advance phase

```bash
ark agent task plan
```

This transitions `task.toml.phase` to `Plan` and seeds `00_PLAN.md` from the embedded PLAN template.

### 2.2 Fill the PLAN

Using the PRD and related specs as input, fill `00_PLAN.md`:

- Frontmatter: Status = `Draft`, Iteration = `00`, Depends on = PRD + related specs
- `## Summary` — what this PLAN proposes
- `## Log` — *None in 00_PLAN*
- `## Spec` — Goals (G-N), Non-goals (NG-N), Architecture, Data Structure, API Surface, Constraints (C-N)
- `## Runtime` — Main Flow, Failure Flow, State Transitions
- `## Implementation` — phases (Phase 1, Phase 2, Phase 3 as needed)
- `## Trade-offs` — options with adv./disadv. (T-N)
- `## Validation` — Unit/Integration/Failure/Edge tests (V-*-N), Acceptance Mapping linking each G/C to a V

**Gate:** every Goal (G-N) must be mapped to at least one Validation (V-*-N) in the Acceptance Mapping table.

### 2.3 Advance

**Standard tier:** `ark agent task execute` — skip to Phase 4.

**Deep tier:** `ark agent task review` — proceed to Phase 3.

## Phase 3 — REVIEW (deep tier only — plan review loop)

### 3.0 Refresh phase context

```bash
ark context --scope phase --for review --format json
```

Returns current task + latest PLAN + related feature specs + project specs — exactly what a reviewer needs to evaluate the plan.

### 3.1 Review is seeded

`ark agent task review` (called in 2.3) has already transitioned to Review and seeded `00_REVIEW.md`.

### 3.2 Act as reviewer

Ideally, this is a fresh agent or a reviewer model. For this command, ask the user: *"Should I self-review, or will you run the reviewer?"*

If self-review: switch framing — *you are now the reviewer*. Read the latest `NN_PLAN.md` critically against the PRD and project specs. Fill the matching `NN_REVIEW.md` with:

- Verdict (Approved / Approved with Revisions / Rejected)
- Blocking / Non-blocking counts
- Findings (R-NNN) with Severity, Section, Problem, Why it matters, Recommendation
- Trade-off Advice (TR-N)

### 3.3 Loop if revisions needed

If verdict is *Rejected* or *Approved with Revisions*:

1. Copy `.ark/templates/PLAN.md` to `NN+1_PLAN.md` (next iteration number).
2. Copy `.ark/templates/REVIEW.md` to `NN+1_REVIEW.md`.
3. Edit `task.toml`: bump `iteration` to `NN+1`, set `phase = "plan"`, update `updated_at`.
4. Fill `NN+1_PLAN.md`:
   - Fill the `Response Matrix` in `## Log` — every prior CRITICAL/HIGH finding must appear with Accepted/Rejected/Deferred + reasoning.
   - Revise the relevant sections. `## Spec` must stay self-contained (deltas go in `## Log`); it is the body of the future feature SPEC.
5. `ark agent task review` — transition back to Review, ready for the next review pass.
6. Fill `NN+1_REVIEW.md` with the next verdict.
7. Repeat until verdict is *Approved* (zero open CRITICAL).

**Max iterations** is recorded in `task.toml.max_iterations` (agent-judged — typically 3–5 for deep). If exhausted without approval, halt and ask the user how to proceed.

### 3.4 Advance

When the latest REVIEW is *Approved* with zero open CRITICAL:

```bash
ark agent task execute
```

## Phase 4 — EXECUTE

### 4.0 Refresh phase context

```bash
ark context --scope phase --for execute --format json
```

Returns current task + latest PLAN + git dirty files + project specs. Use the dirty-files list to know what's already in flight before you start editing.

### 4.1 Implement the plan

Work through the latest PLAN's Implementation phases. Follow project specs and related feature SPECs.

If implementation reveals gaps in the design, **update the latest PLAN's `## Spec` section** to reflect reality. Do not silently diverge.

### 4.2 Run checks

Run whatever checks the project enforces (tests, lints, builds). Implementation is complete when checks pass and code is committed (by the user).

### 4.3 Advance

Once code is committed:

```bash
ark agent task verify
```

This transitions to Verify and seeds `VERIFY.md` from the embedded template.

## Phase 5 — VERIFY

### 5.0 Refresh phase context

```bash
ark context --scope phase --for verify --format json
```

Returns current task with PRD + latest PLAN + VERIFY.md path (if exists) + git state — the inputs a verifier needs to check plan-fidelity, correctness, and SPEC drift.

### 5.1 Act as verifier

Ideally a fresh agent or reviewer model. If self-verifying, apply the **higher quality bar**: this is not just "does it work" — it covers plan fidelity, correctness, code quality, organization, abstraction, and SPEC drift.

Fill VERIFY.md:
- Verdict (Approved / Approved with Follow-ups / Rejected)
- Findings (V-NNN) with Severity, Scope, Location, Problem, Why it matters, Expected
- Follow-ups (FU-NNN) if any

**VERIFY does not loop.** Single-pass gate.

### 5.2 Decide

- **Approved / Approved with Follow-ups** → report the verdict to the user. If follow-ups exist, list them; they can create new tasks. Tell the user: "Stage your work with `git add <files>`, then run `/ark:commit -m \"<message>\"` to close out the task." Do NOT commit automatically — the staged-work precondition belongs to the user.
- **Rejected** → halt. Summarize findings to the user and ask how to proceed (create fix tasks, promote tier via `ark agent task promote`, accept with acknowledgement, discard).

Closure is a separate user-invoked step. See `/ark:commit`. Bulk archive (post-closure) is a manager-only operation via the top-level `ark archive` CLI; slash commands no longer archive.
