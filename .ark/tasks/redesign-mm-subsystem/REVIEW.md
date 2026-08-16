# Decoupled MM Design REVIEW — Iteration 2

> Status: Closed
> Feature: `redesign-mm-subsystem`
> Owner: Reviewer
> Target Plan: `PLAN.md`
> Scope: Plan correctness · Spec alignment · Design soundness · Validation adequacy · Trade-off advice

---

## Verdict

- Decision: Rejected
- Blocking: 9
- Non-blocking: 0

## Summary

The rewrite fixes the central ownership error: removing
`MemorySet<Backend>` from `xmm`, retaining owned frames beside PTEs, and using
one `xvma` VMA tree is the correct architecture. It also makes borrowed kernel
mappings and local flush ordering explicit. The Spec is still not
self-contained enough to execute. Core `xvma`, object, fault, frame-preparation,
COW, file-mode, transaction, user-copy, and validation contracts are omitted or
contradict the PRD. The findings below must be folded into `PLAN.md` before
EXECUTE.

---

## Findings

### R-101 The process-facing Spec is still placeholder-level

- **Severity:** HIGH
- **Section:** Spec / Data Structures / API Surface
- **Problem:** Every `AddressSpace` method is declared with `...`, while
  `SharedBacking`, `FileBacking`, `SharedObject`, `VmObject`, `FaultOutcome`,
  fault inputs, mapping mode, and public error behavior are absent from the
  Spec. The runtime sections depend on those undeclared contracts.
- **Why it matters:** Different implementations can satisfy the prose while
  exposing incompatible ownership and error semantics. In particular, the
  reviewer cannot determine whether file mappings are private or shared, how
  shared offsets enter `map_shared`, or how `SIGSEGV` versus `SIGBUS` reaches
  the trap layer.
- **Recommendation:** Replace every ellipsis with the complete final API. Define
  all backing structs and the synchronous `VmObject` trait, typed fault input
  and outcome, map/file/shared arguments, checked offsets, and `XError`/Linux
  error translation. The promoted deep SPEC must stand alone without relying on
  current source signatures.

### R-102 The stated prepare-before-publish file fault is impossible through the API

- **Severity:** HIGH
- **Section:** Spec / API Surface / File Fault
- **Problem:** File Fault says a private frame is allocated and filled before
  PTE installation, but the API exposes neither frame allocation nor bounded
  mutable frame access. `PageTableSpace::write` can only write through an
  already installed virtual mapping, which reverses the required ordering.
- **Why it matters:** Implementers must either publish a zero page before I/O or
  use raw physical memory from safe `xvma`/kernel code, violating fault
  atomicity or the trusted boundary.
- **Recommendation:** Add safe `xmm` construction APIs such as
  `Frame::allocate_zeroed(PageSize)`, bounded `Frame::write_at`, and
  `Frame::into_ref`; alternatively provide a safe prepared-frame writer. Define
  short-read handling, final-page zero fill, and failure cleanup before
  `map_owned_page` is called.

### R-103 COW exclusivity and `mprotect` interaction are undefined

- **Severity:** HIGH
- **Section:** Spec / Anonymous Fault / Fork
- **Problem:** The plan says a write fault checks `FrameRef::strong_count`, but
  does not define the baseline count. `owned_page(s)` necessarily returns cloned
  references, so an otherwise exclusive PTE is not count 1. More importantly,
  `protect_range(...WRITE...)` can make a fork-shared private PTE writable unless
  `xvma` preserves COW state while changing logical VMA permissions.
- **Why it matters:** An incorrect threshold causes needless copies; bypassing
  COW through `mprotect` lets parent and child modify the same private frame.
- **Recommendation:** Define exclusivity relative to the registry/local lookup,
  or replace public `strong_count` policy with an `xmm` query that answers
  whether owners exist beyond the current PTE and temporary handle. Specify that
  adding logical WRITE to anonymous/private-file VMAs keeps shared resident PTEs
  read-only; only the subsequent write fault may upgrade or deep-copy them. Add
  the same rule for fork rollback and originally read-only pages.

### R-104 File `MAP_PRIVATE` and `MAP_SHARED` compatibility is contradictory

- **Severity:** HIGH
- **Section:** Goals / File Fault / Trade-off T-4
- **Problem:** G-4 promises preserved file behavior, while T-4 says all file
  pages are private read copies. Current behavior distinguishes private lazy
  file mappings from `MAP_SHARED` file mappings backed by one shared frame set
  that remains shared across fork. The Spec has no map mode in `FileBacking` and
  no explicit shared-file path.
- **Why it matters:** A rewrite may silently turn `MAP_SHARED` into private
  memory or apply COW to shared file pages, breaking visibility after fork and
  changing Linux-visible behavior.
- **Recommendation:** State the compatibility boundary explicitly. A minimal
  valid contract may keep private files as lazy private frames and preserve the
  current shared-file snapshot as one `SharedObject` per mmap, shared across
  fork but not coherent across independent mmaps and without writeback. Define
  which faults yield `SIGBUS`, EOF/final-page behavior, and partial-unmap object
  offsets. Keep cross-mapping page-cache identity and writeback deferred.

### R-105 Safe mechanism methods cannot have best-effort transaction guarantees

- **Severity:** HIGH
- **Section:** Spec / API Surface / PTE mutation ordering
- **Problem:** “Failures leave the previous PTE and ownership entry intact where
  the underlying page-table API permits” weakens the central safety invariant.
  Multi-page borrowed map, unmap, and protect operations can otherwise fail
  after partially changing PTEs while the VMA tree remains unchanged.
- **Why it matters:** A safe caller may receive `Err` with hardware mappings and
  `owned_pages` out of sync, recreating the parallel-state bug the redesign is
  intended to remove.
- **Recommendation:** Give every safe method an unconditional postcondition.
  Preflight complete ranges before mutation, roll back installed leaves on map
  failure, and define holes as skipped or rejected consistently. Make
  replace-owned a single validated leaf transaction. State that
  `copy_mappings_from` imports only borrowed kernel top-level entries and that
  dropping a user page table never frees or edits those borrowed tables.

### R-106 The PRD and PLAN disagree on user-copy completion

- **Severity:** HIGH
- **Section:** PRD Outcome / Non-goal NG-3 / Implementation
- **Problem:** The PRD requires `xuspace` to remove borrowed/raw user-memory
  access, but NG-3 defers its typed POD redesign and no implementation or API
  section defines the replacement. A generic safe `read<T>` implemented by
  copying arbitrary user bytes into `T` is unsound for types with invalid bit
  patterns even if it returns an owned value.
- **Why it matters:** The task can claim lifetime safety while retaining an
  unsound public typed API, or expand unpredictably across dozens of syscall
  callers during execution.
- **Recommendation:** Choose one contract. Prefer completing byte-oriented
  copy-in/copy-out and owned strings/vectors now, with a sealed audited
  `UserPod` set for typed values; remove `raw_ptr`, `raw_slice`, and borrowed
  returns. Otherwise remove the xuspace outcome from this task and explicitly
  retain the existing risk for the separate POD task. Do not expose a safe
  unconstrained `read<T>`.

### R-107 VMA transactions and backing offsets remain implicit

- **Severity:** HIGH
- **Section:** Spec / Architecture / Implementation Phase 3
- **Problem:** The plan says the mechanism is updated “after VMA validation” but
  does not define map/unmap/protect commit order, fixed replacement rollback,
  merge eligibility, or checked backing-offset updates after prefix/middle
  splits. A two-step “map anonymous then attach file metadata” would also leave
  policy coordination in `xkernel`.
- **Why it matters:** A partial operation can leave a PTE without a VMA, a VMA
  without a PTE, or a retained tail pointing at the wrong shared/file page.
- **Recommendation:** Define each `AddressSpace` operation as one transaction:
  prevalidate complete layout and checked `offset + delta`; perform a mechanism
  operation with the R-105 postcondition; then apply an infallible tree update.
  Merge only contiguous areas with identical flags/page size/backing identity
  and contiguous offsets. `map_file` and fixed replacement must create their
  final backing directly inside `xvma`.

### R-108 Existing huge-page behavior is silently removed

- **Severity:** HIGH
- **Section:** Goals G-4 / Data Structures / Anonymous Fault
- **Problem:** Frames and VMAs carry `PageSize`, but Anonymous Fault hard-codes
  4 KiB and the Spec does not state the fate of currently accepted 2 MiB/1 GiB
  mmap flags. Rejecting those flags is a Linux-visible regression, not merely an
  implementation detail.
- **Why it matters:** The task claims preservation of existing anonymous and
  supported RISC-V behavior while leaving an incompatible implementation as a
  valid reading of the plan.
- **Recommendation:** Either preserve huge anonymous/shared mappings throughout
  validation, allocation, owned-page indexing, split rules, COW, and tests, or
  explicitly add their removal as an approved PRD non-goal with documented ABI
  behavior. The current preservation goal requires the former.

### R-109 Validation still permits an untested lifetime rewrite

- **Severity:** HIGH
- **Section:** Validation / Acceptance Mapping
- **Problem:** C-6 through C-8 are judgment-only, the correctness rows are manual
  “trace” reviews, and all guest behavioral testing is optional. No automated
  test covers owned-frame rollback/drop, COW after fork/mprotect, shared-window
  offsets, file fault failure, or VMA split correctness. The acceptance table
  maps only Goals, not the Constraints.
