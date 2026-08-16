# StarryX MM Ownership VERIFY

> Status: INCOMPLETE — no implementation blocker found; deterministic
> later-apply/OOM injection and host xvma unit execution remain unavailable
> Feature: `redesign-mm-subsystem`
> Target Task: `redesign-mm-subsystem`
> Tier: `deep`
> Verified: 2026-08-16

---

## Verdict

**INCOMPLETE (no implementation blocker found).**

The final frozen snapshot passes the supported RISC-V xmm/xvma check, canonical
build, targeted clippy, page-table regression, the focused MM guest profile
(8/8), and the complete first-party cases profile (16/16). Source audit found
no high-confidence ownership, rollback, fork/COW, Static, SharedObject,
`PROT_NONE`, or SysV SHM lifecycle defect. The latest reviewer verdict is
Approved with zero blocking and zero non-blocking findings.

The verdict remains INCOMPLETE because PLAN V-U-5's deterministic
later-page/later-VMA apply or allocation-failure injection does not exist and
host xvma unit tests cannot compile through xhal on macOS AArch64. A fresh
final-snapshot OS-COMP run also reached the 4800-second host deadline while
lmbench was running: its first six suites passed, but it cannot be reported as
a complete final-snapshot OS-COMP pass. These are verification gaps, not
observed MM implementation failures.

## Final Design Verification

### Backend boundary and VMA ownership

- PASS — xvma uses one flat module level with cohesive files: `space.rs` owns
  `VmSpace` lifecycle and address-space operations, `area.rs` owns VMA range
  invariants, `backend.rs` owns closed backing dispatch, `fault.rs` owns fault
  and COW handling, and `object.rs` owns memory-object contracts. The former
  one-method `fork.rs` is absent; `try_clone` lives with `VmSpace` lifecycle.
- PASS — `Backend` is the only public mapping-construction request. Its private
  fields and constructors encode Static, anonymous Private, sourced Private,
  and Shared requests plus one-shot population policy.
- PASS — `Backend::prepare` validates size, alignment, checked source/object
  windows, and converts the request to persistent policy before publication.
  One-shot population policy is not retained in a VMA.
- PASS — persistent `Backing` and `VmArea` are private to xvma. `Backing` has
  exactly the three lifetime/fork classes `Static`, `Private`, and `Shared`.
- PASS — `AreaBackend` is crate-private and implemented only for the closed
  `Backing` enum. External code cannot add a backend that bypasses Static proof,
  COW, object-offset, or cleanup invariants.
- PASS — slicing advances Static proof ranges and Private/Shared object offsets
  with checked arithmetic. Shared merge/futex identity preserves the same
  `Arc<SharedObject>` and contiguous object offset; Static areas never merge.
- PASS — `VmSpace::map` publishes one final backing directly. Static/Shared
  failures remove installed prefixes; failed eager Private population sparsely
  unmaps installed Alloc leaves before removing the VMA.

### ProtectionTransaction and atomic mprotect

- PASS — `protect_range` first uses the authoritative xmm preflight to validate
  Frame kind/page size/Alloc owner and count actual resident leaves, including
  logical `PROT_NONE` leaves. Sufficient journal capacity is reserved before
  the second walk or any PTE mutation; sparse holes use no slots.
- PASS — the second walk records only address, old flags, and fully computed
  new flags. Apply then writes those prepared changes directly instead of
  calling the ordinary AddressSpace protect path and repeating preflight.
- PASS — Private WRITE protection uses one batched
  `protect_alloc_range_with` call. xmm supplies each Alloc leaf's current
  exclusivity to the xvma policy closure, replacing per-page transaction and
  repeated query calls.
- PASS — uncommitted Drop restores journaled changes in reverse order using in-place
  PTE protection and per-leaf TLB invalidation. Rollback transfers no Frame and
  performs no allocation.
- PASS — a later validation/reserve failure rolls back every earlier applied
  range. `VmSpace::protect` prepares the complete merged VMA tree before PTE
  apply and publishes it only after transaction commit.
- PASS — overlap slices use each area's real page size, avoiding partial huge
  Static updates across VMA boundaries.

