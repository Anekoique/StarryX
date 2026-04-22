# Ark Workflow

How work flows from intent to archive. Read before starting any task.

---

## 1. Principles

1. **Right ceremony for the right task.** Three tiers. Pick the smallest that fits.
2. **Intent before edits.** Write down what the change is before touching code.
3. **Review is a gate, not a ritual.** Verdicts block progress; do not fabricate compliance.
4. **Archive is memory.** Every completed task leaves a traceable record.

---

## 2. Layout

```
.ark/
├── workflow.md
├── templates/             # read-only source templates
│   ├── PRD.md
│   ├── PLAN.md
│   ├── REVIEW.md
│   ├── VERIFY.md
│   └── SPEC.md
├── tasks/
│   ├── <slug>/            # active task
│   │   ├── task.toml      #   phase, tier, dates
│   │   ├── PRD.md         #   all tiers — design-phase artifact
│   │   ├── NN_PLAN.md     #   standard (NN=00) / deep (iterated)
│   │   ├── NN_REVIEW.md   #   deep only — pairs with NN_PLAN
│   │   └── VERIFY.md      #   standard + deep
│   └── archive/YYYY-MM/<slug>/
└── specs/
    ├── project/<name>/SPEC.md     # user-authored conventions
    └── features/<name>/SPEC.md    # promoted on archive (deep)
```

---

## 3. Tiers

| Tier     | Claude command       | Codex skill   | OpenCode command     | Artifacts                                                               | Path through states                                              |
| -------- | -------------------- | ------------- | -------------------- | ----------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Quick    | `/ark:quick`         | `ark-quick`   | `/ark:quick`         | `PRD.md`                                                                | design → execute → committed → archived                          |
| Standard | `/ark:design`        | `ark-design`  | `/ark:design`        | `PRD.md`, `PLAN.md`, `VERIFY.md`                                        | design → plan → execute → verify → committed → archived          |
| Deep     | `/ark:design --deep` | `ark-design`  | `/ark:design --deep` | `PRD.md`, `NN_PLAN.md`, `NN_REVIEW.md`, `VERIFY.md`, promoted `SPEC.md` | design → plan ⇄ review → execute → verify → committed → archived |

PRD captures *what we're building and why*. PLAN elaborates *how*. VERIFY is a living checklist + findings document the implementer maintains during EXECUTE → COMMIT (no verdict, no loop; completion = no `PENDING` items or findings). `/ark:commit` atomically lands work + task.toml + (deep) SPEC + features INDEX in one git commit. Archive is a manager-only bulk operation via the top-level `ark archive` CLI; slash commands no longer archive.

```
quick:    reversible + no new abstractions
deep:     breaking / cross-cutting / new subsystem
standard: everything else
```

Promote mid-flight with `ark agent task promote --to <tier>`; prior artifacts are preserved.

To run multiple tasks in parallel without `.current` collisions, pass `--worktree` at scaffold time: `ark agent task new --slug foo --tier <t> --worktree` creates branch `feat/foo` (override with `--branch-type fix|refactor|...` or `--branch <full>`) at `.ark/worktrees/<branch>/` and scaffolds the task dir inside it. The parent checkout's `.current` is untouched. Configure copies and post-create hooks via `.ark/config.toml`'s `[worktree]` section (user-editable; preserved across `ark upgrade`).

---

## 4. Lifecycle

```
       ┌────────────┐
       │  /ark:*    │  slash command starts a task
       └─────┬──────┘
             ▼
       ┌────────────┐
       │  DESIGN    │  write PRD.md — What / Why / Outcome
       └─────┬──────┘
             │  (quick skips plan/review/verify)
             ▼
       ┌────────────┐
       │    PLAN    │  write NN_PLAN.md — elaborate how
       └─────┬──────┘
             │         (deep only — plan review loop)
             │         ┌──────────────┐
             ├────────►│    REVIEW    │  NN_REVIEW.md
             │         └──────┬───────┘
             │ ◄─── rejected ─┘
             ▼
       ┌────────────┐
       │  EXECUTE   │  implement; update PLAN's Spec section if gaps emerge
       └─────┬──────┘
             ▼
       ┌────────────┐
       │   VERIFY   │  living checklist + findings; complete when
       └─────┬──────┘  no `PENDING` items or findings remain
             ▼
       ┌────────────┐
       │   COMMIT   │  atomic: VERIFY gate, deep-tier SPEC extract,
       └─────┬──────┘  one git commit covering work + task.toml +
             │        (deep) SPEC + features INDEX
             │
             ▼  (later, manager-invoked)
       ┌────────────┐
       │  ARCHIVE   │  `ark archive` (top-level CLI, manager-only)
       └────────────┘  bulk-moves committed tasks to
                       tasks/archive/YYYY-MM/, side-effect-free
```

Each stage below names its **purpose**, the **calls** to make, and the **gate** to advance.

### DESIGN — capture what & why