- **Why it matters:** VERIFY could complete after compilation and inspection even
  if a page is freed while mapped or private/shared semantics are wrong.
- **Recommendation:** Add deterministic unit tests around a fake/failure-injected
  leaf mapper or extracted pure transition helpers, plus mandatory first-party
  RISC-V tests for anonymous fault, fork+COW, mprotect-after-fork, shared memory,
  file EOF/failure, and prefix/middle unmap. Map every constraint to executable
  evidence. If runtime prerequisites are unavailable, verification must remain
  incomplete rather than treating manual trace as equivalent.

---

## PFN Page Model Follow-up

### R-201 managed/borrowed ownership could be forged through safe APIs

- **Severity:** HIGH
- **Location:** `xcore/xmm/src/aspace.rs`, `xcore/xmm/src/page.rs`
- **Problem:** A PFN-wide nonzero map count could not prove that the leaf being
  removed was itself managed. A borrowed alias to a managed PFN could therefore
  consume another PTE's reference through `unmap_managed_range`.
- **Resolution:** FIXED — every managed leaf now carries an architecture
  software-reserved `MappingFlags::MANAGED` PTE bit. Managed map/replace set it,
  borrowed map clears it, protection preserves it, and unmap/resident/COW paths
  validate the exact leaf marker before touching `PageMeta`. A round-trip test
  covers managed and borrowed markers, including high-half leaf walking.

## Trade-off Advice

### TR-1 Keep synchronous fault resolution for this task

- **Related Plan Item:** T-3
- **Topic:** Concurrency vs Simplicity
- **Reviewer Position:** Prefer coarse serialized correctness
- **Advice:** It is acceptable for `AddressSpace` to remain behind one sleeping
  mutex and resolve synchronous file reads while held in this iteration.
- **Rationale:** This removes the need for generation revalidation now. The
  `VmObject` contract must forbid reentry into the same address space, and the
  lock-free/drop-lock model can be designed separately.
- **Required Action:** Keep with clarification

### TR-2 Hide reference-count arithmetic from xvma

- **Related Plan Item:** T-1, T-3
- **Topic:** Abstraction vs Correctness
- **Reviewer Position:** Prefer a mechanism query over raw `strong_count`
- **Advice:** Let `xmm` determine whether a resident frame has external owners,
  accounting for its registry and temporary lookup clone.
- **Rationale:** Exposing Arc counts makes COW correctness depend on incidental
  handles and future instrumentation.
- **Required Action:** Adopt

### TR-3 Preserve the current limited shared-file behavior explicitly

- **Related Plan Item:** T-4
- **Topic:** Compatibility vs Scope
- **Reviewer Position:** Prefer narrow compatibility
- **Advice:** Preserve shared visibility for the frame set associated with one
  mapping and its forks, while documenting no independent-mmap coherence or
  writeback until `xcache::FileMapping`.
- **Rationale:** This avoids both a regression and accidental page-cache scope.
- **Required Action:** Adopt

### TR-4 Gate the local-only TLB model to uniprocessor execution

- **Related Plan Item:** T-5
- **Topic:** Scope vs Safety
- **Reviewer Position:** Need explicit enforcement
- **Advice:** Until remote shootdown exists, require `SMP=1` for user address
  spaces or otherwise prevent frame reuse after a merely local invalidation.
- **Rationale:** Deferring shootdown cannot make stale remote writable mappings
  or frame reuse safe; the supported configuration boundary must be explicit.
- **Required Action:** Adopt or expand scope

---

## Minimal Intrusive Ownership Follow-up

### Verdict

- **Decision:** Rejected
- **Blocking:** 3
- **Non-blocking:** 0

The single-count `PageMeta` and managed-PTE transfer protocol are internally
coherent: failed map/remap retains the incoming `PageRef`, successful publication
forgets exactly one handle, and replace/unmap flush before reconstructing and
dropping the removed PTE reference. `MANAGED` also fixes the earlier wrong-kind
unmap problem for leaves created through `AddressSpace`. The remaining blockers
are at the safe borrowed-mapping boundary and in the promoted Spec contract.

### R-301 Safe borrowed mappings do not retain or require target lifetime

- **Severity:** HIGH
- **Location:** `xcore/xmm/src/aspace.rs:106-153`,
  `xmodules/xvma/src/space.rs:80-98`
- **Problem:** `map_borrowed_range` and `VmSpace::map_linear` are safe APIs that
  accept only a raw `PhysAddr`. Safe code can obtain a managed page's physical
  address, install it as a borrowed leaf, drop the last `PageRef`, and then use
  the still-present borrowed PTE through safe address-space read/write paths.
  The `MANAGED` marker correctly prevents the borrowed leaf from consuming a
  managed reference during unmap, but it provides no lifetime for its target.
- **Why it matters:** The page can be returned to `xalloc` and reused while the
  borrowed PTE remains accessible, producing use-after-free and potentially
  aliased mutable access from safe Rust.
- **Recommendation:** Make raw borrowed mapping explicitly `unsafe` with a
  documented target-lifetime and aliasing contract, or require an RAII owner
  token retained by the address space/VMA. Keep convenience safe constructors
  only for types that prove static or otherwise sufficient lifetime.

### R-302 Imported top-level page tables outlive no source owner

- **Severity:** HIGH
- **Location:** `xcore/xmm/src/aspace.rs:75-84`,
  `crates/page_table_multiarch/page_table_multiarch/src/bits64.rs:366-392`
- **Problem:** `copy_mappings_from(&Self)` copies source top-level entries and
  marks them borrowed, but records neither a source lifetime nor shared ownership
  of the referenced lower-level page-table frames. The source can be dropped
  immediately after this safe call; its `PageTable64::drop` then frees those
  frames while the destination root still points to them. The method also does
  not enforce its documentation that imported leaves are borrowed rather than
  `MANAGED`.
- **Why it matters:** Activating the destination can make hardware walk freed or
  reused page-table memory. If a managed/user source is accepted, copied leaves
  additionally outlive the PTE references owned only by the source.
- **Recommendation:** Encode the source lifetime/ownership in the destination,
  deep-copy the imported hierarchy, or restrict this operation to a verified
  immortal kernel address space through an unsafe or dedicated API. Reject any
  imported subtree containing `MANAGED` leaves unless references are explicitly
  cloned and owned by the destination.

### R-303 The promoted Spec still relies on diff-style API descriptions

- **Severity:** HIGH
- **Location:** `PLAN.md:97-106`
- **Problem:** The `## Spec` API Surface says “Keep”, “Add”, and “Remove” and
  refers to “existing ... compatibility entry points” without declaring final
  signatures, error/postconditions, the borrowed lifetime obligation, or the
  source-page-table import lifetime. A promoted feature SPEC therefore cannot be
  implemented or audited without consulting this particular source snapshot.
- **Why it matters:** Ark promotes this section verbatim. A future implementation
  could satisfy the prose while choosing incompatible and unsound ownership
  boundaries, including the two safe borrowed contracts above.
- **Recommendation:** Rewrite the API Surface as the final self-contained
  contract. Define each mapping/import API's safety, ownership transfer,
  rollback, TLB/release postcondition, `MANAGED` preservation, and public error
  behavior without historical “keep/remove” language.

### Trade-off Advice

#### TR-5 Keep `PageMeta` minimal, but do not model borrowing as ownership-free safety

- **Related Plan Item:** G-1, G-2, NG-1
- **Topic:** Minimal metadata versus safe lifetime expression
- **Reviewer Position:** Keep the one-counter design
- **Advice:** Neither blocker requires adding map counts, pins, page flags, or
  VMA policy to `PageMeta`. Resolve borrowed lifetimes at the mapping/import API
  boundary instead.
- **Rationale:** The managed intrusive count is sound when every `MANAGED` PTE
  owns one reference. Borrowed memory has a different proof obligation that a
  PFN ownership counter cannot infer.
- **Required Action:** Adopt

---

## Static Borrowing Resolution Follow-up

### Verdict

- **Decision:** Rejected
- **Blocking:** 1
- **Non-blocking:** 0

R-302 is fixed: the generic hierarchy-copy primitive is unsafe, and the only
safe public wrapper imports the immortal `KERNEL_ASPACE` after rejecting every
`MANAGED` leaf. R-303 is fixed: the promoted Spec now states final low-level
signatures together with the token safety obligation, ownership transfer,
rollback, TLB-before-release, wrong-kind error, and kernel-import contracts.

R-301 is fixed at the `xmm` boundary: raw token construction is unsafe,
`map_borrowed_range` accepts only `StaticPhysicalRange`, validates its extent
and access flags, and `xvma` stores and slices the token. However, the VMA merge
path below invalidates that proof extent, so the end-to-end borrowed-mapping
resolution is not yet sufficient.

### R-401 Borrowed VMA merge extends the VMA but not its physical-range proof

- **Severity:** HIGH
- **Location:** `xmodules/xvma/src/area.rs:81-100`,
  `xmodules/xvma/src/space.rs:470-477`
- **Problem:** `VmArea::can_merge` allows adjacent borrowed areas when their
  physical starts are contiguous, but `merge_adjacent` extends only
  `previous.range`. It retains the left area's original
  `StaticPhysicalRange`, whose `size` still covers only the pre-merge prefix.
  This happens both for separately mapped adjacent static ranges and when
  protection changes split a borrowed area and a later operation merges its
  slices again.