### PROT_NONE, ownership, and teardown

- PASS — accessible Alloc and `PROT_NONE` PTEs retain the same PFN, page-size
  encoding, `ALLOC_FRAME` marker, and owning reference. Protection never
  transfers, clones, or releases that owner.
- PASS — no-access PTEs are hardware-invalid but logically resident through the
  architecture software bit. RISC-V receives no valid leaf lacking R/X and no
  malformed W-only/V-only leaf; query/walk/unmap still find `PROT_NONE` leaves.
- PASS — successful map transfers its `Frame` only after PTE installation and
  TLB flush. Replace/unmap change or remove the leaf, invalidate the TLB, and
  only then reconstruct/release the old PTE-owned Frame. Static leaves never
  touch allocator metadata.
- PASS — static scans find no `InactiveFrame`, `inactive_frames`,
  `alloc_mapping_count`, old `ManagedPage`/`PageRef`, or `MANAGED` side state.
  PTEs are the sole resident index and owner record.
- PASS — `AddressSpace::Drop` walks the authoritative user range and rejects any
  remaining accessible or `PROT_NONE` Alloc leaf. Imported immortal kernel
  hierarchy lies outside the user range and import rejects Alloc leaves.
- PASS — `AddressSpace::new_user` rejects `SMP != 1`; local TLB invalidation is
  therefore the supported frame-reuse boundary.

### Fork, COW, Static, Shared, and faults

- PASS — fork owns the complete child before parent permission changes. Child
  construction failure uses ordinary teardown; a later parent-protect failure
  rolls back the parent transaction and drops the unpublished child.
- PASS — Private fork includes accessible and `PROT_NONE` residents, clones
  their Frames into the child without WRITE, and removes WRITE from writable
  parent residents through the same protection transaction.
- PASS — mprotect cannot bypass COW: an exclusive Private Frame may regain
  WRITE; a shared resident remains read-only until a write fault upgrades an
  exclusive owner or deep-copies and atomically replaces it.
- PASS — fault dispatch obtains resident flags once through `mapping_flags`.
  It calls `frame_if_shared` only for Private write faults; exclusive leaves
  return no temporary handle, while shared leaves clone exactly the COW source
  required to keep the old page live.
- PASS — Static map/fork/protect revalidates the exact `StaticFrameRange`,
  preserves offsets/page size, enforces allowed permissions with `EACCES`, and
  never modifies allocator refcounts.
- PASS — `xvdso` has no manifest, source, or transitive dependency on `xruntime`
  or `xmm`. It exposes static image/vvar references and an explicit refresh
  operation; `xkernel::vdso` implements the generic runtime timer hook,
  constructs the read-only or read/execute `StaticFrameRange` proof, and
  installs the VMA.
- PASS — Shared mappings retain Frames directly in one `SharedObject`; fork
  preserves shared identity and WRITE semantics, and slices preserve checked
  object offsets.
- PASS — anonymous faults allocate zeroed Frames. Sourced Private faults check
  object length/offset, distinguish Retry/NoMemory/Bus, populate before PTE
  installation, and leave no resident leaf on input error.
- PASS — xvma remains `#![forbid(unsafe_code)]`; raw Frame/PTE lifetime work
  remains in xmm/page-table code.

### SysV SHM process lifecycle and lock order

- PASS — `CLONE_NEWIPC` is rejected with `EOPNOTSUPP` before task/PID creation;
  the former incorrect clone/share path and `IpcManager`/`ShmManager` Clone
  implementations are absent.
- PASS — ordinary fork shares the IPC manager and then explicitly inherits the
  parent's SHM attachments. Partial inheritance rolls back prior segment
  attaches and publishes the child's attachment index only after success.
- PASS — process exit calls `clear_proc_shm`, detaches only that PID, updates
  `nattch`, and reclaims orphaned RMID segments. Explicit `shmdt` and automatic
  exit detach are both exercised by the focused guest test.
- PASS — audited ordering is manager -> segment -> `VmSpace`. `shmat` maps
  before publishing attachment state and unmaps on attachment failure;
  `shmdt` drops prior guards before any orphan cleanup manager reacquire;
  `shmctl` drops the manager guard before locking a segment.

