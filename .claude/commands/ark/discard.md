---
description: Remove an unarchived Ark task. Refuses without --force when seeded files have user content.
argument-hint: "[<slug>] [--force]"
---

# `/ark:discard $ARGUMENTS`

Remove an unarchived Ark task in one step: drop the slug from the active set, clear any session focus pointing at it, then delete the task directory.

This is the **discard** path, not the **archive** path. Discard is for tasks the user no longer wants on disk at all (mistyped slug, abandoned exploration, scaffolding the user wants to throw away). Archive is for tasks that completed their lifecycle and should be preserved as memory.

## Preconditions

- `.ark/` is initialized.
- The task is **not** archived. Already-archived tasks live under `tasks/archive/YYYY-MM/<slug>/` and are not removed by this command.

## Steps

### 1. Resolve the slug

Parse `$ARGUMENTS`:
- If a slug is given, use it.
- Otherwise, use this session's focused slug from `.ark/.state.toml` (the CLI defaults to it automatically).
- If the user passed `--force`, plumb it through.

### 2. Decide whether to require `--force`

By default, `/ark:discard` refuses when any seeded artifact (`PRD.md`, `NN_PLAN.md`, `NN_REVIEW.md`, `VERIFY.md`) differs from its embedded template — the "PRD has user content" guard. The user must pass `--force` to override.

If you (the agent) are about to run `/ark:discard` on the user's behalf without their explicit go-ahead, **never** pass `--force`. Surface the `TaskStillActive` error to the user and let them decide.

### 3. Run the op

```bash
ark agent task discard            # uses this session's focus from .ark/.state.toml
# or
ark agent task discard --slug <slug>
# or, when the user has authorized data loss:
ark agent task discard --slug <slug> --force
```

This single command:
- Validates the slug.
- Refuses with `TaskNotFound` when the task dir is missing or its `phase == Archived`.
- Refuses with `TaskStillActive { file }` when seeded files diverge from templates and `--force` is not set.
- Otherwise: removes the slug from `.ark/.state.toml`'s `tasks.active`, clears any session focus pointing at it, deletes the cache file when this session's focus matched, and `rm -rf`s `.ark/tasks/<slug>/`.

### 4. Report to user

In one line, confirm the discard and the deleted directory path. If the discarded slug was this session's focus, mention that this session no longer has a focused task.

## Failure modes

- `TaskNotFound` → no `.ark/tasks/<slug>/`, or the task is already archived. Suggest `task list` if the user is unsure what's active.
- `TaskStillActive { file }` → a seeded file diverges from its template. Show the file name and ask the user whether to re-run with `--force`.
- `InvalidTaskField` → the slug failed validation. Reject with the validator's message.
