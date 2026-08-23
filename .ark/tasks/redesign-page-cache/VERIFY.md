# `redesign-page-cache` VERIFY

> Status: PASS
> Feature: `redesign-page-cache`
> Tier: `deep`
> Verified: 2026-08-23 (second pass; supersedes the same-day FAIL that raised
> V-001..V-008)
> Target: frozen uncommitted worktree snapshot (base `a076bcc`) carrying the
> V-001..V-008 repairs; source snapshot SHA-256 `6e15e3d8…7b7a` (verified below)

## Result

All eight findings from the first 2026-08-23 audit are repaired on this
snapshot and the repairs introduce no new CRITICAL or HIGH defect. The
slot/registry TOCTOU is closed by `ensure_registered` re-pinning plus
slot-locked unlink discard; direct reclaim no longer performs infallible
allocation; writeback rotates across mappings; the ext4 orphan release is
exactly-once by `Arc` drop semantics; closed-admission mappings no longer leak
`EAGAIN` into unrelated writes; the `unsafe impl`s are bounded and justified;
PLAN and REVIEW now describe the shipped contract. Every cheap gate was re-run
on this exact tree and passes; the strict iozone comparator re-run reproduces
`docs/benchmarks/iozone-page-cache.json` byte for byte, proving the evidence
was generated on this repaired tree. Two residual issues remain (1 MEDIUM,
1 LOW), neither blocking.

## Verification commands (re-run on this snapshot, 2026-08-23, second pass)

- PASS — `scripts/check-page-cache-boundary` → `page-cache boundary: ok`.
- PASS — `cargo check -p xcache -p xvma -p xvfs -p xmm -p xkernel -p starry
  --target riscv64gc-unknown-none-elf`.
- PASS — `cargo clippy -p xcache -p xvma -p xvfs -p xmm -p xalloc -p xkernel
  -p starry --target riscv64gc-unknown-none-elf --no-deps -- -D warnings`
  (same two repo-wide command-line exemptions as prior passes:
  `new_without_default`, `useless_conversion`; no source suppressions).
- PASS — `rustfmt --edition 2024 --check` over every changed `.rs` file
  (exit 0).
- PASS — canonical release build:
  `make build ARCH=riscv64 BLK=y NET=y MODE=release LOG=off` (exit 0,
  final image objcopy completed).
- PASS — `cargo test --manifest-path xtest/Cargo.toml --release`: 31/31 host
  framework tests, including `multi_boot_case_requires_an_isolated_run`.
- PASS — `scripts/bench/compare-page-cache-iozone` re-run against the three
  recorded fresh boots (`6a8ada76`, `6a8adae9`, `6a8adb5d`): 33/33 medians
  strictly above baseline, `page-cache iozone gate: PASS`; regenerated JSON is
  byte-identical to `docs/benchmarks/iozone-page-cache.json`. The comparator
  recomputes `starryx_source_sha256` from the *current* tree (Cargo.lock,
  crates/lwext4_rust, starry, xcore, xkernel, xmodules — committed, uncommitted
  and untracked files), so byte-identity proves the implementation is frozen
  since evidence generation (`6e15e3d8ddd17851642dc57ec9fb53bbacd9c4e3019cf7e
  49412c93617754b7a`).
- N/A — root `cargo fmt --all`: nested Ark worktree resolves one excluded
  crate against the main checkout (same limitation as prior passes); changed
  files were checked directly instead.
- N/A — loongarch64 build: no per-arch asm/linker file changed; riscv64 is the
  evidence architecture per the PLAN's verification plan.

## Guest and performance evidence (recorded on this snapshot; reports verified)

QEMU runs were not re-launched (per dispatch instruction); each cited run
directory exists, is immutable, and reports an honest terminal `passed`:

- Focused: `6a8ada3f` (`fs/page_cache`), `6a8ada48` (`fs/page_cache_unlink`),
  `6a8ada4d` (`fs/page_cache_pressure`), `6a8ada57` (`mm/file_mmap`),
  `6a8ada5c` (`fs/page_cache_persist`, two boots — `serial.boot-1.log` and
  `serial.boot-2.log` present) — all `passed`.
