---
name: ark-discard
description: Remove an unarchived Ark task in one step. Refuses without --force when seeded files have user content. Use when the user wants to throw away a task they no longer want.
---

# `ark-discard <task description>`

Remove an unarchived Ark task: drop the slug from the active set, clear any session focus pointing at it, delete the task directory. **Discard ≠ archive.** Discard throws away tasks the user no longer wants on disk (mistyped slug, abandoned exploration). Archive preserves completed tasks as memory.

## Preconditions

- `.ark/` is initialized.
- The task is **not** archived. Already-archived tasks live under `tasks/archive/YYYY-MM/<tier>/<slug>/` and are not touched by this command.

## Steps

### Step 1: Resolve the slug `[AI]`

Parse `<task description>`. If a slug is present, use it; otherwise default to this session's focused slug. Plumb `--force` through if the user passed it.

### Step 2: Decide on `--force` `[AI]`

By default, `discard` refuses when any seeded artifact (`PRD.md`, `PLAN.md`, `REVIEW.md`, `VERIFY.md`) diverges from its template — the "PRD has user content" guard.

**If you (the agent) are about to run `ark-discard` on the user's behalf without their explicit go-ahead, never pass `--force`.** Surface the error and let the user decide.

### Step 3: Run the op `[AI]`

```bash
ark agent task discard --slug <slug>
# or, when the user has authorized data loss:
ark agent task discard --slug <slug> --force
```

Removes the slug from `tasks.active`, clears session focus pointing at it, and `rm -rf`s `.ark/tasks/<slug>/`.

### Step 4: Report `[AI]`

In one line: confirm the discard and the deleted directory path. If the discarded slug was this session's focus, mention that this session no longer has a focused task.

## Failure Modes

| Code | Cause | Recovery |
|------|-------|----------|
| `TaskNotFound` | no `.ark/tasks/<slug>/`, or task already archived | Suggest `task list` if the user is unsure what's active |
| `TaskStillActive { file }` | seeded file diverges from template; `--force` not set | Show the file name; ask the user whether to re-run with `--force` |
| `InvalidTaskField` | slug failed validation | Reject with the validator's message |

## See Also

- `workflow.md` §8 (state & multi-session)
- `ark-resume` — switch focus instead of discarding