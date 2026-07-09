---
name: ark-commit
description: Close out an Ark task by atomically committing work + task.toml + (deep tier) SPEC in one git commit. Use when the user says they're done with a task and wants to land it. Replaces the older ark-archive skill; bulk archive is now a manager-only `ark archive` CLI.
---

# `ark-commit <task description>`

Close an Ark task by committing the user's staged work plus the Ark-managed closure artifacts (updated `task.toml`; on deep tier, the promoted feature SPEC + features INDEX) in **one** git commit. Bulk archive is a separate manager-only operation via the top-level `ark archive` CLI.

## Preconditions

- Task has reached its tier's pre-commit phase:
  - **Quick:** `phase = "execute"`
  - **Standard / Deep:** `phase = "verify"` (VERIFY.md filled)
- **User has staged work first.** `ark-commit` only stages Ark-managed artifacts; user code must already be in the index. Empty staging area → `NothingStaged` error. Pass `-a` to stage every tracked + untracked change (`git add -A`) first; incompatible with `--no-commit`.
- **Deep tier:** VERIFY.md has no `PENDING` items / unresolved Findings (gate refuses).
- **Standard tier:** pending VERIFY entries warn but do not block.

## Steps

### Step 1: Pull commit-phase context `[AI]`

```bash
ark context --scope phase --for commit --format json
```

Body-free projection (per `ark-context` SPEC). Returns paths to the latest VERIFY.md and `PLAN.md` plus git state. Read VERIFY.md from the returned path before composing the message.

### Step 2: Resolve the slug `[AI]`

Parse `<task description>`. If a bare slug is present, use it; otherwise default to this session's focused slug.

### Step 3: Compose the commit message `[AI]` `[USER]`

If `<task description>` includes `-m "<msg>"`, use it verbatim. Otherwise:

1. `git diff --cached` — see what the user staged.
2. `git log -n 5 --oneline` — mirror the project's commit-message style.
3. Generate a Conventional Commits subject (≤70 chars) + optional body for non-trivial changes.
4. Show the message to the user and ask for confirmation/edit. **Do not invent a message without asking.**

### Step 4: Append the journal entry (workspace) `[AI]`

If `.ark/.developer` exists, append a session block to the active journal at `active_journal_path` (from `ark context --scope record`):

```markdown
## Session N: <title>

### Summary

<one-line summary>

### Main Changes

| Area | Description |
|------|-------------|
| <area> | <description> |
```

The CLI inserts `**Date**`, `**Slug**`, `**Branch**`, `**Base Branch**`, `**Start Head**`, `**Closing Commit**`, and `### Git Commits` after your `## Session N:` heading. Do not write them.

**Style — keep it tight.**
- `### Summary`: one line, ≤120 chars. Lead with the user-visible effect. Drop "now" / "currently" / "going forward".
- `### Main Changes`: ≤4 rows, one line per cell, ≤80 chars. No nested code blocks. Drop incidental rows (`tests`, `template parity`, `doc updates`, downstream SPEC amendments) unless that *is* the change. Skip the row rather than pad it.

If `.ark/.developer` is absent, skip this step.

### Step 5: Run the commit `[AI]`

```bash
ark agent task commit -m "<message>"
# pass -a to stage every tracked + untracked change before committing:
ark agent task commit -a -m "<message>"
# or to skip the git commit entirely:
ark agent task commit --no-commit
```

`--no-commit` flips phase to `Committed` and (deep tier) extracts the SPEC, but skips the git commit. The user owns any follow-up commit.

The CLI does (in order): VERIFY gate → deep-tier SPEC extraction (`specs/features/<slug>/SPEC.md`) + features INDEX upsert → save `task.toml` (`phase = Committed`, `committed_at = now`) → stage exactly the Ark-managed files (no `git add -A`) → `git commit -m "<message>"`.

If `git commit` fails, a scoped rollback restores every snapshot (task.toml, deep-tier SPEC + features INDEX) and unstages only what Ark added — the user's pre-existing index entries survive.

### Step 6: Report `[AI]`

After success, summarize:
- The commit SHA (`summary.head_sha`).
- Deep tier only: the promoted SPEC path.
- A note that no Ark-managed file is dirty post-commit.

## Failure Modes

| Code | Cause | Recovery |
|------|-------|----------|
| `NothingStaged` | empty staging area | User runs `git add <files>` first |
| `VerifyIncomplete` (deep) | VERIFY.md has `PENDING` items / Findings | Resolve each, then re-run |
| `CommitMessageRequired` | invoked without `-m` and without `--no-commit` | Logic bug — Step 3 should have produced one |
| `GitCommitFailed` | pre-commit hook (or git) rejected | Surface stderr to user; rollback already happened; re-run after fixing the hook |
| `IllegalPhaseTransition` | task not in pre-commit phase | Tell the user current phase + expected |

## See Also

- `workflow.md` §4 (commit phase contract), §7 (CLI surfaces)
- `ark-design`, `ark-quick` — task creators that flow into `ark-commit`