- `6a8ada63-0042d178-1256a` — full first-party cases: 13/13, `passed`.
- `6a8adc1d-21259c68-13171` — full OS-COMP: 10/10 suites `pass` with
  `exit_code: 0` each, run status `passed`.
- iozone fresh boots `6a8ada76-16b60e70-127fb`, `6a8adae9-36fe5a98-12add`,
  `6a8adb5d-242fcb40-12db9` — strict comparator PASS (33/33), re-validated
  above.

## Project Spec Compliance

- N/A — `.ark/specs/project/` contains only the template `INDEX.md`; no
  user-authored project SPEC exists. The enforced floor is `CLAUDE.md`:
  - PASS — rustfmt/clippy clean (re-run above).
  - PASS — no new `.unwrap()` in production paths (the two diff-added
    `.unwrap()`s are inside `#[cfg(test)]` code, `xmodules/xvma/src/area.rs:222,233`);
    `.expect()` uses carry logically-impossible conditions
    (`writeback.rs:224`, `resize.rs:163`, `manager.rs:373`,
    `space.rs:302`) or are boot/shutdown surfacing (`starry/src/main.rs:72`).
  - PASS — new `unsafe` carries `// SAFETY:` comments
    (`xcore/xmm/src/frame.rs:249-289`; `xcore/xfs/src/fs/ext4/fs.rs:102-107`
    now bounded and justified, closing V-007). Exception: one bare FFI unsafe
    in the vendored lwext4 crate — V-010 (LOW).
  - PASS — no `xmodules → xkernel` dependency; xcore stays OS-agnostic
    (boundary script + `xmodules/xcache/Cargo.toml` audit: xerrno, xmm,
    weak-map, xsync, xtask, log, spin only).
  - PASS — file sizes within the 800-line cap (largest changed:
    `xmodules/xcache/src/mapping/mod.rs` 519, `xmodules/xvma/src/space.rs` 514).
  - PASS — dead code removed rather than accumulated: the commented-out
    `XAllocIf::evict_cache` seam and trait were deleted
    (`xcore/xalloc/src/lib.rs`), `FileWrapper` removed from
    `xkernel/src/mm/uspace.rs`.

## Related Feature Spec Compliance

### `kernel/mm/redesign-mm-subsystem/SPEC.md` (amended by this task)

- C-1..C-6 PASS — `FrameMeta` remains one `AtomicU32 ref_count`; PTE/Frame
  reference transfer unchanged; `try_write_at` still unique-only (used at
  `xmodules/xcache/src/mapping/mod.rs:333`).
- C-7 PASS — no cache/dirty/writeback state in `xmm` (`frame.rs` grep clean).
- C-8 PASS — `xmodules/xvma/src/lib.rs` `#![forbid(unsafe_code)]`.
- C-9..C-18, C-20 PASS — unchanged mechanisms; changed call sites re-checked
  and clippy clean.
- C-19 PASS — `Frame::{read_bytes,write_bytes}` hold `NoPreemptIrqSave` for
  the whole bounded copy and expose no slice (`frame.rs:237-289`).
- C-20/C-21 PASS — `xmodules/xvma/src/object.rs` is the only page-source seam;
  the single never-reused object-id allocator lives at `object.rs:28-35`; no
  file/cache type or observer registry in xvma (boundary script inventory).
- C-22 PASS — guard storage reserved before any writable PTE
  (`xmodules/xvma/src/fault.rs:183-210` `commit_page`); `unmap_object_range`
  removes PTEs before dropping guards (`xmodules/xvma/src/space.rs:298-307`).
- C-23 PASS — all observer `validate` calls precede infallible `invalidate`
  (`xmodules/xcache/src/mapping/resize.rs:103-140`,
  `xkernel/src/mm/file_mapping.rs:141-158`).

### `xtest/redesign-xtest-framework/SPEC.md` (amended by this task)

