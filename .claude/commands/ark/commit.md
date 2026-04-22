---
description: Close out the current (or a named) Ark task. Atomically commits work + task.toml + (deep) SPEC in one git commit.
argument-hint: "[-m <message>] [--no-commit] [<slug>]"
---

# `/ark:commit $ARGUMENTS`

Close an Ark task by committing the user's staged work plus the Ark-managed
closure artifacts (updated `task.toml`, and on deep tier the promoted feature
SPEC + features INDEX) in **one** git commit. Replaces the older
`/ark:archive`; bulk archive is now a manager-only operation via the
top-level `ark archive` CLI.

## Preconditions

- The task has reached its tier's pre-commit phase:
  - Quick: `phase = "execute"`
  - Standard / Deep: `phase = "verify"` (VERIFY.md filled in)
- **The user has staged their work first.** `/ark:commit` only stages
  Ark-managed closure artifacts; user code/edits must already be in the
  index via `git add <files>`. If the staging area is empty, `task_commit`
  errors with `NothingStaged`.
- Deep tier: VERIFY.md has no `PENDING` checklist items or unresolved
  findings (the gate refuses; resolve each before invoking).
- Standard tier: any pending VERIFY entries warn but do not block.

## Steps

### 1. Pull commit-phase context

```bash
ark context --scope phase --for commit --format json
```

Returns paths to the latest VERIFY.md and the latest `NN_PLAN.md`, plus git
state. Read VERIFY.md from the returned path before composing the commit
message — flagged FAIL items or open Findings need acknowledgement, and the
staged diff plus the recent `git log` style are the inputs to the message
generator.

### 2. Resolve the slug

Parse `$ARGUMENTS`:
- If a bare slug is supplied, use it.
- Otherwise, default to this session's focused slug from `.ark/.state.toml`.

### 3. Compose the commit message

If `$ARGUMENTS` includes `-m "<msg>"`, use it verbatim. Otherwise:

1. Run `git diff --cached` to see what the user staged.
2. Run `git log -n 5 --oneline` to mirror the project's commit-message style.
3. Generate a Conventional Commits message that summarizes the staged diff
   in one short subject line (≤ 70 chars), with optional body lines for
   non-trivial changes.
4. Show the generated message to the user and ask for confirmation/edit
   before invoking the CLI. **Do not invent a message without asking.**

### 4. Append the journal entry (workspace)

If `.ark/.developer` exists, append a session block **directly to the
active journal file** (its path is in the `active_journal_path` field of
`ark context --scope record`). The block must include exactly three
agent-authored sections, in this order:

```markdown
## Session N: <title>

### Summary

<one-line summary>

### Main Changes

| Area | Description |
|------|-------------|
| <area> | <description> |
```

Do **not** write `**Date**`, `**Slug**`, `**Branch**`, `**Base Branch**`,
`**Start Head**`, `**Closing Commit**`, or `### Git Commits`. The CLI
inserts those auto-fields after your `## Session N: <title>` heading
during `task commit`. Show the user what you wrote and let them revise
before continuing.

If `.ark/.developer` is absent, skip this step. The journal write is
silently skipped on installs without identity.

### 5. Run the commit

```bash
ark agent task commit -m "<message>"
# or to skip the git commit entirely:
ark agent task commit --no-commit
```

`--no-commit` skips the git commit but still flips phase to `Committed`
and (deep tier) extracts the SPEC. The user is responsible for any
follow-up commit.

The CLI does:
- VERIFY gate check (deep refuses on `PENDING`; standard warns).
- Deep tier: SPEC extraction (`specs/features/<slug>/SPEC.md`) + features
  INDEX upsert.
- Save updated `task.toml` (phase = `Committed`, `committed_at = now`).
- Stage exactly the Ark-managed files (task.toml, plus on deep tier the
  SPEC + features INDEX) — **not** `git add -A`.
- Run `git commit -m "<message>"`.

If `git commit` fails (pre-commit hook rejects, etc.), `task_commit` rolls
back every snapshot it took: `task.toml` restored, deep-tier SPEC files
restored, and the targeted `git reset HEAD <ark-files>` unstages only what
ark added. The user's pre-existing index entries are preserved.

### 5. Report to user

After success, summarize:
- The commit SHA (`summary.head_sha`).
- Deep tier only: the promoted SPEC path.
- A note that **no Ark-managed file is dirty** post-commit. (The user's
  pre-existing unstaged files outside Ark's purview were intentionally
  not touched, by design.)

## Failure modes

- `NothingStaged` — staging area is empty. Tell the user to `git add <files>`
  first.
- `VerifyIncomplete` (deep tier) — VERIFY.md has `PENDING` items or
  findings. Tell the user which counts and to resolve each.
- `CommitMessageRequired` — the CLI was invoked without `-m` and without
  `--no-commit`. The slash command should have generated and confirmed the
  message before reaching this; treat as a logic bug.
- `GitCommitFailed` — the pre-commit hook (or git itself) rejected the
  commit. Surface the original `stderr` to the user; rollback already
  happened. Re-run after fixing the hook.
- `IllegalPhaseTransition` — the task is not in its pre-commit phase. Tell
  the user which phase it's in and what's expected.