- **Why it matters:** The resulting VMA claims bytes beyond its retained proof.
  A later `checked_slice` of the suffix fails because `subrange` correctly
  rejects the out-of-token extent, while fork maps only `physical.size()` bytes
  but installs the enlarged VMA metadata. This can make `unmap`/`protect`
  unexpectedly fail or leave a child VMA with missing PTEs, and it defeats the
  lifetime/access invariant introduced to resolve R-301.
- **Recommendation:** Do not merge borrowed VMAs unless the backing token is
  merged as part of the same operation. Prefer a checked
  `StaticPhysicalRange::join` that requires contiguous ranges and identical
  access authority, then replace the backing with the joined token before
  extending the VMA. The simpler safe alternative is to make borrowed areas
  non-mergeable. Add a regression test that split/protect/merges a borrowed
  range and then successfully slices and forks its full extent.

### Trade-off Advice

#### TR-6 Prefer non-merging borrowed VMAs until token joining is required

- **Related Plan Item:** G-2, G-5
- **Topic:** VMA normalization versus proof preservation
- **Reviewer Position:** Prefer the smallest sound fix
- **Advice:** Return `false` for borrowed pairs in `can_merge` for this task;
  introduce token joining only if VMA-count pressure is demonstrated.
- **Rationale:** Borrowed mappings are few and static in current call sites, so
  retaining one VMA per proof avoids expanding the token API and makes the
  invariant mechanically obvious.
- **Required Action:** Adopt or implement checked token joining

---

## Final Borrowed-VMA Follow-up

### Verdict

- **Decision:** Approved
- **Blocking:** 0
- **Non-blocking:** 0

R-401 is fixed. `VmArea::can_merge` now rejects every borrowed/borrowed pair,
so `merge_adjacent` cannot extend a VMA beyond the exact
`StaticPhysicalRange` retained in its backing. The regression test constructs
physically contiguous subrange tokens and confirms that they remain separate.
The promoted Spec and MM/xvma documentation state the same proof-boundary rule.

Together with the prior fixes, R-301, R-302, R-303, and R-401 are closed. This
targeted final review found no new HIGH-severity correctness or safety issue.
The conclusion is based on source and contract inspection; test execution
remains part of VERIFY.

---

## Frame Unification Follow-up

### Verdict

- **Decision:** Rejected
- **Blocking:** 1
- **Non-blocking:** 0

The `Frame` unification itself preserves the intrusive ownership protocol.
`try_write_at` requires `&mut Frame` and observes a count of one before writing;
given the counted-handle invariant, no clone or Alloc PTE then exists. Failed
map/remap drops the still-owned input normally, successful publication forgets
exactly one `Frame`, and replace/unmap flush before reconstructing and releasing
the removed PTE reference. Private fork/COW checks exclusivity before taking a
snapshot clone, shared mappings retain object and PTE references separately,
and static fork retains its proof token. `ALLOC_FRAME` is encoded and decoded in
the architecture PTE implementations and is preserved by protection changes.

The remaining blocker is not the rename or count model; it is a safe raw-copy
path that bypasses the new static access proof.

### R-501 Safe address-space copying can write through a read-only static proof

- **Severity:** HIGH
- **Location:** `xcore/xmm/src/aspace.rs:371-419`,
  `xmodules/xvma/src/space.rs:222-228`, `xcore/xmm/src/frame.rs:355-405`
- **Problem:** `AddressSpace::read` and `AddressSpace::write` are safe public
  methods that translate a present PTE and copy through the direct mapping
  without checking its flags or `FrameKind`. `VmSpace` republishes both methods
  as safe public APIs. Consequently safe code can create a read-only
  `StaticFrameRange` with `from_static_bytes`, map it read-only, and call
  `VmSpace::write` to modify the immutable static backing. The same bypass lets
  reads or writes access a static/MMIO range beyond the access authority proved
  by its token. The normal `UserSpaceAccess` adapter validates permissions, but
  that external convention does not make the lower public APIs safe; existing
  ELF/shared-snapshot initialization calls them directly.
- **Why it matters:** `StaticFrameRange` claims to make safe static mappings
  conditional on lifetime and alias/access compatibility. Mutating storage
  behind an immutable `'static` slice can violate Rust's aliasing contract, and
  unrestricted MMIO access can violate the device contract. It also means the
  count-of-one rationale for exclusive frame writes is not a complete safe API
  invariant while the direct-map copy path can access the same physical frame
  independently of `FrameMeta`.
- **Recommendation:** Remove unrestricted safe copying from the public
  `AddressSpace`/`VmSpace` surface. A safe user-copy API must validate the VMA
  permission and reject a Static leaf whose token did not prove the requested
  access. Keep loader/snapshot initialization separate: initialize a unique
  `Frame` before publication, add a narrowly checked Alloc-only initialization
  operation, or make any genuinely privileged raw copy unsafe with an explicit
  alias/device contract. Add a regression test showing that safe write-through
  of a read-only static mapping is rejected. Include the final read/write
  contract in PLAN's API Surface; it is currently omitted despite being public
  and safety-relevant.

### Trade-off Advice

#### TR-7 Separate user copying from privileged image initialization

- **Related Plan Item:** G-2, G-4, G-5
- **Topic:** Safe API clarity versus loader convenience
- **Reviewer Position:** Prefer two purpose-specific paths
- **Advice:** Keep user-copy permission-checked and safe. Build ELF and eager
  shared contents in owned `Frame`s before installing PTEs rather than retaining
  one general safe physical-copy escape hatch.
- **Rationale:** This makes `Frame::try_write_at` and `StaticFrameRange` the two
  explicit mutation/lifetime authorities and avoids reintroducing implicit raw
  physical access into the safe `xvma` policy layer.
- **Required Action:** Adopt or document an equally strong checked boundary

---

## Checked Byte-Copy Resolution Follow-up

### Verdict

- **Decision:** Rejected
- **Blocking:** 1
- **Non-blocking:** 0

R-501 is fixed. `VmSpace::read_bytes` and `write_bytes` enforce the VMA's
logical permission, `AddressSpace::read_bytes` checks READ and rejects DEVICE
leaves, and `write_alloc_bytes` requires both WRITE and `FrameKind::Alloc`.
Therefore a safe caller can no longer mutate a read-only `StaticFrameRange` or
use the ordinary byte-copy path for MMIO. `XUserSpace` uses these checked
operations, and the temporary ELF/shared-snapshot mappings now include
READ|WRITE before restoring their final permissions. PLAN and MM documentation
state the same contract.

One independent PTE-validity blocker is exposed by restoring an empty final
permission set.

### R-502 `PROT_NONE` turns an owned RISC-V leaf into a page-table pointer

- **Severity:** HIGH
- **Location:** `xcore/xmm/src/aspace.rs:291-318`,
  `crates/page_table_multiarch/page_table_entry/src/arch/riscv.rs:60-81,118-121`,
  `xkernel/src/syscall/mm/mmap.rs:156-179`
- **Problem:** Protection preserves `ALLOC_FRAME` even when the requested access
  flags are empty. On RISC-V, converting `MappingFlags::ALLOC_FRAME` produces a
  valid PTE with no R/W/X bits. Such an entry is architecturally and locally
  classified as a non-leaf page-table pointer (`is_huge` checks R|X), not as a
  protected leaf. In debug builds `set_flags` rejects it; in release builds
  page-table query/walk can descend into the mapped data frame as though it were
  a page table. The R-501 shared-file initialization path reaches this directly
  when a file is mapped with `PROT_NONE`: it creates temporary READ|WRITE Alloc
  leaves and then restores empty final flags.
- **Why it matters:** The address-space hierarchy becomes malformed, and the
  PTE-owned `Frame` reference can no longer be found and released by normal
  Alloc unmap. This can cause invalid page-table memory accesses, reference
  leaks, and the address-space teardown assertion to fire. The same condition
  applies when `mprotect(PROT_NONE)` targets any resident Alloc page.
- **Recommendation:** Define an explicit non-present owned-page policy rather
  than encoding `ALLOC_FRAME` as a present RISC-V entry with no leaf permission.
  For this iteration, either retain resident ownership outside a hardware leaf
  while protected, or unmap/drop resident private pages and provide a correct
  rematerialization path (including Shared mappings) when permissions return.
  Do not merely force READ into the final mapping, because that violates Linux
  `PROT_NONE`. Add RISC-V tests for populated anonymous, private, and shared
  mappings across `mprotect(PROT_NONE)` and restoration, plus the file-backed
  shared snapshot path. Add this non-present ownership state to PLAN's Spec.

### Trade-off Advice

#### TR-8 Treat non-present protection as policy state, not a leaf permission

- **Related Plan Item:** G-3, G-5
- **Topic:** Linux protection semantics versus minimal PTE metadata
- **Reviewer Position:** Preserve `PROT_NONE` without malformed leaves
- **Advice:** Keep the VMA as the authoritative requested permission and choose
  an explicit resident-frame strategy for a non-present hardware mapping.
- **Rationale:** RISC-V has no valid present leaf with zero R/W/X permissions;
  the ownership model must not depend on representing one.
- **Required Action:** Adopt

---

## Non-present Ownership Resolution Follow-up