### User-copy boundary

- PASS — xuspace exposes byte copy and pointer/range validation rather than a
  safe unconstrained generic typed read. Typed Linux UAPI conversion is private
  to xkernel behind the finite `UserRead`/`UserWrite` codec set.
- PASS — input structs decode fields through integer/explicit codecs; output
  buffers are zero-filled before encoding, so kernel-object padding is never
  exposed. Linux boolean socket options decode integer ABI values rather than
  Rust `bool` bit patterns.

## Acceptance and Runtime Evidence

- C-1 through C-12: PASS by source audit, supported-target check/build,
  page-table regression, and guest evidence.
- C-13 through C-19: PASS by source audit of transaction atomicity, unchanged
  PTE ownership across `PROT_NONE`, prebuilt VMA commit, exact leaf reservation,
  PTE-authoritative teardown, fixed 4-KiB Alloc/Shared granularity, and
  allocation-free authoritative unmap/teardown preflight. The reduced boundary
  removes `frame_at`, `frame_is_exclusive`, `is_present`, and `allows_access`
  from AddressSpace in favor of cohesive batch/query mechanisms.
- V-U-4 and V-I-3: PASS — focused RISC-V run
  `6a81d6e4-38032838-4e6b` completed 8/8 with QEMU exit 0:
  `file_private_fault`, `fork_cow`, `mmap_anon`, `mprotect_mixed_vmas`,
  `mprotect_prot_none`, `shared_anon_fork`, `sysv_shm_fork`, and
  `vdso_static_protection`.
- PASS — those cases directly exercise PROT_NONE SIGSEGV/restore, private COW,
  shared-anonymous visibility, cross-VMA failure atomicity, file-private faults
  and EOF SIGBUS, SHM fork/explicit detach/exit auto-detach/`nattch`, explicit
  NEWIPC rejection, and static vDSO permission denial/restore.
- PASS — complete first-party cases run `6a81d702-3476e880-510c` completed
  16/16, 0 failed, 0 timed out, QEMU exit 0.
- INCOMPLETE — V-U-5 has no deterministic later-apply/reserve-OOM injection.
  The mixed-VMA hole case proves an ordinary later-range validation failure is
  atomic, but does not substitute for the requested apply/OOM fault injection.
- ENVIRONMENT BLOCKED — host `cargo test -p xvma` stops in xhal's explicit
  unsupported macOS AArch64 target gate before xvma unit tests execute.

## Fresh Commands and Reports

Freshly executed against the final frozen snapshot:

- PASS — `cargo check -p xvma -p xkernel --target
  riscv64gc-unknown-none-elf`.
- PASS — `make ARCH=riscv64 FEATURES=fp_simd build`.
- PASS — `cargo clippy -p xmm -p xvma -p xkernel --target
  riscv64gc-unknown-none-elf --no-deps`; emitted only pre-existing xkernel
  warnings outside the MM/SHM cleanup.
- PASS — `cargo tree -p xvdso --target riscv64gc-unknown-none-elf` and direct
  source/manifest scans contain no `xruntime` or `xmm` dependency.
- PASS — `cargo test --manifest-path
  crates/page_table_multiarch/page_table_multiarch/Cargo.toml --test
  alloc_tests`; macOS-AArch64 available test passed 1/1 and covers sparse
  traversal plus 4-KiB/2-MiB `PROT_NONE` round trips.
- PASS — changed/untracked Rust files passed individual rustfmt checks without
  recursively formatting untouched lwext4 modules.
- PASS — `git diff --check`, removed-side-state scans, and removed-old-IPC-path
  scans.

Fresh final-snapshot OS-COMP attempt:

- INCOMPLETE — run `6a81a4a2-0c4c29b0-f569` ended `host_timeout` at the
  4800-second host deadline. `basic`, `busybox`, `cyclictest`, `iozone`,
  `libcbench`, and `libctest` all completed PASS (6 passed, 0 failed, 0 guest
  timed out). lmbench printed `Simple syscall: 5.0386 microseconds` and then
  made no further serial progress; its case never produced `case_end`.