- PASS — multi-boot (isolated case, per-boot serial logs, disposable image)
  exercised by `6a8ada5c` (`fs/page_cache_persist`, two boot serials) and host
  test `multi_boot_case_requires_an_isolated_run` (re-run, passed); immutable
  run directories and truthful terminal reports confirmed on all cited runs.

### `xtest/port-oscomp-suites/SPEC.md`

- PASS — OS-COMP profile unmodified; run `6a8adc1d` reports 10/10 native
  verdicts (`outcome: pass`, `exit_code: 0` per suite), no quarantine or
  workload change; the comparator additionally rejects workload-marker
  mismatches and passed.

## PRD Outcomes

1. PASS — one live mapping per file incarnation via per-inode `InodeSlot`/
   `CacheSlot` (`xcore/xfs/src/fs/ext4/fs.rs:87-99`,
   `xkernel/src/fs/cache.rs:80-103` compare-and-attach with loser
   `release_hint` + re-pin); pseudo files/devices take the `None` bypass
   (`xmodules/xvfs/src/node/file.rs:70-72`). The V-001 race window is closed
   (see Findings resolutions).
2. PASS — xcache is `#![forbid(unsafe_code)]` (`lib.rs:10`), owns pages only
   through `xmm::Frame`; single-flight `LoadAttempt` publication
   (`xmodules/xcache/src/mapping/mod.rs:263-368`).
3. PASS — dirty/writeback/redirty/error/isolation state in `PageState`
   sequences (`xmodules/xcache/src/page.rs:26-99`); no cache lock crosses
   backing I/O, sleep, or observer callbacks (lock-scope audit of
   writeback.rs, resize.rs; observer invalidate runs with no cache lock held,
   `resize.rs:105-113`).
4. PASS — buffered/positioned/append/truncate/sync/stat/seek routed through
   the cache in `xkernel/src/fs/fd/file.rs`; `O_DIRECT` on cacheable nodes is
   an explicit `EOPNOTSUPP` (`file.rs:35-37`); guest `fs/page_cache` passes.
5. PASS — private COW, shared guarded writes, msync, truncate invalidation,
   EOF SIGBUS (`xmodules/xvma/src/fault.rs`; `sys_msync` at
   `xkernel/src/syscall/mm/mmap.rs:212-236`; guest `mm/file_mmap` passes).
6. PASS — worker/watermarks/clean-only reclaim delivered
   (`manager.rs:149-171,251-291`); direct reclaim's only allocator call is the
   fallible `try_reserve(1)` re-insertion that drops the candidate on failure
   (`manager.rs:264-273`), exactly as C-12 now sanctions. V-002 closed.
7. PASS — one-way boundaries hold (boundary script; Cargo.toml audit;
   `xkernel/src/mm/file_mapping.rs` is the sole cross-module adapter).
8. PASS — focused deterministic guest cases cover races, redirty, errors,
   truncate, coherence, reclaim, accounting (13/13). Residual limit: no
   host-level fault injector for every allocation/I-O failure interleaving
   (unchanged; noted, not a constraint breach).
9. PASS — full first-party 13/13 (`6a8ada63`) and OS-COMP 10/10 (`6a8adc1d`),
   honest terminal reports, persistence across restart (`6a8ada5c`).
10. PASS — 33/33 strict per-metric iozone medians above baseline on three
    fresh boots; comparator re-run reproduces the committed evidence exactly.

## Plan Fidelity

### Goals

- G-1 PASS — single coherent representation (Outcomes 1/4/5).
- G-2 PASS — boundaries preserved (Outcome 7; mm SPEC C-21).
- G-3 PASS — load/writeback/invalidation/reclaim/shutdown are finite and
  failure-safe; the V-001/V-002 race windows that failed this goal in the
  first pass are repaired and re-audited (registry re-pin invariant, slot-
  locked discard, fallible-only reclaim allocation).
- G-4 PASS — memory bounded by watermarks; all 33 iozone metrics improved.

### Constraints

- C-1 PASS — `xmodules/xcache/src/lib.rs:10`; boundary script.
- C-2 PASS — `xmodules/xcache/Cargo.toml` deps: xerrno, xmm, weak-map, xsync,
  xtask, log, spin only.