### Verdict

- **Decision:** Rejected
- **Blocking:** 1
- **Non-blocking:** 0

R-502 is fixed at the `AddressSpace` boundary. A no-access Alloc mapping owns
its `Frame` in `inactive_frames` and installs no malformed leaf. Active to
inactive transfer removes the PTE, flushes its TLB entry, then reconstructs the
same reference. Activation retains the inactive owner while cloning and mapping
all leaves; on failure it removes and releases only the installed clones, and
on success it drops the inactive owners. Unmap, frame queries, sparse fork
enumeration, COW exclusivity, and teardown account for both states. Static
no-access mappings retain only their VMA proof token and restore through checked
`StaticFrameRange` mapping. The RISC-V encoder now rejects invalid leaves in
release builds as well, and syscall WRITE protection is normalized to
READ|WRITE.

The remaining blocker is the transaction boundary above these individually
sound operations.

### R-601 Failed `VmSpace::protect` can leave PTE state inconsistent with VMA policy

- **Severity:** HIGH
- **Location:** `xmodules/xvma/src/space.rs:288-385`,
  `xcore/xmm/src/aspace.rs:371-471,485-517`
- **Problem:** `VmSpace::protect` mutates each affected area, and private
  writable ranges even mutate one page at a time, but delays all VMA splitting
  and flag updates until every operation succeeds. A later operation can still
  fail: restoring an inactive Alloc range can return OOM, and a later Static
  range can reject permissions not authorized by its proof token. Earlier
  active/inactive transitions or permission changes are not rolled back. The
  function then returns an error while `self.areas` still carries every old
  permission.
- **Why it matters:** `VmSpace` is specified as the authoritative policy owner,
  yet after a failed protection call the hardware can grant access that the VMA
  denies, or a VMA can claim access while its PTE has been parked. For example,
  restoring two adjacent `PROT_NONE` private areas can activate the first and
  fail allocating a page-table frame for the second; the first remains
  user-accessible even though the syscall failed and its VMA still says
  no-access. Conversely, a partial transition to `PROT_NONE` can make a later
  fault fail despite the unchanged accessible VMA. The per-page private WRITE
  branch permits the same split within one VMA.
- **Recommendation:** Make the full `VmSpace::protect` operation transactional,
  not only each `AddressSpace::protect_alloc_range` call. Preflight every
  affected backing and permission proof before mutation, and either stage all
  fallible inactive/static activations before committing non-fallible changes,
  or keep a rollback journal that restores both hardware/inactive state and VMA
  metadata on any error. Do not call the fallible activation API once per page;
  batch private subranges so its existing all-or-nothing rollback applies.
  Add failure-injection tests spanning multiple VMAs and multiple inactive
  private pages, asserting identical VMA flags, PTE presence/flags, frame
  counts, and inactive entries before and after a failed operation. State this
  cross-VMA error contract in PLAN's Spec.

### Trade-off Advice

#### TR-9 Prefer prepare/commit for protection changes

- **Related Plan Item:** G-3, G-5
- **Topic:** Transactionality versus implementation size
- **Reviewer Position:** Stage fallible work before publishing policy changes
- **Advice:** Represent each affected slice as a prepared transition, perform
  token validation and required page-table allocation first, then commit PTE
  and VMA changes in one non-fallible pass.
- **Rationale:** Per-operation rollback inside `AddressSpace` cannot preserve the
  higher-level invariant when `VmSpace` composes several operations.
- **Required Action:** Adopt or provide equivalent full-operation rollback

---

## Protection Transaction Resolution Follow-up

### Verdict

- **Decision:** Approved
- **Blocking:** 0
- **Non-blocking:** 0

R-601 is fixed. `VmSpace::protect` snapshots effective Alloc flags for every
resident active or inactive page and records each Static slice's proof, flags,
page size, and presence state before applying changes. Any apply error restores
those snapshots before returning; VMA splitting and logical flag publication
occur only after the complete apply succeeds.

The rollback's no-allocation assumption is valid for the current page-table
implementation. `PageTable64::unmap` clears only the leaf and deliberately
retains intermediate page-table frames until `PageTable64::drop`. Therefore an
originally active Alloc or Static mapping can be reinstalled into its existing
hierarchy without allocation. An initially inactive `PROT_NONE` mapping may
allocate hierarchy during attempted activation, but rollback only removes the
new leaf; the newly retained hierarchy does not require allocation to restore
the inactive state.

The Alloc transitions preserve exactly one mapping owner throughout rollback:
active-to-inactive reconstructs the PTE reference after TLB invalidation, while
inactive-to-active installs a clone and drops the inactive owner only after
success. Restoring the snapshotted flags reverses either state without changing
`alloc_mapping_count`. Holes are absent from the snapshot and remain holes.
Private WRITE/COW restoration uses the effective per-page flags, so pages that
were read-only for COW and pages that were initially inactive return to their
distinct prior states. Shared mappings use the same counted transition without
COW. Static rollback uses the original proof token and correctly distinguishes
initially present from initially absent ranges.

PLAN constraint C-13 and the MM/xvma documentation describe this full-operation
transaction boundary. This targeted review found no new HIGH-severity safety or
correctness issue. Execution of failure-injection and guest tests remains part
of VERIFY.

---

## Reserved Inactive-Storage Follow-up

### Verdict

- **Decision:** Rejected
- **Blocking:** 1
- **Non-blocking:** 1

The preceding approval is superseded. The verifier correctly identified that
the former `BTreeMap` could allocate a node when rollback reinserted an inactive
owner; retaining page-table hierarchy alone was not sufficient.

The replacement inactive container's own invariants are sound. It is kept
sorted by binary-search insertion, both insertion sites reserve capacity before
any relevant PTE mutation, `insert_inactive_reserved` asserts spare capacity,
and `Vec::remove` preserves that capacity. For one protection request the target
access direction is uniform: activation only removes inactive entries, while
transition to no-access only adds entries. Thus rollback cannot transiently
need more inactive slots than were present or reserved. Binary-search lookup,
range collection, unmap, frame queries, sparse fork enumeration, and teardown
all account for the sorted vector without changing frame ownership counts.

However, the rollback call graph still performs other heap allocations, so the
claimed failure-atomic OOM recovery remains incomplete.

### R-701 Protection rollback still allocates temporary vectors

- **Severity:** HIGH
- **Location:** `xmodules/xvma/src/space.rs:439-479`,
  `xcore/xmm/src/aspace.rs:312-347,349-389,406-499,548-564`
- **Problem:** `rollback_protection` calls `protect_alloc_page`,
  `static_range_is_present`, and sometimes `map_static_range`. Those operations
  allocate fresh scratch vectors: `preflight_present_leaves` pushes into a new
  leaf vector, `inactive_addresses` collects another vector, Alloc activation
  grows `activated`, and Static mapping grows `installed`. None of this scratch
  capacity is reserved during the snapshot phase or passed into rollback.
  Replacing `inactive_frames` with a capacity-retaining `Vec` removes only the
  persistent-container insertion allocation.
- **Why it matters:** A normal trigger for apply failure is inability to
  allocate a page-table frame. Rollback then runs under the same memory
  pressure, but an implicit `Vec` growth uses the kernel allocator and follows
  the allocation-error/panic path rather than returning a recoverable error.
  The kernel can therefore abort midway through `mprotect` instead of restoring
  the snapshotted VMA/PTE/Frame state, violating C-13. Even a one-page Alloc
  rollback can allocate once while collecting the present/inactive page and
  again while recording activation.
- **Recommendation:** Provide a genuinely allocation-free rollback path. Use
  the already allocated protection snapshot as scratch and add single-page
  restore primitives that query/protect/unmap/remap directly without building
  vectors. For Static rollback, pre-reserve all required scratch before apply or
  restore leaves directly into the retained hierarchy without an `installed`
  vector. Add allocator failure injection that rejects every allocation after
  apply begins and proves rollback completes with identical VMA flags, PTEs,
  inactive entries, and reference counts.

### R-702 MM documentation still describes the removed `BTreeMap`

- **Severity:** MEDIUM
- **Location:** `docs/StarryX/mm.md:138-155`
- **Problem:** The architecture snippet and prose still declare
  `inactive_frames: BTreeMap<...>` and discuss avoiding BTreeMap lookups, while
  the implementation now relies on a sorted capacity-retaining `Vec` for its
  rollback guarantee.
- **Recommendation:** Update the type and document the sorted-order,
  reserve-before-mutation, and no-shrink capacity invariants.

### Trade-off Advice

#### TR-10 Reserve rollback scratch, not only persistent storage

- **Related Plan Item:** C-13
- **Topic:** Failure atomicity versus helper reuse
- **Reviewer Position:** Keep rollback non-allocating by construction
- **Advice:** Do not reuse convenience range helpers whose internal vectors are
  invisible to the transaction. Either pass preallocated scratch through those
  helpers or use dedicated no-allocation restore operations.
- **Rationale:** Recovering from allocator failure cannot depend on further
  successful allocations, even when the persistent owner container has spare
  capacity.
- **Required Action:** Adopt

---

## Allocation-free Restore Resolution Follow-up

### Verdict

- **Decision:** Approved
- **Blocking:** 0
- **Non-blocking:** 0

