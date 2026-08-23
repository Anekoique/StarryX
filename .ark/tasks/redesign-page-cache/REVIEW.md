# `redesign-page-cache` REVIEW

> Status: Closed
> Feature: `redesign-page-cache`
> Target: frozen uncommitted worktree snapshot (base `a076bcc`), including the
> identity refactor, simplification/performance pass, and V-001..V-007 repairs
> Reviewed: 2026-08-23 (supersedes the stale 2026-08-20 review, per V-008)

## Verdict

- Decision: Approved with Revisions
- Blocking: 0
- Non-blocking: 3 (1 HIGH, 1 MEDIUM, 1 LOW)

## Summary

The PLAN's contracts were re-judged against the frozen implementation rather
than on paper, with focus on the post-2026-08-20 passes. The V-001 registry
races are closed airtight, lock nesting slot → registry → pages has no
inversion path, the ext4 orphan release is exactly-once with no self-deadlock
path, direct reclaim no longer has an infallible-allocation hazard, and all
boundary constraints (C-1/C-2/C-15/C-24) hold on source. What blocks a clean
Approved is documentation, not design: the `## Spec` API surface names a
`CacheManager::lookup` that does not exist (and contradicts the PLAN's own
"never a lookup key" rule), misnames `create_mapping`, and omits the
`ensure_registered` re-pin invariant that the V-001 repair depends on — the
promoted SPEC would misdescribe the shipped contract.

## Contract audit (code-verified)

### V-001 closure — registry owns every mapping with pages; reopen converges

Airtight. The three interleavings all resolve correctly:

- Reopen path (`xkernel/src/fs/cache.rs:84-89`) upgrades the slot weak, then
  `ensure_registered` (`xmodules/xcache/src/manager.rs:118-125`) re-inserts
  under the registry lock. If the opener's Arc exists before
  `prune_mapping`'s registry-locked re-check (`manager.rs:243-247`,
  `strong_count == 2` with pointer identity), the prune skips. If the upgrade
  lands inside the prune's critical section, the removal completes first and
  `ensure_registered` re-pins immediately after — it cannot run before the
  removal while the pruner holds the registry lock, and if it runs earlier
  the opener's Arc makes the count ≥3 and the prune skips. No ordering
  strands a live mapping outside the registry.
- Loser path (`cache.rs:98-101`) releases its own mapping and re-pins the
  winner.
- Discard vs. reopen is serialized by the slot lock: `CacheSlot::get`
  upgrades under the attachment lock (`xmodules/xvfs/src/node/file.rs:30-32`)
  and `complete_unlink` decides inside `detach_if` under the same lock
  (`cache.rs:126-139`, `file.rs:51-60`), so a concurrent open either sees the
  retained mapping (`discard_unowned` returns false at strong count ≥2,
  `manager.rs:137`) or a cleared slot. A transient `cached_len` Arc can only
  force the conservative "retained" outcome, never a mid-open discard.
- No page can be inserted into an unregistered mapping: pages are only
  created through `Arc<FileMapping>` handles reachable from `CachedMapping`
  or `FileVmObject`, both of which pin the count ≥3 and therefore defeat
  every prune/discard predicate.

### Lock nesting — slot → registry → pages, no inversion

Verified. xcache has no xvfs dependency (`xmodules/xcache/Cargo.toml`), so
the manager can never take a slot lock. `discard_unowned` nests registry →
pages → page-state → candidates (`manager.rs:132-147`), matching the declared
order (candidates rank below registry). `prune_mapping` releases the pages
lock before taking the registry lock (`mapping/mod.rs:453-455` via
`manager.rs:238-242`). No path holds candidates and takes registry or pages.
`InodeSlot::Drop` takes the ext4 inner lock, but every last-Arc drop of a
`FileMapping` happens after cache locks are released (prune loop iteration
end; in `complete_unlink` the caller's borrowed node keeps the Inode alive so
the drop chain does not fire under the slot/registry locks).

### Direct reclaim (C-12)

The V-002 abort hazard is gone: candidate re-insertion is `try_reserve(1)`
guarded and drops the candidate on failure, with recovery via idle pruning or
discard (`manager.rs:264-273`); registration is likewise fallible
(`manager.rs:406-411`). The scan is bounded (two clock passes), acceptance
requires clean + no lease/guard/writeback + unique frame under the page-tree
and page-state locks (`mapping/mod.rs:436-451`), and there is no sleep. Two
residual edges are documented in R-002/R-003.

