# MM Subsystem Redesign PRD

---

[**What**]

Redesign StarryX memory management around a minimal trusted `xmm` mechanism
layer and a single high-level `xvma` address-space owner, then define the
object boundary needed for anonymous, shared, and future file-backed mappings.

[**Why**]

The current process memory state is split between `xmm::AddrSpace` and
`xvma::VmaManager`. Mapping metadata, page-table state, file population, COW,
and fork are therefore coordinated through parallel structures rather than one
transactional owner. `xmm` also mixes trusted page-table and frame mechanisms
with user-space mapping policy, while `xvma` only supplements file-backed
regions. This makes faults non-atomic, complicates lifetime reasoning, and
prevents a coherent page-cache and file-mapping design.

The redesign must preserve the existing `xmm` and `xvma` crate names. `xmm`
becomes the narrow trusted mechanism boundary; high-level address-space, VMA,
fault, COW, and fork policy moves into `xvma`. Every allocator-backed physical
frame has minimal PFN-indexed `FrameMeta` containing only its intrusive
reference count. Every present `FrameKind::Alloc` PTE owns one reference, while
higher-level shared-memory
and future cache objects retain pages by cloning `Frame`. The resulting
boundary must support a later `xcache::FileMapping` without coupling `xmm` or
`xvma` to `xfs` or `xkernel`.

[**Outcome**]

- A process owns exactly one `xvma::VmSpace`; the parallel
  `xmm::AddrSpace` plus `xvma::VmaManager` model is removed.
- `xmm::AddressSpace` represents the real hardware address space and TLB
  lifetime domain; `xvma::VmSpace` represents VMA and fault policy.
- `xvma` owns the complete process VMA tree and delegates low-level page-table
  changes to its private `xmm::AddressSpace`.
- The transitional `MemorySet<Backend>` and universal resident-page
  `BTreeMap` are removed. Ordinary and `PROT_NONE` resident pages are indexed
  only by page tables; one software PTE bit marks a resident mapping whose
  hardware valid/present and access bits are clear.
- `xmm` reserves a contiguous PFN-indexed `FrameMeta` array before initializing
  the allocator. Metadata carries only an atomic reference count and does not
  predeclare page-cache, reclaim, mapping-count, or pinning policy.
- All allocator-backed resident mappings use one RAII `Frame` type. Installing
  a PTE transfers one `Frame` into the mapping; unmap restores that reference
  and releases it only after the TLB flush. Anonymous COW and fork policy remain
  in safe `xvma`.
- Shared pages are ordinary `Frame`s retained by an `xvma::SharedObject`;
  `xmm` no longer contains a VMA-specific `SharedPageSet` abstraction.
- `VmArea` backing is organized by lifetime and fork semantics as static,
  private,
  or shared. Anonymous zero-fill and file input are private page sources rather
  than independent lifetime categories.
- Safe static mappings require a `StaticFrameRange` proof of kernel-long
  lifetime and allowed access. Kernel page-table import is restricted to the
  immortal global kernel hierarchy and rejects `Alloc` leaves.
- Sparse resident enumeration walks populated page-table subtrees over a range
  rather than scanning every virtual page or maintaining a second virtual-key
  tree.
- Fault handling has typed outcomes and one kernel entry point; ordinary
  invalid faults become `SIGSEGV`, while invalid file-backed pages become
  `SIGBUS`.
- The kernel translates Linux ABI and signal policy around `xvma` instead of
  directly coordinating page-table and file-population state.
- The already removed borrowed/raw `xuspace` APIs remain absent. The remaining
  unconstrained typed-copy POD hazard is explicitly deferred to a separate
  safety task and is not presented as completed by this redesign.
- A minimal synchronous, filesystem-independent `VmObject` boundary exists for
  future `xcache::FileMapping` integration. Stable file-page identity,
  write-fault notification, writeback, reclaim, and SMP TLB shootdown are
  explicitly deferred.
- Existing supported RISC-V behavior remains operational, relevant crates pass
  formatting and static checks, and current memory, fork, mmap, shared-memory,
  and test-suite paths are validated as far as the repository environment
  permits.
- User object mappings are intentionally limited to 4-KiB pages in this
  iteration; unsupported huge mmap flags return `EINVAL`. Static kernel
  mappings continue to carry their declared page size.
- User address spaces are supported only with `SMP=1` until remote TLB
  shootdown exists.
- `PROT_NONE` keeps PFN, page size, and Alloc ownership in an invalid hardware
  PTE marked by one architecture software bit. It is never encoded as a valid
  RISC-V PTE without R/W/X, and permission restoration is an in-place PTE
  update.

[**Related Specs**]

No existing feature SPEC governs the MM subsystem. The existing vDSO and xtest
SPECs are consumers or validation infrastructure and are not modified by this
design.

[**SPEC Path**]

kernel/mm/redesign-mm-subsystem
