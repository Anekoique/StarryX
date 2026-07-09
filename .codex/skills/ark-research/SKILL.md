---
name: ark-research
description: Start a research-tier Ark task. Gather a reference corpus on a topic; follow-up implementation is optional. Use when the user wants to investigate a direction before committing to a design.
---

# `ark-research <topic>`

Create a research-tier task: the deliverable is a curated reference corpus under `.ark/tasks/<slug>/research/`. No PLAN, no VERIFY, no SPEC promotion. Follow-up implementation is optional — close the task when the corpus is sufficient.

Structural ops (task dir, phase transitions, archive moves) are owned by `ark agent`. Do not hand-edit `task.toml` or move directories.

## Preconditions

- `.ark/` is initialized.
- The goal is to *learn*, not to ship code. If you can already write a PRD's What/Why/Outcome, stop and use `ark-quick` or `ark-design` instead — the embedded `ark-researcher` subagent dispatched during DESIGN/PLAN covers knowledge-gathering in service of a known deliverable.

## Steps

### Step 1: Pull design-phase context `[AI]`

```bash
ark context --scope phase --for design --format json
```

Returns the snapshot of git, active tasks, and project + feature specs.

### Step 2: Create the task `[AI]`

Slugify the topic (lowercase, hyphen-separated, ASCII, ≤40 chars).

```bash
ark agent task new --slug <slug> --title "<topic>" --tier research
# add --worktree if research will run in parallel with in-flight code work;
# then `cd .ark/worktrees/<branch>/` for all subsequent steps
```

Scaffolds `.ark/tasks/<slug>/{PRD.md, task.toml}`, registers the slug, sets this session's focus. Refuses if the slug already exists.

### Step 3: Fill the PRD `[AI]` `[USER]`

Edit `.ark/tasks/<slug>/PRD.md`. On research tier the fields map as:

- **What** — the question / direction being investigated.
- **Why** — motivation. Not required to lead to implementation.
- **Outcome** — "Why this corpus is the right next step" (optional; the corpus itself is the deliverable).
- **Related Specs** — feature SPECs consulted (optional).
- **SPEC Path** — ignored on research tier.

### Step 4: Iteratively dispatch `ark-researcher` `[AI]` `[USER]`

For each sub-topic, dispatch the `ark-researcher` subagent. Each call writes one `.ark/tasks/<slug>/research/<topic>.md`. Repeat until the corpus is sufficient. The PRD's Scope is the bound; stop when the question is answered or when "more searching wouldn't change the conclusion".

### Step 5: Stage and close `[USER]` then `[AI]`

User runs `git add .ark/tasks/<slug>/`. Then invoke `ark-commit -m "<message>"`. See `ark-commit` for the contract.

## If the corpus turns into implementation

Research tasks do NOT promote to implementation tiers (`task promote` rejects research↔tiered cross-over). When the corpus is ready and you want to build:

1. Close the research task as above.
2. Start a fresh `ark-quick` or `ark-design` task whose PRD references the research slug in prose (e.g. "based on the corpus at `tasks/archive/.../<slug>/research/`").

## See Also

- `workflow.md` §3 (tiers), §Research, §4 (phase contracts), §5 (lifecycle)
- `ark-commit` — closure contract