- C-3 PASS — single never-reused allocator (`xmodules/xvma/src/object.rs:28-35`);
  aliases share one `InodeSlot` (`ext4/fs.rs:87-99`); recreated files get
  fresh slots (self-cleaning `WeakMap`); registry pins every mapping holding
  pages; weak-slot revival re-registers before use
  (`xkernel/src/fs/cache.rs:84-89,98-101` → `manager.rs:118-125`); unlink
  discard decides under the slot lock (`cache.rs:126-139` via
  `CacheSlot::detach_if`, `xmodules/xvfs/src/node/file.rs:51-60`). The stale
  id-namespace-split wording (V-004) is rewritten.
- C-4 PASS — per-attempt result publication; replacement loaders publish
  `EAGAIN` to old waiters (`mapping/mod.rs:343-368`).
- C-5 PASS — page-state lock crosses only the bounded one-hart frame copy;
  backing I/O, sleeps, observer callbacks, and fallible allocation happen
  outside all cache locks (`writeback.rs:145-201`, `resize.rs:103-140`).
  Frame deallocation under the page-tree lock (discard/shrink/reclaim tallies)
  is a bounded leaf-lock operation, judged within the constraint's intent.
- C-6 PASS — `mark_dirty` always advances `dirty_seq` (`page.rs:49-56`);
  settle advances `persisted_seq` only to the submitted sequence
  (`writeback.rs:182`), so redirty survives.
- C-7 PASS — failed page stays resident+dirty, background skips
  `failed_seq == dirty_seq` (`page.rs:91-96`); each cursor reports each error
  generation once (`writeback.rs:228-235`).
- C-8 PASS — `fs/page_cache` in `6a8ada3f` and `6a8ada63`.
- C-9 PASS — enrollment enters mapping admission (`resize.rs:57-74`); token
  owns observer (`resize.rs:28-41`); validate-all before invalidate
  (`resize.rs:103-140`).
- C-10 PASS — `mm/file_mmap` in `6a8ada57`.
- C-11 PASS — WRITE only with guard (`fault.rs:153-210`); PTE removal precedes
  guard release (`space.rs:190,246,298-307`).
- C-12 PASS — as reworded (per R-002/TR-1): no sleep, no I/O, no allocation
  required for progress; the only allocator call is the fallible
  `try_reserve(1)` whose failure drops the candidate (`manager.rs:264-273`);
  registration is likewise fallible (`manager.rs:406-411`); acceptance
  requires clean, idle, unique-frame pages under the page-tree and page-state
  locks (`mapping/mod.rs:436-451`); the final-owner drop chain into ext4
  deferred release runs after all cache locks are released (accepted, R-003,
  now stated in C-12).
- C-13 PASS — `fs/page_cache_pressure` in `6a8ada4d`.
- C-14 PASS — `Running→Closing→Closed`, worker-owned or inline flush, `EBUSY`
  on unresolved data, no I/O in `Drop` (`manager.rs:343-390,464-468`). The
  V-001 spurious-`EBUSY` path is gone: every mapping holding pages is
  registered, and `shutdown_flush` scans the full resident range explicitly
  (`writeback_some` explicit path, `writeback.rs:84-85`; the `dirty_pages == 0`
  fast path and the closed-admission `Ok(0)` skip apply only to `!explicit`,
  `writeback.rs:73,81`).
- C-15 PASS — routing centralized in `mapping_for`/`File`
  (`xkernel/src/fs/cache.rs:80`, `fd/file.rs:40`, `syscall/mm/mmap.rs:94`);
  boundary script routing inventory ok; `cache_slot`/`mapping_for` appear
  nowhere else in syscall paths.
- C-16 PASS — host test `multi_boot_case_requires_an_isolated_run` (re-run).
- C-17 PASS — `fs/page_cache_persist` two boots in `6a8ada5c`.
- C-18 PASS — 31/31 (re-run).
- C-19 PASS — boundary script (re-run).
- C-20 PASS — comparator re-run, 33/33, evidence reproduced byte-identically.
- C-21 PASS — OS-COMP 10/10 in `6a8adc1d` (recorded; report verified).
- C-22 PASS — one-refcount `FrameMeta`; bounded copies expose no direct-map
  reference (`frame.rs:237-289`); no cache state in xmm.
