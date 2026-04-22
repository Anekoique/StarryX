---
description: Record a manual session entry into the developer's workspace journal.
argument-hint: "[<title>]"
---

# `/ark:record $ARGUMENTS`

Append a manual session entry (not tied to any task) to the developer's
active journal under `.ark/workspace/<dev>/journal-N.md`. Use this for
notes between tasks: explorations, investigations, observations.

Task-driven entries are written automatically by `/ark:commit`; do not run
`/ark:record` for those.

## Preconditions

- `.ark/.developer` exists (run `ark init --developer <name>` to bootstrap).

## Steps

### 1. Pull record context

```bash
ark context --scope record --format json
```

Returns the resolved identity, active journal path, session count, branch,
and `journal_max_lines`. Use the title from `$ARGUMENTS` (or prompt for
one if absent).

### 2. Append the entry to the active journal

Append a block to the file at `active_journal_path` containing exactly
three agent-authored sections:

```markdown
## Session N: <title>

### Summary

<one-line summary>

### Main Changes

| Area | Description |
|------|-------------|
| <area> | <description> |
```

Do **not** write `**Date**`, `**Slug**`, or `**Branch**`. The CLI inserts
those auto-fields after your `## Session N: <title>` heading. Show the
user what you wrote.

### 3. Stamp the auto-fields

```bash
ark agent workspace record --manual
```

The CLI:
- Resolves identity from `.ark/.developer`.
- Locates the active `journal-N.md` (the highest-numbered file under the
  developer's directory).
- Stamps `**Date**: <today>`, `**Slug**: -`, `**Branch**: <current branch>`
  after your last `## Session N: <title>` heading.
- Updates the personal index's Session History row.
- Refreshes the top-level Active Developers row.
- Transactional: a failure mid-flight rolls back to the pre-stamp state.
  The one documented exception is concurrent-append drift: rollback
  detects bytes the transaction did not write and leaves the file
  untouched, surfacing `JournalDriftDetected`.

## Failure modes

- `MissingIdentity` — run `ark init --developer <name>`.
- `EntryFileMalformed` — the journal does not end with a `## Session N:
  <title>` heading. Check that step 2 wrote the heading at the very end of
  the file (no trailing content after the Main Changes table).
- `JournalDriftDetected` — concurrent appender wrote bytes after this
  transaction. The journal is left intact; investigate manually.