- **Purpose:** write `PRD.md` (What / Why / Outcome / Related Specs). Brainstorm: quick = none, standard = ≤3 clarifying questions, deep = thorough.
- **Calls:**
  - `ark context --scope phase --for design` — orient on git, project specs, feature specs index, recent archive.
  - `ark agent task new --slug <s> --title "<t>" --tier {quick|standard|deep} [--worktree]` — scaffolds the task dir + PRD + `task.toml`. Pass `--worktree` to bind the task to a fresh git worktree at `.ark/worktrees/<branch>/` (see §3 and §6).
- **Gate:** PRD drafted, Outcome stated. Quick → EXECUTE; standard/deep → PLAN.

### PLAN — elaborate how

- **Purpose:** fill `NN_PLAN.md` from the embedded template (Spec, Runtime, Implementation, Trade-offs, Validation). Every Goal mapped to ≥1 Validation.
- **Calls:**
  - `ark context --scope phase --for plan` — pulls current PRD + related feature specs (filtered to the PRD's `[**Related Specs**]`) + project specs.
  - `ark agent task plan` — transitions DESIGN → PLAN and seeds `00_PLAN.md`.
- **Gate:** PLAN complete; Acceptance Mapping fills every Goal. Standard → EXECUTE; deep → REVIEW.
- **Rule:** `## Spec` must be self-contained every iteration (deltas go in `## Log`). It is copied verbatim to `specs/features/<name>/SPEC.md` on archive.

### REVIEW — pre-execute gate (deep only, iterative)

- **Purpose:** evaluate the latest `NN_PLAN.md` against PRD and project specs; write `NN_REVIEW.md` with verdict + findings. Loop until verdict = *Approved* with zero open CRITICAL.
- **Calls:**
  - `ark context --scope phase --for review` — pulls current task, latest PLAN, related feature specs, project specs.
  - `ark agent task review` — transitions PLAN → REVIEW and seeds `NN_REVIEW.md`.
- **Reject (HIGH)** if the latest PLAN's `## Spec` references prior iterations instead of restating in full.
- **Iteration:** copy `NN_PLAN.md`/`NN_REVIEW.md` to the next number, bump `task.toml.iteration`, reset `phase = "plan"` (hand-edited; the state machine is small).
- **Gate:** verdict *Approved*, zero open CRITICAL. → EXECUTE.

### EXECUTE — implement

- **Purpose:** work through the latest PLAN's Implementation phases. If implementation reveals design gaps, **update the latest PLAN's `## Spec` section** to reflect reality.
- **Calls:**
  - `ark context --scope phase --for execute` — git dirty files + current task + latest PLAN + project specs.
  - `ark agent task execute` — transitions to EXECUTE.
- **Gate:** implementation complete; project's checks pass.
- **Worktree note:** if the task was created with `--worktree`, all phase commands (plan/review/execute/verify/commit) operate on the *worktree's* `.ark/`. `cd .ark/worktrees/<branch>/` and run them there. After merging the branch, run `ark agent task worktree cleanup --slug <s> [--delete-branch]` from the parent to remove the dir; archive does NOT auto-clean.

### VERIFY — living checklist + findings

- **Purpose:** maintain `VERIFY.md` as the implementer audits the shipped code against project specs, related feature specs, PRD constraints, plan goals, and SPEC drift. Each section's items resolve to PASS / FAIL / N/A; findings (V-NNN) capture cross-cutting observations with a Resolution. **No verdict line.**
- **Calls:**
  - `ark context --scope phase --for verify` — current task with PRD + latest PLAN + VERIFY.md path + git state.
  - `ark agent task verify` — transitions to VERIFY and seeds `VERIFY.md` with sections auto-populated from project SPEC INDEX, the PRD's Related Specs and Outcome, and the latest PLAN's Goals.
- **Gate:** every checklist item has a non-`PENDING` state and every finding's Resolution is non-`PENDING`. Then run `/ark:commit`. Deep tier refuses commit when any `PENDING` remains; standard warns; quick has no VERIFY.

### COMMIT — atomic closure

- **Purpose:** land the user's staged work alongside the Ark-managed closure artifacts (updated `task.toml`, deep-tier promoted SPEC + features INDEX) in **one** git commit. Replaces the older slash-command archive step; bulk archive (post-closure) is a separate manager-only operation.
- **Preconditions:** user has staged work (`git add <files>`); deep-tier VERIFY has no `PENDING`.
- **Calls:**
  - `ark context --scope phase --for commit` — paths to VERIFY.md and the latest plan, plus git state. **Body-free** by design — slash commands read the artifact files from the returned paths.
  - `ark agent task commit -m "<message>"` (hidden CLI; user-facing slash command is `/ark:commit`). Performs: VERIFY gate; deep-tier SPEC extract; save `task.toml` (`phase = Committed`, `committed_at = now`); explicit per-file `git add` of only Ark-managed artifacts; `git commit -m "<message>"`. On any failure, a scoped `RollbackGuard` restores every snapshot it took (task.toml, deep-tier SPEC + features INDEX) and unstages only what Ark added — the user's pre-existing index entries survive.
- **`--no-commit`:** skips the git commit but still flips phase to `Committed` and (deep tier) extracts SPEC. The user owns any follow-up commit.

### ARCHIVE — manager-only bulk operation

- **Purpose:** sweep every `phase = Committed` task into `tasks/archive/YYYY-MM/<slug>/` using each task's own `committed_at` for the month bucket. Side-effect-free: no SPEC promotion — that already happened at commit time.
- **Calls:**
  - `ark archive [--month YYYY-MM] [--dry-run]` — top-level CLI, visible in `ark --help`. Default: archive every committed task. `--month` filters to one bucket. `--dry-run` lists candidates without moving.
  - `ark agent task archive --slug <s> [--month YYYY-MM]` — hidden internal helper for one-off archive moves; defaults `--month` to the task's own `committed_at`.
- **Reopen:** move the archived dir back to `.ark/tasks/<slug>/` and hand-edit `task.toml` to `phase = "verify"` + clear `archived_at`. Refuse if a same-slug active task exists.


---

## 5. Specs

Two layers: `specs/project/<name>/SPEC.md` (user-authored conventions) and `specs/features/<name>/SPEC.md` (extracted from deep-tier PLANs on archive).

**Read pattern.**
- **Project specs** — read every SPEC listed in `specs/project/INDEX.md` before any task. These are conventions that apply always.
- **Feature specs** — scan `specs/features/INDEX.md`, then read only the SPECs the task touches. Record them in PRD's `[**Related Specs**]` so VERIFY can check adherence. The DESIGN/PLAN/REVIEW context calls above expose both indices in their JSON output.

**SPEC promotion (deep tier).** `/ark:commit` extracts the final PLAN's Spec section to `specs/features/<name>/SPEC.md` and appends a row to the features INDEX. The new SPEC + INDEX rows land in the closing commit alongside the work. If the task modifies an existing feature SPEC, a `[**CHANGELOG**]` entry is appended to that SPEC. (Bulk `ark archive` does **not** touch SPEC files.)

**Divergence.** If a PLAN contradicts an existing feature SPEC, REVIEW flags it. Either the PLAN conforms or explicitly updates the SPEC.

---

## 6. Mechanics

Three CLI surfaces drive the workflow; all are referenced inline above.

- **`ark context`** — top-level, semver-stable, **read-only**. Reports git + active tasks + specs + recent archive + current task. Auto-invoked at session start via the `SessionStart` hook in `.claude/settings.json`. Use `--scope session` (default) for orientation; `--scope phase --for <phase>` for phase-targeted slices (`design | plan | review | execute | verify | commit`). `--format json` for machine consumers; default text for humans. The `commit` projection is body-free per the `ark-context` SPEC's additive-only schema; slash commands read VERIFY.md and the latest plan from the artifact paths the projection carries on `current_task`.
- **`ark archive`** — top-level, **manager-only**, visible in `ark --help`. Bulk-moves every `phase = Committed` task to `tasks/archive/YYYY-MM/<slug>/`, deriving the month from each task's own `committed_at`. Side-effect-free: no SPEC promotion. Run after a release cut or whenever you want to consolidate completed work.
- **`ark agent`** — hidden, **not semver-stable**, structural mutation only. Each subcommand prints a one-line summary; illegal transitions error out (e.g. `IllegalPhaseTransition`, `WrongTier`) — never bypass them with hand-edits. Every `--slug`-taking command defaults to *this session's focused task* in `.ark/.state.toml`. `ark agent task commit` is the structural mutation invoked by `/ark:commit`; `ark agent task archive` is the per-slug helper that backs `ark archive` (also useful for one-off recovery). `ark agent --help` lists the children.

**Operations without a CLI.** Deep-tier iteration (copy `NN_PLAN.md`/`NN_REVIEW.md` to the next number, bump `iteration`, reset `phase = "plan"`) and task reopening are handled by direct file edits — the state machine is small enough that hand-edits stay manageable, and `ark agent task plan/review/...` rejects illegal transitions if the agent gets the phase wrong.

**Cleanup after merge.** When a `--worktree` task's branch has been merged, run `ark agent task worktree cleanup --slug <s> [--delete-branch]` from the parent to remove the worktree dir and (optionally) the branch. Archive does NOT auto-clean. `ark agent task worktree list` enumerates active worktree-backed tasks. `.ark/config.toml`'s `[worktree]` section configures the workflow knobs (`worktree_dir`, `branch_prefix`, `copy`, `post_create`).

**Multi-session focus.** `.ark/.state.toml` carries developer identity, the active-task slug set, and a per-session focus map. Each shell that drives Ark gets its own session id (cached under the OS temp dir, keyed by `(project, ppid)`), so two terminals in the same checkout each see their own focused task. `ark agent task new` warns to stderr when other active tasks already exist (does not refuse). `ark agent task resume --slug <slug>` switches this session's focus to an existing active task. `ark agent task discard --slug <slug>` removes an unarchived task; pass `--force` if seeded files (`PRD.md`, `NN_PLAN.md`, `NN_REVIEW.md`, `VERIFY.md`) have user content. The state file is gitignored, per-checkout (each worktree owns its own), and skipped by `ark unload`.
