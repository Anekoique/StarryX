---
description: Extract a feature SPEC from an existing codebase. For brownfield projects adopting Ark mid-life — produces specs/features/<slug>/SPEC.md without faking a deep-tier task.
argument-hint: "<feature-name> [hint]"
---

# `/ark:spec-extract $ARGUMENTS`

Author a feature SPEC for an existing implementation, then register it in the features INDEX with a provenance CHANGELOG entry. Use when the project already ships the feature (an OS kernel with copy-on-write, a webapp with auth) and Ark needs a SPEC to reference, but no deep-tier task ever produced one.

Structural mutation (write SPEC, upsert INDEX) is owned by `ark agent spec import`. Discovery, confirmation, and synthesis are yours.

Parse `$ARGUMENTS`: first token is `<feature-name>` (slugified for the SPEC dir); remainder is an optional hint to scope the search.

## Preconditions

- `.ark/` is initialized.
- `.ark/specs/features/<slug>/SPEC.md` does not already exist. If it does, stop and tell the user to amend via a deep-tier task instead — extraction is an *initial* SPEC operation.
- Project is a git repo (extraction stamps the current HEAD short-SHA into the SPEC's CHANGELOG as provenance).

## Phase 1 — Discover `[AI]`

Slugify `<feature-name>` (lowercase, hyphen-separated, ASCII).

Sweep three sources in parallel:

1. **Code** — grep for symbol/file/comment matches across the source tree. Try snake / kebab / camel / Pascal variants of the feature name.
2. **Docs** — search `docs/`, top-level `README*`, `CHANGELOG*`, `*.md` for prose mentions.
3. **Git history** — `git log --grep="<term>"` for introducing/major commits; `git log -S<symbol>` for symbol introduction; pick 3–5 anchor commits.

Build a **candidate set**: ranked list of files, symbols, doc sections, and key commits. Do NOT synthesize a SPEC yet — discovery alone overweights "what's in the code" relative to "what the feature is."

## Phase 2 — Confirm `[AI]` `[USER]`

Present the candidate set to the user. Ask them to:

1. **Trim** false positives (a `find` matching by filename but unrelated implementation).
2. **Add** anything missed (key files / symbols / commits the sweep didn't surface).
3. **Supply a one-line intent** describing what the feature *is for* — the *why* / observable contract, not the implementation. This is the hardest thing to recover from code alone, and it's what differentiates a SPEC from reference docs.

Do NOT proceed without explicit user confirmation. The confirm gate is mandatory; brownfield extraction without it reliably produces SPECs that describe the codebase rather than the feature.

## Phase 3 — Synthesize `[AI]`

Read every confirmed source in full. Then author the SPEC body in the feature-SPEC template's shape:

- `[**Goals**]` — verb-led, ≤80 chars, the *what* (capability-oriented). Soft cap 5.
- `[**Non-goals**]` — only when a reader would assume it's in scope. Soft cap 3.
- `[**Architecture**]` — module / file layout with a one-line note per file. Tree or diagram, no prose narration.
- `[**Data Structure**]` — public types, fields + types + a one-line comment when meaning is non-obvious.
- `[**API Surface**]` — public function signatures + one-line semantics. No bodies.
- `[**Constraints**]` — one declarative sentence each, ≤120 chars. Cite the source of truth (constant / test / file path) when one exists.

**Discipline:**

- Do NOT fabricate non-goals or constraints. If evidence is thin, leave the section terse or omit it.
- Do NOT add a `[**CHANGELOG**]` section — `spec import` stamps the provenance entry.
- The SPEC describes *what was built*, not *how the extraction happened*. Process metadata belongs nowhere in the body.

Write the body to a tempfile (`.ark/.spec-extract-<slug>.md` or any path the slash command can clean up).

## Phase 4 — Import `[AI]`

```bash
SHA=$(git rev-parse --short HEAD)
ark agent spec import \
    --feature "<slug>" \
    --scope "<one-line intent from Phase 2>" \
    --from-file ".ark/.spec-extract-<slug>.md" \
    --from-commit "$SHA"
```

The CLI:

1. Validates `--feature` / `--scope` / `--from-commit` (no `|`, `\n`, `\r`; non-empty).
2. Refuses if `.ark/specs/features/<slug>/SPEC.md` already exists.
3. Reads the body, splices a `[**CHANGELOG**]` entry: `` - `YYYY-MM-DD` `extracted`: initial extraction from codebase at `<short-sha>`. ``
4. Writes `.ark/specs/features/<slug>/SPEC.md`.
5. Upserts a row in `.ark/specs/features/INDEX.md` with `from-task = "extracted"` (sentinel; same managed block as deep-tier promotion uses).
6. Prints a one-line summary.

Clean up the tempfile.

## Phase 5 — Hand off `[USER]`

Tell the user:

> Review `.ark/specs/features/<slug>/SPEC.md`. The extracted SPEC is *not* part of an Ark task — when satisfied, stage it (`git add .ark/specs/`) and commit it manually. Future deep-tier work that touches this feature will amend the SPEC through the normal flow.

Do NOT commit on the user's behalf — the SPEC is a deliberate artifact and the user owns the commit.

## Failure Modes

| Code | Cause | Recovery |
|------|-------|----------|
| `SpecAlreadyExists` | `.ark/specs/features/<slug>/SPEC.md` exists | Use a different slug, or amend via a deep-tier task; do not pass `--force`. |
| `InvalidSpecField` | `--feature` / `--scope` empty or contains `\|`, `\n`, `\r` | Sanitize the input and retry. |
| `Io` | `--from-file` missing | Re-author the body file; re-run import. |

## See Also

- `workflow.md` §6 (Specs) — the feature-spec layer extraction is feeding into.
- `/ark:design --deep` — the path for *new* features; produces a SPEC by promotion, not import.
- `ark agent spec import --help` — the CLI surface this slash command drives.