- C-23 PASS — `fs/page_cache_unlink` (dirty unlink cycles) in `6a8ada48`; the
  V-005 concurrent-alias race is structurally closed (exactly-once `Arc` drop),
  so the sequential test's blind spot no longer hides a defect.
- C-24 PASS — boundary script; `xmodules/xvma/src` contains no
  `VmFile`/`FileInvalidation`/xcache reference.

### Repair audit (requested focus)

- **`detach_if` nesting (slot → registry → pages, no inversion).** Verified.
  xcache carries no xvfs dependency, so the manager can never take a slot
  lock; the only slot→cache path is `complete_unlink` → `detach_if` →
  `discard_unowned` (registry → pages → page-state → candidates), matching
  the declared order. `complete_unlink` reads `node.metadata()` (ext4 inner
  lock) *before* taking the slot lock (`cache.rs:121-126`). Inside
  `detach_if`, the closure drops its mapping Arc before `discard_unowned`
  checks `strong_count == 1` (`cache.rs:130-131`), and `registry.remove` can
  only destroy a `FileMapping` whose `VfsBacking` holds an `Inode` Arc that
  the caller's borrowed node also holds — so the `InodeSlot` drop chain
  cannot fire under the slot/registry locks.
- **Re-pin protocol (V-001 closure).** Verified airtight across
  interleavings: `prune_mapping` re-checks pointer identity and
  `strong_count == 2` under the registry lock (`manager.rs:242-247`); an
  upgrade landing inside that critical section is followed by
  `ensure_registered`, which must wait for the registry lock and re-inserts
  (`manager.rs:118-125`); an upgrade landing before makes the count ≥3 and
  the prune skips. Both `mapping_for` paths (direct hit at `cache.rs:84-89`,
  lost race at `cache.rs:98-101`) re-pin before returning. `cached_len`
  upgrades without re-pinning but is read-only and can only force the
  conservative "retained" outcome in `discard_unowned`.
- **`ensure_registered` during shutdown.** `permit()` fails `ESHUTDOWN` once
  admission closes (`manager.rs:392-404`); `mapping_for` propagates it with
  `?`, so no opener can hold an unregistered live mapping across
  `shutdown_flush`, which itself first drains `active_operations`
  (`manager.rs:378-380`).
- **`InodeSlot` drop path.** Exactly-once by `Arc` semantics
  (`ext4/fs.rs:42-51`); release failure is logged, never swallowed silently.
  No self-deadlock: audited every ext4 inner-lock scope in `inode.rs` —
  lock-held code only *creates* Inodes/slots (lookup/create/link) and never
  drops a last `Inode` Arc; `rename`'s temporary guard drops at statement end
  before its locals; `inode_slot` uses the separate `inode_slots` mutex.
- **Rotation correctness (`writeback_inner`).** The pivot resumes strictly
  after `writeback_resume`, wrapping via `unwrap_or(0)`; the resume id is
  stored after each serviced mapping including the one that exhausts the
  budget, so the next batch starts past it; a mapping whose `writeback_some`
  errors is retried first next batch (resume not advanced past it) and its
  failed page is then skipped via `failed_seq` (`manager.rs:189-207`).
- **Dirty-gate / `Ok(0)` vs explicit sync and shutdown.** The
  `dirty_pages == 0` fast path and the closed-admission skip are `!explicit`
  only (`writeback.rs:73,76-83`); explicit sync and `shutdown_flush`
  (explicit=true, `manager.rs:381`) scan the full resident range
  (`writeback.rs:84-85`) and propagate `enter_operation` errors. Dirty
  accounting re-traced across the repaired paths: clean→dirty edges counted
  once under the page-state lock (`mapping/mod.rs:370-390`, `page.rs:138-156`),
  dirty→clean settles decrement once (`writeback.rs:192-196`), guard drops
  redirty without recount (`page.rs:178-193`), discard/shrink tally under the
  page-tree lock while drained (`mapping/mod.rs:468-492`, `resize.rs:152-171`).
  No drift path; global `resident/dirty` counters still backstop shutdown with
  `EBUSY`.