R-701 is fixed. `rollback_protection` now walks its already allocated Alloc
snapshot directly and uses `PageIterWrapper` for Static ranges; neither path
constructs a temporary collection. `restore_alloc_page` performs only a leaf
query, protect/map/unmap, one intrusive `Frame` clone or ownership transfer,
and a capacity-preserving inactive-vector remove/insert. Page-table unmap keeps
intermediate tables, so remapping the same snapshotted leaf cannot require a
new table frame. Every rollback insertion corresponds either to capacity
reserved before an active-to-inactive mutation or to a slot removed by the
failed activation; `Vec::remove` does not shrink that capacity.

Alloc ownership remains exact in both directions. Restoring an active page
maps a clone before transferring it into the PTE and only then drops the old
inactive owner. Restoring an inactive page removes the PTE, flushes its TLB
entry, reconstructs that PTE's one `Frame` owner, and inserts it into reserved
storage. Neither transition changes `alloc_mapping_count`.

`restore_static_page` is likewise allocation-free for the reachable rollback
states. An originally present leaf is protected in place or remapped into its
retained hierarchy; an originally absent leaf is removed if apply installed
it. Each protect/map/unmap result is flushed, Static mappings never touch
`FrameMeta`, and `PageIterWrapper` restores 4 KiB, 2 MiB, and 1 GiB leaves using
the snapshotted page size and physical offset.

The apply-side journals are also made infallible before publication:
`map_static_range` reserves its complete installed-leaf journal,
`protect_alloc_range` reserves both its activation journal and worst-case
inactive insertion capacity before the first PTE mutation. R-702 is fixed by
describing `inactive_frames` as a sorted `Vec` in PLAN and MM documentation.

The final snapshot also closes the VMA commit-side allocation gap.
`VmSpace::protect` builds and merges the complete `committed_areas` map before
the first hardware mutation; the successful commit is only a map assignment.
Its affected-area and protection-snapshot vectors reserve their complete upper
bounds up front, while page-table preflight, inactive-address, installed, and
activated scratch vectors use fallible `try_reserve`. A scratch allocation
failure after an earlier area was changed therefore reaches the allocation-free
rollback path, and a successful hardware apply cannot subsequently fail while
publishing the VMA policy.

This final targeted review found no new HIGH-severity correctness or safety
issue. Allocation-failure injection and guest behavior remain VERIFY evidence,
not a blocker discovered by this source audit.

---

## PTE-resident PROT_NONE Security Follow-up

### Verdict

- **Decision:** Rejected
- **Blocking:** 2
- **Non-blocking:** 1

The preceding approval is superseded by this PTE-resident `PROT_NONE` review.
The RISC-V and AArch64 software-bit encodings are internally coherent, and the
LoongArch manual defines the selected low bits as ignored by page walking. The
new logical-resident contract is also closed for query, protect, unmap, huge
leaf recognition, and page-table teardown on architectures whose directory
entries satisfy `is_present`. Alloc leaves retain exactly one PTE owner across
protection changes; replacement and unmap still flush before releasing that
owner. `StaticFrameRange::allows` removes the internal `PROT_NONE` marker before
checking the requested access set, so the marker cannot grant WRITE, DEVICE, or
UNCACHED access outside the proof. `VmSpace::protect` prepares its VMA commit
and snapshots before mutation, while rollback uses only single-leaf
query/protect/flush operations and does not allocate. `InactiveFrame` and its
side-map helpers are absent from the implementation.

Two architecture-specific blockers remain.

### R-801 LoongArch leaf walking skips every ordinary page-table subtree

- **Severity:** HIGH
- **Location:** `crates/page_table_multiarch/page_table_entry/src/arch/loongarch64.rs:167-193`,
  `crates/page_table_multiarch/page_table_multiarch/src/bits64.rs:557-563,592-618`,
  `crates/page_table_multiarch/page_table_multiarch/tests/alloc_tests.rs:279-288`
- **Problem:** `LA64PTE::new_table` stores only the aligned next-table physical
  address, while `LA64PTE::is_present` recognizes only leaf `P` or software
  `PROT_NONE`. Consequently every ordinary LoongArch directory entry reports
  false to both generic walkers. `walk_leaf_range` skips the subtree before it
  can reach any normal or `PROT_NONE` leaf. The LoongArch test calls only the
  direct query/protect `run_prot_none_test`; it does not call
  `run_leaf_range_test`, so the defect is hidden.
- **Why it matters:** `AddressSpace::preflight_leaves` sees an empty address
  space on LoongArch. Alloc unmap then releases no PTE owner and does not
  decrement `alloc_mapping_count`, protection and fork omit resident pages,
  teardown panics, and `copy_static_mappings_from` cannot enforce its rejection
  of Alloc leaves before borrowing a hierarchy.
- **Recommendation:** Do not use leaf-resident `is_present` to decide whether a
  non-leaf directory entry exists. Either add an explicit table/entry-exists
  predicate to `GenericPTE`, or make both walkers use `is_unused` at non-leaf
  levels and `is_present` only when classifying a leaf, while preserving huge
  leaf handling. Run `run_leaf_range_test` for LoongArch and add a
  `PROT_NONE`-leaf walk/unmap/drop regression on that architecture.

### R-802 x86 PROT_NONE exposes the real PFN to L1TF

- **Severity:** HIGH
- **Location:** `crates/page_table_multiarch/page_table_entry/src/arch/x86_64.rs:45-54,84-132`
- **Problem:** The x86 encoder clears `PRESENT` for `PROT_NONE` but leaves the
  real physical frame number in bits 12..51. For huge leaves it also retains
  `HUGE_PAGE`. Intel documents that a terminal fault on a non-present entry can
  speculatively use exactly those PFN bits to read a matching L1D line; its OS
  guidance explicitly includes user-requested disabled access and recommends
  encoding an address that cannot refer to cacheable secret memory. See
  <https://www.intel.com/content/www/us/en/developer/articles/technical/software-security-guidance/technical-documentation/intel-analysis-l1-terminal-fault.html>.
- **Why it matters:** On affected Intel processors, an untrusted task can place
  one of its resident frames into `PROT_NONE` and use the terminal-fault side
  channel to infer the referenced physical contents. Retaining `HUGE_PAGE`
  increases the potentially selected physical range. The functional round-trip
  unit test cannot detect this microarchitectural confidentiality failure.
- **Recommendation:** Use an x86-specific mitigated non-present encoding: poison
  or invert the stored PFN into a guaranteed non-cacheable/nonexistent range and
  decode it in `paddr`, and do not leave an unsafe huge-page interpretation.
  If preserving huge size needs another software bit or side metadata, prefer
  that over the one-bit-uniformity goal. Until such an encoding exists, gate the
  x86 `PROT_NONE` backend as unsupported rather than claiming four-architecture
  safety. Add raw-entry tests for poisoned PFNs and 4 KiB/2 MiB restoration.

### R-803 Private writable mprotect walks the same PTE twice

- **Severity:** LOW
- **Location:** `xmodules/xvma/src/space.rs:404-417`,
  `xcore/xmm/src/aspace.rs:492-510`
- **Problem:** Each resident private page is first queried by
  `has_alloc_mapping` and immediately queried again by `frame_is_exclusive`.
  There is no mutation between the calls, and `has_alloc_mapping` has no other
  caller.
- **Recommendation:** Remove the redundant API. Let the exclusivity query
  return `Ok(None)` for a hole, or handle its existing not-mapped result as the
  loop's `continue` case, while retaining `BadState` for a wrong-kind leaf.
  This is a low-risk cleanup and not a correctness blocker.

### Trade-off Advice

#### TR-11 Prefer architecture-safe non-present encodings over uniform bits

- **Related Plan Item:** Implementation 11, C-12
- **Topic:** Uniformity versus hardware security
- **Reviewer Position:** Permit architecture-specific representation
- **Advice:** Keep the generic logical-resident contract, but allow each PTE
  backend to encode and recover `PROT_NONE` PFNs and page sizes differently.
- **Rationale:** Software-reserved flag bits are portable as metadata; the
  security properties of a non-present PFN are not portable, as x86 L1TF
  demonstrates.
- **Required Action:** Adopt

---

## PTE-resident PROT_NONE Fix Verification Follow-up

### Verdict

- **Decision:** Approved
- **Blocking:** 0
- **Non-blocking:** 1

R-801 is fixed. `LA64PTE::is_present` now treats every nonzero entry as a
logical resident entry, which includes LoongArch directory entries created by
`new_table` as well as normal and `PROT_NONE` leaves. Generic query and mutable
lookup still distinguish tables through traversal level and `is_huge`, while
`walk` and `walk_leaf_range` no longer discard the directory subtree. The
LoongArch target now invokes `run_leaf_range_test`, covering ordinary child
tables, sparse leaves, high-half canonicalization, and table teardown.

R-802 is fixed. The x86 encoder complements every address bit under
`PHYS_ADDR_MASK` whenever `PROT_NONE` is set and transparently complements it
again in `paddr`. `set_paddr` encodes according to the current state, and
`set_flags` first decodes the current logical address before encoding it for
the destination state. Therefore active-to-inactive, inactive-to-active,
inactive remap, query, and unmap retain the same logical PFN. The independent
`HUGE_PAGE` bit is preserved by `set_flags`, so `PageTable64::get_entry{,_mut}`
continues to identify 2 MiB leaves without interpreting their poisoned PFN as
a child table. The raw non-present address is outside the original cacheable
frame while the logical address and page size round-trip.