### ext4 exactly-once orphan release

Correct. The old racy `strong_count` check is gone; release now rides
`Arc<InodeSlot>` drop semantics (`xcore/xfs/src/fs/ext4/fs.rs:30-51`), which
the language guarantees runs exactly once regardless of alias-drop
interleaving. `release_unlinked` is a no-op for `nlink != 0`
(`crates/lwext4_rust/src/fs.rs:130-137`). No self-deadlock path: the fs inner
lock is never held at any site that can drop the last `InodeSlot` Arc —
`Inode::new` under the inner lock only creates slots (`ext4/inode.rs:34`,
`fs.rs:87-99`), and the `rename` temporary guard drops before its locals. The
`unsafe impl Send/Sync` are now bounded `M: RawMutex + Send + Sync` with a
SAFETY comment (`fs.rs:102-107`), closing V-007.

### Boundaries

- C-1: `#![forbid(unsafe_code)]` at `xmodules/xcache/src/lib.rs:10`.
- C-2: xcache deps are xerrno/xmm/weak-map/xsync/xtask/log/spin only.
- C-24: no `VmFile`/`FileInvalidation`/xcache token in `xmodules/xvma/src`;
  the single allocator lives at `xmodules/xvma/src/object.rs:28-35`;
  `xkernel/src/mm/file_mapping.rs` is the sole cross-module adapter.
- C-15: routing centralized in `mapping_for`
  (`xkernel/src/fs/cache.rs:80-103`) and `fd/file.rs:35-46` (O_DIRECT bypass
  rejection included); mmap consumes the same `CachedMapping` via
  `FileVmObject` (`syscall/mm/mmap.rs:128`).

### Other repairs spot-checked

- V-003: cross-mapping rotation via `writeback_resume`
  (`manager.rs:189-207`).
- V-006: background writeback treats a closed-admission mapping as
  zero-progress skip while explicit sync still propagates
  (`mapping/writeback.rs:76-83`).
- V-004: PLAN C-3 now states the single-allocator invariant matching
  `object.rs`; no id-namespace-split residue anywhere (repo grep clean).
- Two-phase shrink matches the PLAN transaction exactly: validate-all before
  any infallible invalidate, reserved drain slot, `set_len` as the only
  irreversible step, tail zero + beyond-EOF removal + size publication
  (`mapping/resize.rs:76-171`).
- Dirty accounting, PageState sequences, and single-flight loads match the
  `## Spec` structs field-for-field (`page.rs:26-99`,
  `mapping/mod.rs:263-368`).

Evidence accepted as recorded (not re-launched, per instruction): boundary /
build / clippy / fmt pass on this tree; focused runs 6a8ada3f/48/4d/57/5c;
full cases 13/13 (`6a8ada63-0042d178-1256a`); iozone fresh boots
6a8ada76/6a8adae9/6a8adb5d with strict comparator 33/33; full OS-COMP in
flight (6a8adc1d) — the commit gate must confirm its terminal result.

## Findings

### R-001 `## Spec` API surface misdescribes the frozen identity contract
- Severity: HIGH
- Section: PLAN `## Spec` [**API Surface**] (PLAN.md:434-442) vs. "File
  identity and routing" (PLAN.md:49-52)
- Problem: The API surface lists `CacheManager::mapping(...)` (implemented as
  `create_mapping`, `xmodules/xcache/src/manager.rs:102`) and
  `CacheManager::lookup(&self, id) -> LinuxResult<Option<Arc<FileMapping>>>`,
  which does not exist anywhere in the crate and directly contradicts the
  PLAN's own identity rule that the object id "is never a lookup key"
  (PLAN.md:52). It omits `CacheManager::ensure_registered`
  (`manager.rs:118-125`), and no constraint states the invariant the V-001
  repair rests on: any mapping revived through the slot's weak reference must
  be re-pinned in the registry before use, and unlink-discard must decide
  under the slot lock. The `FileMapping.observers` field is also shown as
  `BTreeMap<u64, Weak<..>>` where the code uses a self-cleaning `WeakMap`
  (`mapping/mod.rs:136`).
