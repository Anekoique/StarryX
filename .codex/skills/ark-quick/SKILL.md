---
name: ark-quick
description: Start a quick-tier Ark task. For trivial, reversible changes. Produces PRD.md only. Use when the user asks for a small fix, typo, or one-line change that's reversible in a single commit.
---

# `ark-quick <task description>`

Create a quick-tier task: trivial, reversible change in a single commit. No clarifying questions, no PLAN, no separate VERIFY. The PRD's `[**Outcome**]` is the acceptance gate.

Structural ops (task dir, phase transitions, archive moves) are owned by `ark agent`. Do not hand-edit `task.toml` or move directories.

## Preconditions

- `.ark/` is initialized.
- The change is reversible in one commit and introduces no new abstractions. If not, stop and suggest `ark-design` (standard) or `ark-design --deep`.

## Steps

### Step 1: Pull design-phase context `[AI]`

```bash
ark context --scope phase --for design --format json
```

Returns the snapshot of git, active tasks, and project + feature specs. See `workflow.md` §4 for the projection contract.

### Step 2: Create the task `[AI]`

Slugify the title (lowercase, hyphen-separated, ASCII, ≤40 chars).

```bash
ark agent task new --slug <slug> --title "<title>" --tier quick
# add --worktree if work would collide with in-flight changes on the active branch;
# then `cd .ark/worktrees/<branch>/` for all subsequent steps
```

Scaffolds `.ark/tasks/<slug>/{PRD.md, task.toml}`, registers the slug, sets this session's focus. Refuses if the slug already exists.

### Step 3: Fill the PRD `[AI]` `[USER]`

Edit `.ark/tasks/<slug>/PRD.md` per the template: **What**, **Why**, **Outcome**, **Related Specs** (or leave empty).

### Step 4: Advance to execute `[AI]`

```bash
ark agent task execute
```

### Step 5: Implement and self-verify `[AI]`

Implement to satisfy the PRD's Outcome. Run whatever check the Outcome describes (test, build, manual). If work grows beyond trivial, stop and propose promotion to standard.

### Step 6: Stage and close `[USER]` then `[AI]`

User runs `git add <files>`. Then invoke `ark-commit -m "<message>"`. See `ark-commit` for the contract.

## If the task grows mid-flight

Stop. Tell the user: *"This change is larger than quick-tier scope. Recommend promoting to standard (`ark-design`) — I'll preserve the PRD."* Wait for the user's decision.

```bash
ark agent task promote --to standard
```

Then continue from Phase 2 of `ark-design` (write PLAN, etc.). Existing artifacts are preserved.

## See Also

- `workflow.md` §3 (tiers), §4 (phase contracts), §5 (lifecycle)
- `ark-commit` — closure contract