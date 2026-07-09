---
name: ark-record
description: Record a manual session entry into the developer's workspace journal. Use when the user wants to write down notes between tasks (explorations, observations, status updates).
---

# `ark-record <task description>`

Append a manual session entry (not tied to any task) to the developer's active journal under `.ark/workspace/<dev>/journal-N.md`. Use this for inter-task notes: explorations, investigations, observations.

Task-driven entries are written automatically by `ark-commit`; do not run `ark-record` for those.

## Preconditions

- `.ark/.developer` exists (run `ark init --developer <name>` to bootstrap).

## Steps

### Step 1: Pull record context `[AI]`

```bash
ark context --scope record --format json
```

Returns resolved identity, `active_journal_path`, session count, branch, `journal_max_lines`. Use `<task description>` as the title or prompt for one if absent.

### Step 2: Append the entry `[AI]`

Append a block to `active_journal_path` containing exactly three agent-authored sections:

```markdown
## Session N: <title>

### Summary

<one-line summary>

### Main Changes

| Area | Description |
|------|-------------|
| <area> | <description> |
```

Do **not** write `**Date**`, `**Slug**`, `**Branch**` — the CLI inserts those after your `## Session N:` heading. Show the user what you wrote.

### Step 3: Stamp the auto-fields `[AI]`

```bash
ark agent workspace record --manual
```

The CLI: resolves identity, locates the active `journal-N.md`, stamps `**Date**`, `**Slug**: -`, `**Branch**` after your last `## Session N:` heading, updates the personal Session History row, refreshes top-level Active Developers. Transactional with rollback.

## Failure Modes

| Code | Cause | Recovery |
|------|-------|----------|
| `MissingIdentity` | `.ark/.developer` not present | Run `ark init --developer <name>` |
| `EntryFileMalformed` | journal does not end with a `## Session N: <title>` heading | Step 2 must place the heading at the very end, no trailing content after the Main Changes table |
| `JournalDriftDetected` | concurrent appender wrote bytes after this transaction | Journal is left intact; investigate manually |

## See Also

- `workflow.md` §8 (state & multi-session)
- `ark-commit` — task-driven journal entries (do not use `ark-record` for those)