## SPEC Drift

- PASS — `kernel/mm/redesign-mm-subsystem/SPEC.md` amendments carry dated
  CHANGELOG entries (2026-08-17, 2026-08-20) covering the shipped wording.
- PASS — `xtest/redesign-xtest-framework/SPEC.md` multi-boot amendment carries
  its 2026-08-17 CHANGELOG entry.
- Note (carried over): entries describe the change but do not name
  `redesign-page-cache` as the source task; acceptable under the current
  template, worth naming on promotion.
- PASS — REVIEW.md refreshed 2026-08-23 against this snapshot (closes V-008);
  its three doc revisions are applied in PLAN.md: R-001 (API surface lists
  `create_mapping` and `ensure_registered`, no `lookup`; `observers` shown as
  `WeakMap`, PLAN.md:390,436-444; C-3 states the re-pin/slot-lock invariant,
  PLAN.md:543-546), R-002 (C-12 reworded, PLAN.md:563-568), R-003 (drop-chain
  release sentence in C-12, PLAN.md:566-568).

## Findings

## Severity Summary: 0 CRITICAL · 0 HIGH · 1 MEDIUM · 1 LOW
## Verification: build PASS · tests PASS(31 host + 13/13 cases + 10/10 oscomp recorded/0 failed) · lint PASS · format PASS

### Prior findings (first 2026-08-23 pass) — resolutions verified on this snapshot

- V-001 (HIGH, slot/registry TOCTOU) — **FIXED**: `ensure_registered` after
  every weak revival (`xkernel/src/fs/cache.rs:84-89,98-101`,
  `manager.rs:118-125`); registry-locked identity+count re-check in
  `prune_mapping` (`manager.rs:242-247`); unlink discard decided under the
  slot lock via `CacheSlot::detach_if` with slot cleared on success
  (`cache.rs:126-139`, `xmodules/xvfs/src/node/file.rs:51-60`). Interleaving
  audit above found no ordering that strands a live mapping.
- V-002 (HIGH, allocation in direct reclaim) — **FIXED**: candidate
  re-insertion is `try_reserve(1)`-guarded and drops the candidate on failure
  (`manager.rs:264-273`); PLAN C-12 sanctions exactly this; REVIEW TR-1
  documents why this beats holding the candidates lock (5→6 order inversion).
- V-003 (MEDIUM, cross-mapping starvation) — **FIXED**: `writeback_resume`
  rotation in `writeback_inner` (`manager.rs:59-61,189-207`); correctness
  audited above.
- V-004 (MEDIUM, stale C-3) — **FIXED**: C-3 rewritten to the
  single-allocator + slot + re-pin invariant (PLAN.md:540-546); no
  id-namespace-split residue (repo grep clean).
- V-005 (MEDIUM, concurrent alias drops skip release) — **FIXED**: `InodeSlot`
  alias token whose `Arc` drop performs `release_unlinked` exactly once
  (`xcore/xfs/src/fs/ext4/fs.rs:30-51`); `Inode` carries no manual `Drop`
  (`inode.rs:18-41`); no self-deadlock path (audit above).
- V-006 (MEDIUM, EAGAIN propagation) — **FIXED**: background `writeback_some`
  returns `Ok(0)` for a closed-admission mapping; explicit sync still
  propagates (`writeback.rs:76-83`). Residual error-class issue tracked as
  V-009.
- V-007 (LOW, unsafe impls) — **FIXED**: `unsafe impl<M: RawMutex + Send +
  Sync> Send/Sync for Ext4Filesystem<M>` with a SAFETY comment
  (`ext4/fs.rs:102-107`); struct gained the `RawMutex` bound (`fs.rs:19`).