R-803 is fixed. The private writable protection loop now performs one
`frame_is_exclusive` query, treats only `BadAddress` as a sparse hole, and
propagates wrong-kind or metadata errors. The unused `has_alloc_mapping` API is
gone.

The upper ownership and transaction conclusions remain unchanged. Protection
only rewrites flags and flushes the leaf TLB; it neither reconstructs nor drops
the PTE-owned `Frame`. Fork clones one owner into each child Alloc PTE, COW
replacement flushes before releasing the old owner, and unmap decodes the x86
PFN before clearing and releasing it. `VmSpace::protect` still prepares its VMA
commit and snapshots before mutation, and its rollback uses allocation-free
single-leaf protect/flush operations for both Alloc and Static mappings.
`StaticFrameRange::allows` continues to remove only internal metadata before
checking the actual requested access, so `PROT_NONE` cannot widen its proof.

### R-901 The x86 L1TF regression is observable only through decoded state

- **Severity:** LOW
- **Location:** `crates/page_table_multiarch/page_table_multiarch/tests/alloc_tests.rs:159-225`,
  `crates/page_table_multiarch/page_table_entry/src/arch/x86_64.rs:92-141`
- **Problem:** `run_prot_none_test` verifies query, flags, size, restore, and
  unmap, but every observation goes through `paddr`, which transparently
  decodes the complemented address. The same test would have passed the former
  raw-PFN implementation, and it does not exercise `set_paddr`/`remap` while an
  entry is `PROT_NONE`.
- **Recommendation:** Add an x86-local raw-entry unit test that asserts the
  masked stored address differs from the logical PFN while non-present, then
  covers `set_paddr` or `PageTable64::remap`, active restoration, and both 4 KiB
  and 2 MiB `is_huge`/address round-trips. This is regression hardening, not a
  correctness blocker in the reviewed implementation.

The host page-table integration test passed on AArch64. The x86 and LoongArch
target-gated cases were not executed in this review environment, so their
runtime test status must remain a VERIFY item.

---

## x86 Raw-Encoding Regression Resolution Follow-up

### Verdict

- **Decision:** Approved
- **Blocking:** 0
- **Non-blocking:** 0

R-901 is fixed. The x86-only regression now observes `X64PTE::bits` directly
and proves that a `PROT_NONE` entry stores the complement of the logical PFN
under `PHYS_ADDR_MASK`, while `paddr` returns the original address. It then
changes the logical address through `set_paddr`, verifies that the replacement
is still stored complemented, and clears `PROT_NONE` through `set_flags`,
verifying that the active entry stores and reports the replacement PFN
normally. This would fail against the former raw-PFN implementation and covers
the state-sensitive primitives used by `PageTable64::remap`.

The generic `run_prot_none_test` continues to cover 4 KiB and 2 MiB logical
address, flag, page-size, restore, and unmap behavior. Together the two tests
cover the raw x86 security encoding separately from architecture-neutral huge
leaf traversal, without coupling the generic test to private x86 details. No
new correctness or safety issue was found in this test-only increment.

An x86_64 target compile was attempted but did not reach this test because the
existing `x86_64 0.15.5` dependency implements `Step` methods removed by the
active nightly. That pre-existing toolchain/dependency incompatibility remains
VERIFY environment status; it does not invalidate the source-level resolution
above, and this review does not claim the x86 test was executed.

---

## Authoritative PTE Teardown Guard Follow-up

### Verdict

- **Decision:** Approved
- **Blocking:** 0
- **Non-blocking:** 0

Removing `alloc_mapping_count` leaves no parallel ownership state. Every safe
Alloc install, replacement, protection, and removal path now derives its state
from the leaf PTE and its authoritative `ALLOC_FRAME` software bit. The
`AddressSpace` fields, initialization, map/unmap bookkeeping, and debug output
contain no residual aggregate Alloc count.

The new `Drop` guard scans exactly `AddressSpace::range`. All mapping APIs
validate their virtual range against that same boundary, so every Alloc leaf
owned by the address space is covered. Kernel page-table hierarchy copied into
a user address space is outside the user range, is borrowed at the top level,
and is skipped by page-table destruction; moreover,
`copy_static_mappings_from` preflights the source range and rejects any
`ALLOC_FRAME` leaf before copying. Consequently the guard neither treats the
immortal borrowed hierarchy as owned nor permits an owned Alloc leaf to escape
the scan through the safe API surface.

`walk_leaf_range` is allocation-free and uses each architecture's logical
`is_present` semantics. It therefore visits resident `PROT_NONE` leaves,
including x86 entries whose raw PFN is complemented, and checks their decoded
flags for `ALLOC_FRAME`. Sparse holes and ordinary directory entries do not
produce false owners. After the guard passes, `PageTable64::drop` can release
owned page-table pages while continuing to skip borrowed root entries.

The `expect` and ownership assertion in `Drop` are intentional fail-stop
invariant checks. A walk failure or surviving Alloc leaf must not proceed to
page-table teardown, because doing so would discard the PTE-held ownership
record. The scan itself does not allocate or mutate the table, so it introduces
no cleanup failure path. A panic during an existing unwind can abort, but that
is consistent with the kernel invariant policy and is safer than silently
destroying a table with live Alloc ownership.

PLAN C-16 and `docs/StarryX/mm.md` describe this final PTE-derived contract.
The reported targeted xmm/xvma RISC-V checks, page-table tests, and full RISC-V
build cover the changed guard at the stated validation scope; no new
correctness or safety finding was identified in this increment.

---

## Final Simplification Re-review

### Verdict

- **Decision:** Rejected
- **Blocking:** 2
- **Non-blocking:** 0

The low-level simplifications are otherwise coherent. `AddressSpace` performs
an allocation-free authoritative-leaf preflight before its second unmap or
protect pass; sparse holes are skipped consistently, Alloc leaves release their
PTE-owned `Frame` only after TLB invalidation, and static-map rollback can derive
the installed contiguous prefix from the monotonic page iterator. `Frame` and
`SharedObject` encode their 4-KiB granularity once, and direct `deep_copy` keeps
distinct source and destination owners alive. `find_free_area` intersects the
requested limit with the address-space range and advances past every overlapping
VMA. The merged `XUserSpace` copy implementations also retain one VmSpace lock
across their final permission recheck, population, and byte copy.

Two safe higher-layer paths still violate those otherwise sound boundaries.

### R-1001 Partial huge Static ranges break unmap/protect failure semantics

- **Severity:** HIGH
- **Location:** `xmodules/xvma/src/space.rs:272-286`,
  `xmodules/xvma/src/space.rs:290-455`,
  `xmodules/xvma/src/space.rs:529-547`,
  `xcore/xmm/src/frame.rs:387-396`
- **Problem:** `VmSpace` validates `unmap` and `protect` inputs only at 4-KiB
  granularity. `validate_split_offsets` checks that backing offsets are
  representable, but neither it nor `StaticFrameRange::subrange` requires an
  overlap to be aligned to the Static VMA's actual `page_size`. A request can
  therefore cover a valid earlier VMA and only 4 KiB of a later 2-MiB Static
  VMA. `unmap` removes the earlier VMA's leaves before
  `unmap_static_range` rejects the partial huge range, then returns `Err` while
  the unchanged VMA tree still describes the removed leaves. `protect`
  snapshots the misaligned Static subrange, fails when applying it, and then
  calls `PageIter::new(..., Size2M).expect(...)` during rollback; that rollback
  panics because the snapshot was never 2-MiB aligned.
- **Why it matters:** Ordinary page-aligned `munmap`/`mprotect` input can leave
  VMA policy inconsistent with hardware state or panic the kernel. This is not
  a corruption-only path; the public `map_static` API explicitly supports huge
  page sizes.
- **Recommendation:** Before constructing snapshots or changing any PTE,
  validate every affected overlap against its area's `page_size` (both start
  and size/end). If partial huge-page demotion is out of scope, reject the
  complete operation in this first pass. Keep `unmap` as two VMA passes as
  well—validate/preflight all affected overlaps first, then perform the
  infallible removal pass—so a later area cannot make an earlier removal
  observable on `Err`. Add cross-VMA tests in which a valid 4-KiB area precedes
  a partially covered 2-MiB Static area for both unmap and protect.

### R-1002 Safe typed usercopy accepts invalid Rust values and reads padding

- **Severity:** HIGH
- **Location:** `xmodules/xuspace/src/uspace.rs:56-75`,
  `xmodules/xuspace/src/uspace.rs:92-154`,
  `xkernel/src/syscall/net/sockopt.rs:116-135`
- **Problem:** The public safe `read`, `read_slice`, `read_slice_to`, `write`,
  and `write_slice` methods constrain `T` only with `'static`. Arbitrary user
  bytes are copied into `MaybeUninit<T>` and immediately `assume_init`ed, even
  though `T` may have invalid bit patterns. This is exercised concretely by
  `setsockopt`, which reads attacker-controlled bytes as Rust `bool`; any value
  other than 0 or 1 makes `assume_init::<bool>` undefined behavior. In the
  other direction, converting an arbitrary live `T` or `[T]` into `&[u8]` can
  read uninitialized padding and expose it to userspace. Being C-layout does
  not guarantee either all bit patterns are valid or that padding is
  initialized.
