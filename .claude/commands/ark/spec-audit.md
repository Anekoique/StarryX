---
description: Audit every SPEC's constraints for actuator-tag health. Reports untagged rules, malformed tags, and tag/bucket mismatches across project and feature SPECs, then offers to fix them yourself or with agent assistance.
allowed-tools: Read, Edit, Glob, Grep, Bash
---

# `/ark:spec-audit`

Audit the health of every SPEC's constraints — both project conventions under `.ark/specs/project/` and feature SPECs under `.ark/specs/features/` — and surface the problems that let a constraint silently go un-enforced.

Each rule's first line is its actuator tag — `- C-N: @<kind>[: <arg>]`, with the rule on the next line — saying how it is enforced: `tool` (lint/test command), `source-scan` (`<pattern> @ <glob>`), `test-binding` (a test id), or `judgment`. `judgment` is an honest choice, not a defect; this audit reports missing or malformed tags so you can fix them.

## Step 1 — Gather

- Read every SPEC under `.ark/specs/project/` and every `SPEC.md` under `.ark/specs/features/`.
- For each `[**Rules**]` (project) or `[**Constraints**]` (feature) bullet, read its inline actuator tag if present.

## Step 2 — Find problems

Classify each rule:

- **Missing** — the rule's first line is not `- C-N: @<kind>[: <arg>]`. Its enforcement status is unknown; it rests on nothing.
- **Malformed** — a tag that does not parse (unknown kind, missing arg, `test-binding` naming a `V-*` label instead of a test id).
- **Mismatch** — a tag whose kind contradicts the rule's prose (e.g. a clearly tool-enforceable rule tagged `judgment`, or a taste rule tagged `source-scan`).
- **Healthy** — a well-formed tag whose kind fits the rule.

Report a per-file summary: counts by kind, the list of untagged/malformed/mismatched rule ids, and for each fixable item a **proposed** tag.

## Step 3 — Offer the fix

**STOP and ask the user how to apply the proposed fixes:**

- **Fix yourself** — print the proposed tags; the user edits the SPEC files by hand. (Honors the convention that project SPECs are user-owned.)
- **Agent-assisted** — you write the proposed tags into the SPEC files as a reviewable diff the user approves before commit.

Do not edit any SPEC file until the user chooses agent-assisted mode for this run. Default to read-only.

## Notes

- Never write a `V-*` / `C-N` / `G-N` workflow label into a `test-binding` arg or any source; the arg names a concrete test id or command.
- `judgment` is a first-class, honest choice for rules that genuinely cannot be mechanized — flag it, count it, do not force a fake enforcer onto it.