- Why it matters: the `## Spec` is promoted verbatim on deep-tier commit.
  Future audits would grade against a phantom `lookup` API, an internally
  contradictory identity rule, and a contract missing its load-bearing
  registry-re-pin invariant — inviting false FAILs or a silent
  reintroduction of the V-001 race by a "spec-conformant" refactor.
- Recommendation: in PLAN.md, replace `mapping` with the real
  `create_mapping` signature, delete `lookup`, add `ensure_registered`, align
  the `observers` type, and extend C-3 (or add a constraint) with: "the
  registry pins every mapping with resident pages; a weak-slot revival
  re-registers before use; unlink discard decides under the slot lock."
  Documentation-only edit; the frozen code already implements this.

### R-002 Constraint C-12 wording states the opposite of its intent
- Severity: MEDIUM
- Section: PLAN `## Spec` C-12 (PLAN.md:559-560)
- Problem: C-12 reads "Direct reclaim allocates, waits and performs no I/O",
  which literally asserts that direct reclaim allocates and waits — the
  inverse of the prose contract ("performs no allocation, wait or I/O",
  PLAN.md:186-188) and of the implementation. It also predates the accepted
  V-002 remedy: the code performs one *fallible* `try_reserve(1)` on the
  candidate deque and degrades by dropping the candidate on failure
  (`manager.rs:264-273`), which is not literally "no allocation".
- Why it matters: this is a load-bearing acceptance constraint; its garbled
  text cannot be satisfied or audited as written, and the fallible
  reservation deserves explicit sanction so a later auditor does not flag it
  as a breach.
- Recommendation: reword to "Direct reclaim never sleeps, performs no I/O,
  and makes no allocation required for progress; its only allocator call is a
  fallible candidate-list reservation whose failure drops the candidate
  safely; it removes only clean, idle pages with unique cache Frame
  ownership."

### R-003 Direct-reclaim drop chain can run ext4 release I/O
- Severity: LOW
- Section: PLAN C-12 / "Reclaim and lifecycle"
- Problem: when direct reclaim removes the last page of an unowned mapping,
  `try_reclaim` → `prune_mapping` unregisters it and the local Arc drop at
  `manager.rs:280-291` chains `FileMapping` → `VfsBacking` → `Inode` →
  `InodeSlot::Drop` → `fs.lock().release_unlinked` (`ext4/fs.rs:42-51`),
  i.e. filesystem metadata I/O (and fs-lock contention) synchronously inside
  `allocate_frame`'s reclaim path. All cache locks are already released at
  that point, so there is no inversion or deadlock, and the trigger is
  narrow (last alias of a file, typically open-unlinked), but it stretches
  the "no I/O in direct reclaim" reading.
- Why it matters: worst case is a bounded latency spike in a fault/load path
  under memory pressure; correctness is unaffected.
- Recommendation: accept and document (one sentence in the Reclaim section:
  "final-owner drop chains may perform backing release I/O after all cache
  locks are released"), or defer final mapping drops to the worker.

## Trade-off Advice

### TR-1 Candidate re-insertion: fallible reserve vs. holding the lock
- Related Plan Item: C-12 / `reclaim_clean`
- Reviewer Position: Prefer A (current fallible `try_reserve` re-insertion)
- Advice: keeping the candidates lock across examine/reinsert (the verifier's
  alternative in V-002) would nest candidates → page-state, inverting the
  declared order 5→6 and coupling reclaim latency to page-state contention.
- Rationale: the dropped-candidate degradation is self-healing (idle pruning
  and discard still free the page) and the deque's spare slot makes actual
  allocation a rare race outcome.
- Required Action: none beyond the C-12 rewording in R-002.

### TR-2 Reopen protocol: optimistic upgrade + re-pin vs. registry-locked lookup
- Related Plan Item: identity section / V-001 closure
- Reviewer Position: Prefer A (current upgrade-then-`ensure_registered`)
- Advice: a registry-locked lookup keyed by id would reintroduce the id as a
  lookup key, contradicting the PLAN's identity rule, and would serialize
  every open on the global registry lock. The current protocol is proven
  airtight above and keeps the slot as the sole identity authority.
- Required Action: codify the re-pin invariant in the constraint set (R-001).