- **Why it matters:** A userspace process can trigger kernel undefined behavior,
  and padded syscall structures can leak kernel stack data. The consolidated
  VmSpace lock prevents mapping TOCTOU, but it cannot make the typed byte
  conversion sound.
- **Recommendation:** Restrict typed copy to an audited, sealed POD contract
  that guarantees all input bit patterns are valid and no padding is read, or
  use explicit per-ABI decoding/encoding. Read socket boolean options as the
  Linux integer ABI type and convert with `value != 0`; do not implement `bool`
  as an input POD type. Keep the byte-oriented locked copy methods as the
  primitive and add invalid-boolean plus padded-structure regression tests.

No additional high-confidence correctness issue or safely removable duplicate
state was found in the reviewed Frame/SharedObject, static-prefix rollback,
free-area search, or disjoint-field-borrow simplifications.

---

## Unified Backend and Protection Transaction Follow-up

### Verdict

- **Decision:** Approved
- **Blocking:** 0
- **Non-blocking:** 1

The unified `VmSpace::map(..., Backend)` path preserves the map error
postcondition. Static mapping rolls back its monotonic installed prefix, Shared
mapping releases every already installed PTE reference on a later map failure,
and partially populated Private mappings are removed through the sparse Alloc
unmap path before their VMA is erased. The persistent `Backing::Shared` retains
the same `Arc<SharedObject>` identity across slicing, merge checks, fork, and
futex lookup; each object Frame and each installed PTE retain separate counted
owners.

`AreaBackend` remains crate-private and is implemented by the closed `Backing`
enum, so the refactor adds neither trait-object lifetime state nor an extension
point that can bypass `StaticFrameRange`. Static map/protect/fork operations
continue to carry the proof token, validate requested access, preserve physical
offsets, and reject partial huge-page slices before any PTE mutation.

`ProtectionTransaction` records the actual decoded physical address, raw
effective flags, Frame kind, and page size of every resident leaf. An error
drops the transaction before the prepared VMA map is committed; reverse-order
restore uses only in-place PTE protection plus TLB invalidation and does not
transfer Frame ownership. Duplicate snapshots would also unwind correctly in
reverse order. Fork builds and owns the complete child first, then protects the
parent through the same transaction; a later reserve or validation failure
restores the parent's actual flags before the unpublished child is dropped.
Private child mappings use the parent's actual PTE flags, while the parent COW
pass removes WRITE according to VMA policy. `PROT_NONE` remains a logically
resident PTE throughout snapshot, fork, rollback, and unmap.

R-1001 is closed by `VmArea::checked_slice`, which now requires every retained
or protected slice to align to the area's real page size. R-1002 is closed by
making `xuspace` byte-oriented and moving typed Linux UAPI conversion into
crate-private field codecs: input bytes are decoded into audited integer/field
representations, output buffers are zero-filled before encoding, and socket
boolean options are read as Linux `i32` values rather than Rust `bool`.

### R-1101 Sparse protection journals reserve for virtual pages, not resident leaves

- **Severity:** MEDIUM
- **Location:** `xcore/xmm/src/aspace.rs:537-565`,
  `xmodules/xvma/src/backend.rs:275-277`,
  `xmodules/xvma/src/backend.rs:307-318`,
  `xmodules/xvma/src/fork.rs:15-20`
- **Problem:** `snapshot_range` calls
  `try_reserve(size / page_size)` before walking the page table, although it
  stores snapshots only for resident leaves. A process can create one very
  large, almost entirely sparse Private VMA cheaply, then make `mprotect` or
  fork request a journal proportional to the full virtual span. For example, a
  sparse 1-TiB VMA asks to reserve roughly 268 million
  `ProtectionSnapshot`s even when it has no PTEs. The failure is safely rolled
  back, but a valid sparse operation can fail or create severe transient memory
  pressure for state that does not exist.
- **Recommendation:** Keep the journal sparse. During the allocation-only
  snapshot pass, reserve one additional slot immediately before each resident
  leaf is pushed (or first count resident leaves with an allocation-free walk
  and reserve exactly that count). No PTE mutation occurs until snapshotting
  succeeds, so allocation failure still causes the existing transaction Drop
  to restore earlier operations without requiring a full-range reservation.
  Add a sparse-range test that verifies journal growth follows leaf count for
  both mprotect and parent-after-fork.

No HIGH-confidence safety or correctness issue was found in the final Backend,
transaction rollback, map cleanup, COW/fork, SharedObject, Static proof, or
public safe API paths reviewed here.

---

## Sparse Protection Journal Resolution Follow-up

### Verdict

- **Decision:** Approved
- **Blocking:** 0
- **Non-blocking:** 0

R-1101 is fixed. `ProtectionTransaction::snapshot_range` now performs an
allocation-free first walk that validates every resident leaf's Frame kind and
page size while counting only actual leaves. It reserves exactly that
additional snapshot count, then performs the second walk that records address,
decoded physical address, original flags, and page size. A completely sparse
range therefore reserves nothing, and mprotect/fork journal memory is
proportional to resident PTEs rather than virtual span.

The two walks are stable under the API's exclusivity model. A transaction owns
the sole `&mut AddressSpace` for its lifetime, and `snapshot_range` exposes no
page-table mutation or alias between passes. Hardware may consume a page table
but cannot add, remove, resize, or change the software ownership kind of a
leaf. Consequently the second walk cannot observe more snapshot slots than the
validated count through the safe API. `PROT_NONE` entries remain logically
present to `walk_leaf_range`, so they participate in both count and capture
without moving their PFN or `ALLOC_FRAME` owner.

Validation failure in the first walk returns before reservation and before any
new journal entry. `try_reserve` OOM likewise leaves the current range absent
from the journal and triggers normal Drop rollback of only earlier transaction
operations. After successful exact reservation, the second walk performs no
PTE mutation; even an invariant-level traversal error would leave only no-op
snapshots of unchanged leaves, which reverse rollback can safely restore along
with earlier changes. No rollback step allocates.

Apply and rollback remain permission-only operations. They preserve physical
address, Frame kind, page size, and counted PTE ownership; every changed or
restored leaf is followed by TLB invalidation. `VmSpace::protect` commits its
prepared VMA map only after the transaction commits, while fork completes the
child before starting the parent transaction and restores the parent's actual
flags if any later snapshot/apply operation fails.

The Backend boundary also remains closed and sound: public `Backend` values are
one-shot construction requests, persistent `Backing` is private, and the
crate-private `AreaBackend` implementation is statically dispatched on that
closed enum. Static paths retain and validate `StaticFrameRange`; Private COW
and SharedObject ownership cannot be extended by an external backend
implementation. No new duplicate resident state or public safe bypass was
introduced.

---

## MM Guest Regression and Exit Cleanup Follow-up

### Verdict

- **Decision:** Request changes
- **Blocking:** 1
- **Non-blocking:** 1

The anonymous/private/shared fault, COW, mixed-VMA transaction, and file EOF
cases exercise observable Linux behavior rather than implementation details.
Their signal choices are correct (`SIGSEGV` for protection faults and `SIGBUS`
for a file page wholly beyond EOF), and the failed mixed-VMA `mprotect` checks
the required `ENOMEM` plus failure atomicity. All seven new programs also pass
the framework's RISC-V static `-O2 -Wall -Wextra -Werror` compile command.

Changing Static protection denial to `XError::PermissionDenied` is correct:
the request is structurally valid but exceeds the `StaticFrameRange` access
proof, it is rejected before any PTE mutation, and the Linux translation is
`EACCES`. Replacing process-exit `IPC_MANAGER.clear()` with
`clear_proc_shm(pid)` also fixes the immediate namespace-wide destruction bug
and runs only when the last thread exits. However, normal fork still fails to
create child SysV attachment records, so the new regression proves only that
the namespace survives child exit, not correct inherited SHM ownership.

### R-1201 Fork inherits the SHM VMA but not the SysV attachment

- **Severity:** HIGH
- **Location:** `xkernel/src/syscall/task/clone.rs:96-157`,
  `xkernel/src/ipc/shm.rs:142-178`,
  `xkernel/src/ipc/shm.rs:236-291`,
  `xkernel/src/syscall/ipc/shm.rs:45-60`,
  `xtest/cases/mm/sysv_shm_fork.c:15-27`
- **Problem:** `VmSpace::try_clone` carries the SharedObject-backed VMA into the
  child, and normal fork shares the IPC namespace, but the clone path never
  inserts `(child_pid, shmid, vaddr)` into `pid_shmid_vaddr` or calls
  `ShmSegment::attach_process(child_pid, range)`. Consequently
  `clear_proc_shm(child_pid)` is a no-op. `shm_nattch` remains one rather than
  two, an inherited `shmdt` in the child returns `EINVAL`, and after
  `IPC_RMID` the manager may remove the segment when the parent detaches even
  while the child still has a live mapping. The current test passes because it
  never asks the child to detach or inspects attachment count; it only catches
  the former global `clear()` behavior.
- **Why it matters:** The new exit cleanup avoids destroying unrelated IPC
  state, but does not implement the matching fork/exit ownership lifecycle
  required by System V SHM or by G-5's preserved SHM behavior.
