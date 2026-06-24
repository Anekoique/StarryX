---
description: Switch this session's focused task to an existing active slug. Idempotent.
argument-hint: "<slug>"
---

# `/ark:resume $ARGUMENTS`

Claim an existing active task as **this session's focused task**. After this, `--slug`-less commands like `/ark:commit` resolve to the resumed slug.

## Preconditions

- `.ark/` is initialized.
- The slug exists in `.ark/.state.toml`'s `tasks.active` (created via `/ark:quick` or `/ark:design`, not yet archived/discarded).

## Steps

### Step 1: Resolve the slug `[AI]` `[USER]`

Slug is required — no default. If `$ARGUMENTS` is empty, ask the user which active task to resume. List active tasks via `ark context --scope session --format json`.

### Step 2: Run the op `[AI]`

```bash
ark agent task resume --slug <slug>
```

Validates the slug; sets this session's `focus` in `.ark/.state.toml`. Idempotent — re-resuming the slug already focused by this session is a no-op.

### Step 3: Report `[AI]`

Confirm the new focus in one line. Mention any next step that depends on focus (e.g. *"now `/ark:commit` will close out `<slug>`"*).

## Failure Modes

| Code | Cause | Recovery |
|------|-------|----------|
| `TaskNotFound` | slug not in `tasks.active` (archived/discarded or typo) | Show active set via `ark context`; ask the user |
| `InvalidTaskField` | slug failed validation (path traversal, whitespace, non-ASCII) | Reject with the validator's message |

## See Also

- `workflow.md` §8 (state & multi-session)
- `/ark:discard` — remove an unarchived task
