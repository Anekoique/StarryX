---
description: Switch this session's focused task to an existing active slug. Idempotent.
argument-hint: "<slug>"
---

# `/ark:resume $ARGUMENTS`

Claim an existing active task as **this session's focused task**. After this, `--slug`-less commands like `/ark:commit` resolve to the resumed slug.

## Preconditions

- `.ark/` is initialized.
- The slug exists in `.ark/.state.toml`'s `tasks.active` (i.e. the task was created with `/ark:quick` or `/ark:design` and has not been archived or discarded).

## Steps

### 1. Resolve the slug

Parse `$ARGUMENTS`. The slug is required — there is no default for `/ark:resume`. If the user typed `/ark:resume` with no argument, ask them which active task to resume; show the list from `ark context --scope session --format json`.

### 2. Run the op

```bash
ark agent task resume --slug <slug>
```

This single command:
- Validates the slug.
- Refuses with `TaskNotFound` when the slug is not in `tasks.active`.
- Sets this session's `focus` to the slug in `.ark/.state.toml`.
- Idempotent: re-resuming the slug already focused by this session is a no-op.

### 3. Report to user

Confirm the new focus in one line. Mention any next step that depends on focus (e.g. "now `/ark:commit` will close out `<slug>`").

## Failure modes

- `TaskNotFound` → the slug is not active. Either it was archived/discarded already, or the user typed it wrong. Show the active set from `ark context` and ask.
- `InvalidTaskField` → the slug failed validation (path traversal, whitespace, non-ASCII). Reject with the validator's message.