- PASS — the host runner sent SIGTERM and reaped both its runner and QEMU
  descendants. The final JSON/TAP report records `host_timeout`, `qemu_exit:
  null`, and marks lmbench plus the later Lua/netperf/UnixBench suites not run.
- SUPPLEMENTARY ONLY — earlier OS-COMP run `6a819a5a-202a92c8-9c83` completed
  10/10 on the same MM core, but predates the final SHM lock-order and NEWIPC
  changes. It is not claimed as a final-snapshot full-profile pass.

### iozone regression comparison

The baseline is commit `2cfb6ea12f85a8496abe93d6ba2a6f609ef103e2`.
Baseline and candidate values are medians of three standard 4-MiB automatic
mode runs on the same host/QEMU setup. “Final sample” is the completed iozone
sample from final OS-COMP attempt `6a81a4a2-0c4c29b0-f569`; the later lmbench
host timeout does not invalidate this already-completed case. Values are kB/s.

| Metric | Baseline median | Candidate median | Final sample | vs baseline | vs candidate |
| --- | ---: | ---: | ---: | ---: | ---: |
| write | 11143 | 18266 | 18106 | +62.49% | -0.88% |
| rewrite | 11926 | 18445 | 19056 | +59.79% | +3.31% |
| read | 10790 | 20139 | 19176 | +77.72% | -4.78% |
| reread | 11658 | 20278 | 19528 | +67.51% | -3.70% |
| random read | 10056 | 17124 | 17238 | +71.42% | +0.67% |
| random write | 9824 | 17255 | 16361 | +66.54% | -5.18% |
| backward read | 10165 | 17157 | 18003 | +77.11% | +4.93% |
| record rewrite | 10058 | 16675 | 17068 | +69.70% | +2.36% |
| stride read | 10190 | 17860 | 17084 | +67.65% | -4.34% |
| fwrite | 11120 | 16998 | 17879 | +60.78% | +5.18% |
| frewrite | 10472 | 17027 | 17750 | +69.50% | +4.25% |
| fread | 5810 | 9239 | 9144 | +57.38% | -1.03% |
| freread | 5733 | 9083 | 9162 | +59.81% | +0.87% |

All 13 final-sample metrics remain above the three-run baseline median. Against
the candidate median, the single sample ranges from -5.18% to +5.18%; this is
consistent with ordinary host/QEMU variance and shows no observed storage-I/O
regression. It is benchmark evidence, not a deterministic speedup claim.

## Findings

### V-001 Deterministic later-apply/OOM protection injection is absent

- **Severity:** VERIFICATION GAP
- **Resolution:** INCOMPLETE; explicitly deferred by PLAN V-U-5/V-F-3.
- **Detail:** focused guest coverage proves real PROT_NONE, COW, Static, SHM,
  file-fault, and mixed-VMA validation behavior, but no deterministic hook
  forces a later apply or journal-reserve allocation failure and asserts every
  earlier Alloc/Static PTE plus all VMA flags are unchanged.

### V-002 Host xvma unit execution is unavailable

- **Severity:** ENVIRONMENT GAP
- **Resolution:** ENVIRONMENT BLOCKED.
- **Detail:** xhal intentionally rejects the macOS AArch64 host before xvma
  unit tests compile. Supported RISC-V check/build and real guest cases pass.

### V-003 Final-snapshot full OS-COMP did not complete

- **Severity:** VERIFICATION GAP
- **Resolution:** INCOMPLETE; no MM failure observed.
- **Detail:** final run `6a81a4a2-0c4c29b0-f569` hit the host's 4800-second
  deadline during lmbench after six suites passed. The focused MM and complete
  first-party cases profiles pass on the final snapshot; the older 10/10
  OS-COMP run is retained only as pre-final-IPC supplementary evidence.

### V-004 Former unconstrained typed usercopy finding

- **Severity:** HIGH (historical)
- **Resolution:** FIXED.
- **Detail:** public xuspace typed materialization is gone; private audited
  Linux ABI codecs decode fields and zero-fill output buffers.