- **Recommendation:** Add a rollback-safe fork hook that enumerates the
  parent's SHM attachments and registers the child in both manager indices and
  each segment before the child is published; undo registrations if clone
  subsequently fails. Extend the test so the child successfully `shmdt`s its
  inherited mapping (and, where the project ABI permits, verify
  `shm_nattch == 2` before detach), then verify the parent mapping and final
  `IPC_RMID`/detach lifecycle.

### R-1202 The vDSO test does not observe either PROT_NONE or restoration

- **Severity:** MEDIUM
- **Location:** `xtest/cases/mm/vdso_static_protection.c:16-31`
- **Problem:** The test changes the first vDSO page to `PROT_NONE` and restores
  it without accessing that page in between. Its later `clock_gettime` calls
  do not prove the page is accessible or executable: libc may fall back to the
  syscall, and the selected vDSO function may reside on another page. The case
  can therefore pass if Static `PROT_NONE` is a no-op or if restoration leaves
  the first page inaccessible. It also does not prove that a rejected WRITE
  request leaves the prior RX permissions unchanged.
- **Recommendation:** Read and retain the ELF magic before protection; in a
  child, perform a volatile read while `PROT_NONE` and require `SIGSEGV`; after
  restoring RX, directly reread the magic. After the denied WRITE request,
  reread it and use a child volatile write expecting `SIGSEGV`. Keep
  `clock_gettime` only as an additional vDSO smoke check, not as the permission
  oracle.

No additional high-confidence safety or correctness defect was found in the
other six cases, the shared wait helpers, the Static errno conversion, or the
last-thread placement of per-process SHM cleanup.

---

## SysV Inheritance and vDSO Regression Resolution Follow-up

### Verdict

- **Decision:** Request changes
- **Blocking:** 2
- **Non-blocking:** 1

R-1201 is closed for an ordinary fork. After selecting and initializing the
child's IPC resource, `inherit_process` first collects and validates every
parent attachment, then updates segment attachment state, rolls back prior
segment updates on a recoverable error, and publishes the child's bidirectional
index only after all segment updates succeed. The call occurs before the
process, thread, and task are published. Child `shmdt` removes both the child
index entry and segment attachment, so the updated case's successful child
detach plus parent `shm_nattch == 1` checks the missing ownership transfer that
the previous test did not.

R-1202 is closed. The vDSO case now directly reads the ELF header, observes a
`SIGSEGV` from a volatile read under `PROT_NONE`, rereads the header after RX
restoration, and after the denied RW request proves both continued readability
and write denial with another faulting child. `clock_gettime` is now only an
additional smoke check rather than the permission oracle. The reported MM 8/8
and full first-party 16/16 guest runs therefore provide meaningful coverage of
these two repaired paths.

### R-1301 SHM inheritance and shmat acquire the manager and segment locks in opposite order

- **Severity:** HIGH
- **Location:** `xkernel/src/ipc/shm.rs:273-313`,
  `xkernel/src/ipc/shm.rs:434-445`,
  `xkernel/src/syscall/ipc/shm.rs:167-225`
- **Problem:** `inherit_proc_shm` holds the outer `IpcManager` lock and the
  `ShmManager` lock while `inherit_process` acquires each `ShmSegment` lock.
  `sys_shmat` does the reverse: it acquires the segment at line 173 and, while
  retaining that guard through the mapping closure, calls
  `IPC_MANAGER.with_shm` at lines 199-201. A concurrent shmat can therefore
  hold the segment while waiting for the manager, as fork holds the manager
  while waiting for the same segment. `xsync::Mutex` is a sleeping mutex, so
  this is a real circular wait rather than a harmless spin ordering issue.
- **Why it matters:** Concurrent fork and shmat can hang both tasks and prevent
  process creation or IPC progress. The sequential guest cases cannot expose
  the cycle.
- **Recommendation:** Establish one hierarchy for every SHM operation, for
  example `IpcManager -> ShmManager -> ShmSegment -> VmSpace`, and remove the
  nested `with_shm` acquisition from inside the segment guard. Prefer a
  ShmManager attach transaction that publishes the PID index and segment state
  together and rolls them back if VMA installation fails. Audit shmdt,
  process-exit cleanup, and orphan cleanup against the same hierarchy, then add
  a concurrent shmat/fork stress case or a lock-order unit test.

### R-1302 CLONE_NEWIPC copies and shares the parent's IPC objects

- **Severity:** HIGH
- **Location:** `xkernel/src/syscall/task/clone.rs:149-158`,
  `xkernel/src/ipc/util.rs:155-176`,
  `xkernel/src/ipc/util.rs:273-277`,
  `xkernel/src/ipc/shm.rs:204-211`
- **Problem:** The NEWIPC branch initializes the child with
  `IPC_MANAGER.copy_inner()`. `IpcManager::clone` copies all identifiers and
  process indices, while `ShmManager::clone` clones the `Arc<Mutex<ShmSegment>>`
  values. The nominally new namespace therefore starts with every old IPC
  object visible and mutates the same segment objects. The subsequent
  `inherit_proc_shm` adds child attachment state to those shared segments only
  in the copied manager's PID index, so detach, nattch, and RMID state can cross
  the namespace boundary or disagree between its two managers.
- **Why it matters:** `CLONE_NEWIPC` is an isolation boundary. Sharing old
  objects is both Linux-visible incorrectness and a cross-namespace integrity
  failure; the ordinary-fork case does not exercise it.
- **Recommendation:** If NEWIPC is supported, initialize an empty
  `IpcManager::new()` and represent inherited already-mapped SHM ownership
  independently from identifier visibility in the new namespace. If that
  split is out of scope, explicitly reject `CLONE_NEWIPC` rather than creating
  a namespace that appears isolated but shares mutable IPC objects. Add a test
  proving that old shmid/msgid/semid identifiers are not resolvable in the new
  namespace and that child exit cannot mutate the parent's segment metadata.

### R-1303 The revised SysV case no longer exercises exit-time auto-detach

- **Severity:** MEDIUM
- **Location:** `xtest/cases/mm/sysv_shm_fork.c:15-32`
- **Problem:** The child explicitly calls `shmdt` before `_exit`, so
  `clear_proc_shm(child_pid)` finds no remaining child mapping. The case proves
  fork registration and explicit detach, and still catches the old global
  `clear()`, but it cannot distinguish correct exit-time detachment from a
  per-process cleanup no-op for a still-attached child.
- **Recommendation:** Keep the explicit-detach child, then fork a second child
  that exits while attached. Verify `shm_nattch` rises for that fork and returns
  to one after wait, before the parent's final RMID/detach. This covers both
  halves of the fork/exit lifecycle without overfitting internal indices.

Apart from the lock-order and NEWIPC issues above, no new high-confidence
counting, rollback, publication-order, errno, or signal defect was found in the
R-1201/R-1202 repairs.

---

## SHM Locking, NEWIPC Rejection, and Exit Detach Resolution Follow-up

### Verdict

- **Decision:** Approved
- **Blocking:** 0
- **Non-blocking:** 0

R-1301 is closed. `sys_shmat` now enters through one `IPC_MANAGER.with_shm`
scope and acquires `IpcManager -> ShmManager -> ShmSegment -> VmSpace`; it no
longer reacquires the manager while holding the segment. `sys_shmget`, shmdt,
fork inheritance, process-exit cleanup, statistics, and orphan cleanup use the
same manager-before-segment order. `sys_shmctl` releases the manager before
taking its cloned segment handle and never reacquires the manager while that
guard is live. The post-shmdt removal reacquires the manager only after the
first closure, segment guard, and VmSpace guard have all dropped. No remaining
segment-to-manager edge was found in the SHM paths.

The revised shmat publication order is coherent. VMA mapping occurs before
segment attachment and PID-index publication. A map error has the existing
`VmSpace::map` no-residue contract; an attachment error removes the newly
installed VMA before returning, and the PID index is still unpublished. The
attachment error is also unreachable in ordinary concurrent execution after
the same segment guard has validated `is_attached`, but retaining rollback
keeps the error path safe if the invariant changes. Successful publication
updates segment count before installing the matching child/process index while
both manager and segment remain locked.

R-1302 is closed at the stated support boundary. `do_clone` rejects NEWIPC with
`EOPNOTSUPP` before `new_user_task`, PID allocation, parent-TID writes, address
space cloning, namespace construction, or publication. The ordinary process
path therefore always shares the current IPC manager before registering fork
attachments. The misleading `IpcManager`/`ShmManager` Clone implementations and
`copy_inner` path are gone, while the initial user process explicitly receives
`new_inner()`, which constructs an empty manager. The guest's raw NEWIPC clone
assertion directly verifies the advertised rejection instead of accepting a
partially isolated namespace.

R-1303 is closed. The first child still proves inherited explicit `shmdt` and
shared data visibility. The second child leaves the inherited attachment live,
signals readiness, and blocks on a separate release pipe; this makes the
parent's `shm_nattch == 2` observation occur while the child is certainly
attached. After release and wait, `shm_nattch == 1` distinguishes correct
exit-time per-PID detach from a cleanup no-op. Final RMID plus parent detach
then exercises the zero-attachment removal boundary. These are Linux-visible
observations and do not depend on the internal map representation.

The reported guest run `6a81a35d-1d2d5088-e637` passes all eight MM cases. No
new high-confidence lock-order, rollback, namespace, attachment-count,
publication-order, errno, or test-validity issue was found in this resolution.
