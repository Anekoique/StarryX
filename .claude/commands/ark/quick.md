---
description: Start a quick-tier task. For trivial, reversible changes. Produces PRD.md only.
argument-hint: "<title>"
---

# `/ark:quick $ARGUMENTS`

Create a quick-tier task for a trivial, reversible change. No clarifying questions, no PLAN, and no separate VERIFY.md artifact.

Structural operations (task dir creation, phase transitions, archive moves) are handled by `ark agent` — do not hand-edit `task.toml` or move directories with `mv`.

## Preconditions

- `.ark/` is initialized.
- The change is reversible in one commit and introduces no new abstractions.
  If not, stop and suggest `/ark:design` (standard) or `/ark:design --deep` instead.

## Steps

### 1. Pull project context

```bash
ark context --scope phase --for design --format json
```

The output is the authoritative snapshot of `.ark/`, git, and project specs for the design phase. Read it before reading the workflow doc — it tells you what specs to consult, what tasks are active, and where you're starting from.

`.ark/workflow.md` is also worth a quick scan if you haven't read it recently:

```bash
cat .ark/workflow.md
```

### 2. Create the task

Turn the title into a slug: lowercase, hyphen-separated, ASCII, ≤40 chars.

```bash
ark agent task new --slug <slug> --title "<title>" --tier quick
```

This scaffolds `.ark/tasks/<slug>/` with `PRD.md` + `task.toml`, registers the slug in `.ark/.state.toml` as this session's focus, and warns to stderr if other active tasks already exist (use `ark agent task resume --slug <other>` to switch focus, or `ark agent task discard --slug <other>` to remove it). Refuses if the slug already exists.

Pass `--worktree` to scaffold inside a git worktree at `.ark/worktrees/<branch>/` instead — useful when the new task would collide with in-flight changes on the active branch. Standard / deep tiers may also opt in. **When `--worktree` is used, `cd .ark/worktrees/<branch>/` before editing the PRD or running any subsequent `ark agent task ...` commands; they operate on the worktree's own `.ark/` and the parent checkout is untouched.**

### 3. Fill the PRD

Edit `.ark/tasks/<slug>/PRD.md`:
- **What** — one-line description
- **Why** — the reason
- **Outcome** — observable success criteria (doubles as verification checklist for quick tier)
- **Related Specs** — any `specs/features/<name>/SPEC.md` this change touches (or leave blank)

### 4. Advance to execute

```bash
ark agent task execute
```

### 5. Implement the change

Follow the PRD's Outcome. Stay within scope — if work grows beyond trivial, stop and suggest promoting to standard.

### 6. Verify against PRD's Outcome

Run whatever check the Outcome describes (test, build, manual). Record the result by updating PRD's Outcome section with what you verified.

### 7. Stage your work

The user runs `git add <files>` to stage their work. Quick tier has no
VERIFY checklist; the PRD's Outcome already serves as the acceptance gate.

### 8. Close the task

Tell the user to run `/ark:commit -m "<message>"`. That single command flips
phase to `Committed`, stages the Ark-managed files (`task.toml`), and runs
`git commit -m "<message>"`. The user's already-staged work lands in the
same commit.

If the user prefers to write the commit message themselves, they can pass
`-m`; otherwise the agent generates a Conventional Commits message from the
staged diff and shows it for confirmation. See `/ark:commit` for the full
contract (rollback on hook failure, `--no-commit` opt-out).

## If the task grows mid-flight

Stop. Tell the user: "This change is larger than quick-tier scope. Recommend promoting to standard (`/ark:design`) — I'll preserve the PRD as historical context." Wait for user decision.

To promote mid-flight:

```bash
ark agent task promote --to standard
```

Then continue from Phase 2 of `/ark:design` (write PLAN, etc.). Existing artifacts are preserved — the agent decides what to reshape.