- V-008 (LOW, stale REVIEW) — **FIXED**: REVIEW.md rewritten 2026-08-23 on
  this snapshot (Approved with Revisions; all three revisions applied in
  PLAN.md, verified under SPEC Drift).

### V-009 Background writeback I/O errors propagate to unrelated writes through the dirty throttle
- Severity: MEDIUM
- Location: `xmodules/xcache/src/mapping/writeback.rs:96` (`writeback_some`,
  `?` on `writeback_page`), `xmodules/xcache/src/manager.rs:203`
  (`writeback_inner`), `manager.rs:209-216` (`throttle_dirty`), reaching
  `xmodules/xcache/src/mapping/mod.rs:224` (`write_at_inner`) and
  `page.rs:141` (`shared_write_guard`)
- Problem: The V-006 repair skips only the closed-admission `EAGAIN`; a
  genuine backing write error inside a background batch still propagates out
  of `writeback_some` → `writeback_inner` → `throttle_dirty`, so a `write(2)`
  or shared-write fault on a *healthy* file can fail with another file's
  `EIO` whenever total dirtiness sits at `dirty_limit` while a failing
  mapping's page is submitted. The error is bounded (the failed page's
  `failed_seq` makes the next background pass skip it) but recurs on every
  redirty of the failing file, and the same abort also cuts short the
  worker's batch for the round.
- Why it matters: spurious `EIO` from `write(2)` on an unrelated regular file
  is non-Linux behavior; the mapping-error cursor already exists precisely so
  errors reach the *owning* file's sync.
- Recommendation: inside background (`!explicit`) batches, record the error on
  the owning mapping (already done via `record_error`) and treat the page as
  "no progress" instead of propagating — reserve propagation for explicit
  sync, mirroring the V-006 remedy.
- Resolution: DEFERRED — accepted at commit by the user; a follow-up task is
  queued to record-and-continue in background batches. Fixing it here would
  invalidate the frozen benchmark provenance for a non-blocking staleness
  issue.

### V-010 New FFI `unsafe` block without a SAFETY comment in vendored lwext4
- Severity: LOW
- Location: `crates/lwext4_rust/src/blockdev.rs:95-98` (`flush`)
- Problem: The newly added `ext4_block_cache_flush` call sits in a bare
  `unsafe { … }` with no `// SAFETY:` comment. The surrounding vendored crate
  pervasively omits them, but CLAUDE.md requires new `unsafe` to spell out its
  invariants.
- Why it matters: consistency of the project's unsafe-audit floor; the
  invariant (exclusive `&mut` access to the initialized `ext4_blockdev`) is
  real and worth one line.
- Recommendation: add the one-line SAFETY comment.
- Resolution: DEFERRED — bundled with the V-009 follow-up; even a comment-only
  edit changes the evidence-bound source hash, so it is not worth re-running
  the full gate chain alone.

## Notes

- The evidence chain is self-proving: `compare-page-cache-iozone` recomputes
  the source snapshot hash over committed + uncommitted + untracked files of
  the implementation trees, and its regenerated output is byte-identical to
  `docs/benchmarks/iozone-page-cache.json` — the guest/benchmark evidence was
  generated on exactly this repaired tree (`6e15e3d8…7b7a`). The earlier
  `b13fd547…` hash belongs to the superseded pre-repair snapshot.
- Unlink-vs-reopen corner (opener upgraded the slot weak before the unlinker
  took the slot lock, mapping already pruned from the registry): the discard
  finds no registry entry, clears the slot, and the opener's
  `ensure_registered` re-pins — yielding correct open-unlinked semantics
  (pages retained, writeback works, `InodeSlot` release deferred to the last
  owner). No data-loss interleaving found.
- Concurrent `mmap`/`fork` against an in-flight truncate of the same file can
  fail with transient `EAGAIN` through observer enrollment; this is the
  behavior C-9 explicitly sanctions ("completes before shrink captures
  observers or fails after the gate closes").
- Residual test-depth limitation carried over: no deterministic host fault
  injector for every allocation/backing-I/O failure interleaving; those paths
  were re-audited from source in this pass.
