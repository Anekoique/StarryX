# StarryX MM Ownership PLAN

> Status: Revised implementation in progress
> Feature: redesign-mm-subsystem
> Owner: Executor

---

## Summary

StarryX keeps one safe VMA-policy owner and one trusted hardware-address-space
mechanism. Physical memory is described with frame terminology throughout:

```text
XUserSpace -> Mutex<xvma::VmSpace>
                 |-- BTreeMap<VirtAddr, VmArea>
                 `-- xmm::AddressSpace
                       `-- PageTable

xmm allocator-frame lifetime
    PFN -> FrameMeta { ref_count }
              ^
              `-- Frame / Alloc PTE

xvma mapping policy
    VmArea -> Static | Private { source } | Shared { object }
```

The design does not introduce Linux page, folio, rmap, pin, reclaim, or page
flags. `FrameMeta` is only the intrusive reference-count control block needed
to recover an Alloc PTE's hidden `Frame` reference from its physical address.

## Spec

### Goals

- G-1 Keep `FrameMeta` to one allocator-frame lifetime count.
- G-2 Use `Frame` as the only public allocator-frame ownership handle.
- G-3 Make every Alloc PTE own exactly one `Frame` reference.
- G-4 Keep static ranges, shared-memory policy, and VMA policy out of
  `FrameMeta`.
- G-5 Preserve supported mmap, fork, COW, SHM, vDSO, and exec behavior.

### Non-goals

- NG-1 Do not add typed frame metadata, metadata vtables, rmap, pin, reclaim,
  writeback, swap, or SMP TLB shootdown.
- NG-2 Do not implement coherent shared file mappings or a page cache.
- NG-3 Do not add a universal resident-frame `BTreeMap` beside the hardware
  page table. Normal and `PROT_NONE` resident pages remain PTE-owned and
  unindexed elsewhere.

### Architecture

- `xmm::AddressSpace` owns hardware PTE mutation and local TLB invalidation.
- `xmm::Frame` is a counted handle to one allocator-backed 4-KiB frame.
- `xmm::FrameMeta` is its private PFN-indexed control block and contains only
  `ref_count`.
- `xmm::StaticFrameRange` proves kernel-long physical lifetime and maximum
  allowed access for a contiguous static range.
- `xmm::FrameKind::{Alloc, Static}` is an internal PTE teardown classification:
  an Alloc leaf owns one `Frame`; a Static leaf owns none.
- `AddressSpace` stores no derived Alloc-mapping count; teardown checks the
  authoritative `ALLOC_FRAME` leaves directly.
- Each architecture uses one software PTE bit for `PROT_NONE`; the hardware
  valid/present and access bits are clear while page size and Alloc ownership
  remain in the PTE. The mapped physical address remains recoverable; x86
  stores its address bits inverted while non-present to avoid L1TF exposure.
- `xvma::VmSpace` owns VMA layout, fault, fork, COW, and backing policy.
- `xvma::SharedObject` owns a homogeneous `Box<[Frame]>` and supplies clones to
  shared Alloc PTEs.
- Future `xcache` objects may retain `Frame`s and their own cache-specific
  state; they do not extend `FrameMeta`.

### Data Structures

```rust
enum FrameKind {
    Alloc,
    Static,
}

pub struct Frame {
    paddr: PhysAddr,
}

struct FrameMeta {
    ref_count: AtomicU32,
}

pub struct StaticFrameRange {
    start: PhysAddr,
    size: usize,
    allowed_flags: MappingFlags,
}

enum Backing {
    Static { frames: StaticFrameRange },
    Private { source: Option<Arc<dyn VmObject>>, offset: usize },
    Shared { object: Arc<SharedObject>, offset: usize },
}

pub struct Backend {
    backing: Backing,
    page_size: PageSize,
    populate: bool,
}

trait AreaBackend: Clone {
    // Closed static dispatch for slice/merge, map/unmap, protect, fault and fork.
}

pub struct SharedObject {
    frames: Box<[Frame]>,
}
```

`Private { source: None }` is zero-filled anonymous memory. `Private` with a
source is private file-sourced memory; both use the same COW policy. `Shared`
retains one object reference per frame and supplies another reference to every
mapped Alloc PTE. Static PTEs never contribute a reference count.

Callers create every area through `VmSpace::map(..., Backend)`, replacing the
kind-specific `map_alloc`, `map_static`, `map_file`, and `map_shared` surface.
One-shot creation policy is converted into the private persistent `Backing`.
`AreaBackend` is crate-private and implemented by the closed `Backing` enum;
it centralizes VMA behavior without trait objects, per-area allocations, or
moving private/shared policy into `xmm`.

### Why the old page types are removed

- `Page` represented a newly allocated, unpublished frame.
- `PageRef` represented the same physical object after it became cloneable.
- `ManagedPage` was only an address-space query record containing an address,
  `PageRef`, flags, and page size.
- `PageMeta` was not a page object at all; it was the intrusive count control
  block.

`Page` and `PageRef` are collapsed into `Frame`. A newly allocated `Frame`
starts with one reference. `try_write_at(&mut self, ...)` mutates it only when
the count is still one, retaining the old unpublished-write invariant without
exposing two ownership types. `ManagedPage` is deleted: `frame_if_shared`
returns no handle for an exclusive PTE and clones a `Frame` only when it is
shared, while `mapped_frames` returns address/frame/flag tuples. `PageMeta` is
renamed private `FrameMeta` to make the control-block role explicit.

### API Surface

```rust
impl Frame {
    fn allocate_zeroed() -> Option<Frame>;
    fn physical_address(&self) -> PhysAddr;
    fn try_write_at(&mut self, offset: usize, source: &[u8]) -> bool;
    fn deep_copy(&self) -> Option<Frame>;
}

impl Clone for Frame;
impl Drop for Frame;

impl StaticFrameRange {
    unsafe fn new(
        start: PhysAddr,
        size: usize,
        allowed_flags: MappingFlags,
    ) -> XResult<Self>;
    fn from_static_readonly<T: ?Sized + Sync>(value: &'static T) -> XResult<Self>;
    fn from_static_code(code: &'static [u8]) -> XResult<Self>;
    fn subrange(self, offset: usize, size: usize) -> XResult<Self>;
}

impl AddressSpace {
    fn map_frame(&mut self, va: VirtAddr, frame: Frame, flags: MappingFlags)
        -> XResult;
    fn replace_frame(
        &mut self,
        va: VirtAddr,
        expected: &Frame,
        replacement: Frame,
        flags: MappingFlags,
    ) -> XResult;
    fn map_static_range(
        &mut self,
        va: VirtAddr,
        frames: StaticFrameRange,
        flags: MappingFlags,
        page_size: PageSize,
    ) -> XResult;
    fn unmap_alloc_range(&mut self, va: VirtAddr, size: usize) -> XResult;
    fn unmap_static_range(
        &mut self,
        va: VirtAddr,
        size: usize,
        page_size: PageSize,
    ) -> XResult;
    fn begin_protection(&mut self) -> ProtectionTransaction<'_>;
    fn protect_alloc_page(&mut self, va: VirtAddr, flags: MappingFlags) -> XResult;
    fn mapping_flags(&self, va: VirtAddr) -> Option<MappingFlags>;
    fn frame_if_shared(&self, va: VirtAddr) -> XResult<Option<Frame>>;
    fn mapped_frames(
        &self,
        range: VirtAddrRange,
    ) -> XResult<Vec<(VirtAddr, Frame, MappingFlags)>>;
    fn read_bytes(&self, va: VirtAddr, output: &mut [u8]) -> XResult;
    fn write_alloc_bytes(&mut self, va: VirtAddr, input: &[u8]) -> XResult;
}

fn copy_kernel_mappings(destination: &mut AddressSpace) -> XResult;

impl ProtectionTransaction<'_> {
    fn protect_alloc_range(&mut self, va: VirtAddr, size: usize, flags: MappingFlags)
        -> XResult;
    fn protect_alloc_range_with(
        &mut self,
        va: VirtAddr,
        size: usize,
        flags: impl FnMut(bool) -> MappingFlags,
    ) -> XResult;
    fn protect_static_range(
        &mut self,
        va: VirtAddr,
        frames: StaticFrameRange,
        flags: MappingFlags,
        page_size: PageSize,
    ) -> XResult;
    fn commit(self);
}

impl VmSpace {
    fn map(&mut self, va: VirtAddr, size: usize, flags: MappingFlags, backend: Backend)
        -> XResult;
}
```

`StaticFrameRange::new` is unsafe because its caller proves physical lifetime
and alias/access compatibility. Safe static mapping accepts only this proof
token. `copy_kernel_mappings` borrows only the immortal kernel hierarchy and
rejects every Alloc leaf.

`xvdso` exposes only static image/vvar references and an explicit refresh
operation. It does not depend on `xruntime`, `xmm`, or `xvma`, and registers no
kernel callbacks. `xkernel::vdso` implements the generic runtime timer hook,
invokes the refresh operation, and converts the references into read-only or
read/execute `StaticFrameRange` proofs before installing VMAs.

`map_frame` and `replace_frame` retain their incoming `Frame` on every error.
On success an `ALLOC_FRAME` PTE owns that reference. Alloc unmap and replacement
remove or replace the leaf, invalidate its TLB entry, then restore and release
the old reference. Wrong-kind unmap fails before mutation.

`AddressSpace::read_bytes` checks READ on every leaf and rejects device memory;
`write_alloc_bytes` additionally requires WRITE and `FrameKind::Alloc` on every
leaf. `VmSpace::read_bytes`/`write_bytes` first enforce authoritative VMA
permissions. ELF and shared-file snapshot construction temporarily maps Alloc
frames writable, initializes them through this checked path, and restores final
permissions before the address space becomes user-visible. No safe raw-copy API
may bypass `StaticFrameRange` access or alias proofs.

### Constraints

- C-1 `FrameMeta` contains exactly one `AtomicU32 ref_count`.
- C-2 Every `Frame` value and every resident Alloc PTE owns one reference.
- C-3 Alloc PTE removal reconstructs its reference exactly once.
- C-4 The last PTE reference is released only after TLB invalidation.
- C-5 Static PTEs never modify `FrameMeta`.
- C-6 `try_write_at` requires `&mut Frame` and `ref_count == 1` before writing.
- C-7 `xmm` contains no VMA, shared-object, or page-cache policy.
- C-8 `xvma` remains `#![forbid(unsafe_code)]`.
- C-9 Alloc user frames remain 4 KiB and user address spaces require `SMP=1`.
- C-10 Kernel hierarchy import accepts no caller-selected source and rejects
  Alloc leaves.
- C-11 Safe byte-copy APIs enforce both VMA and PTE access flags; writes accept
  Alloc leaves only and ordinary copies reject device memory.
- C-12 A hardware-valid leaf always has architecture-valid access bits.
  `PROT_NONE` clears hardware validity and access while one software bit keeps
  the PTE logically resident; its physical address and `ALLOC_FRAME` ownership
  do not move logically.
- C-13 `VmSpace::protect` is atomic across every affected page and VMA: a
  generic `ProtectionTransaction` journals actual PTE state, applies changes,
  rolls back all earlier changes on failure, and commits VMA flags only after
  complete success.
- C-14 The `PROT_NONE` PTE keeps exactly the same Frame ownership count as its
  accessible form; protect never transfers, clones, or releases that owner.
- C-15 The merged post-protection VMA tree is prepared before apply. Every
  transaction first validates and counts resident leaves, then reserves exactly
  that many journal slots before any PTE mutation; rollback uses allocation-free
  PTE protect and commit only swaps in the prepared tree.
- C-16 PTEs are the sole resident-mapping index. AddressSpace teardown derives
  its no-Alloc-leaf check from `ALLOC_FRAME` rather than cached aggregate state.
- C-17 Alloc Frames and SharedObjects encode their supported 4-KiB granularity
  once in the API rather than storing or passing a repeated page-size value.
- C-18 unmap and VmSpace teardown preflight authoritative leaves without
  allocating a resident-leaf vector; map-static rollback derives its installed
  prefix from iteration order rather than recording a second journal.
- C-19 The xvma/xmm boundary avoids repeated observations of one leaf:
  page-fault dispatch obtains resident flags once, shared-frame cloning occurs
  only when required, and transactional protection computes per-leaf target
  flags inside one batch rather than issuing per-page transaction calls.

## Runtime

### Frame ownership

```text
xalloc frame -> FrameMeta 0 -> 1 -> Frame
Frame::clone -> ref_count + 1
Frame::drop  -> ref_count - 1
ref_count 1 -> 0 -> return frame to xalloc
```

The old `Page::into_ref` state transition no longer exists. Initialization is
performed while the newly allocated `Frame` is the only reference.

### Alloc mapping

```text
map_frame(frame)
    validate vacant leaf
    install an ALLOC_FRAME PTE and transfer the Frame reference
    if no R/W/X: clear hardware validity and set the PROT_NONE software bit

unmap Alloc PTE
    verify FrameKind::Alloc and FrameMeta state
    remove leaf and recover paddr
    flush local TLB
    reconstruct and drop the PTE-owned Frame
```

### No-access mapping

```text
accessible -> clear hardware validity/access + set PROT_NONE -> flush TLB
PROT_NONE -> restore hardware validity/access + clear PROT_NONE -> flush TLB
```

The PTE keeps a recoverable physical address, page-size encoding, and
`ALLOC_FRAME` marker across both transitions. On x86 only, the non-present
entry stores inverted address bits and decodes them for software queries.
`GenericPTE::is_present` reports this software entry as logically resident, so
query, sparse traversal, fork, COW, and unmap use the same path as accessible
mappings while hardware still faults on access.

At the higher layer, `VmSpace::protect` builds the complete post-protection VMA
map, then performs all PTE permission changes through one
`ProtectionTransaction`. One authoritative walk validates and counts resident
leaves, then reserves sufficient journal capacity. A second walk records each
leaf's address, old flags and fully computed target flags; apply follows without
calling the ordinary protect preflight again. Failures in a later page or VMA
trigger reverse-order, allocation-free restore; VMA split/flag changes remain
the final non-failing commit step. The private snapshot type and restore logic
stay in `xmm`, so `xvma` has no separate Alloc/Static rollback APIs or
`StaticProtectionSnapshot`.

`frame_if_shared` checks the Alloc PTE count before cloning. A count of one
returns `None`, allowing xvma to restore write permission directly; otherwise
it returns the one temporary `Frame` needed as the COW copy source.

### Static mapping

Kernel-image, vDSO/vvar, firmware, and MMIO ranges use `StaticFrameRange`.
Their PTEs have `FrameKind::Static`, carry no `ALLOC_FRAME` bit, and never touch
`FrameMeta`. Static VMAs do not merge, so each VMA stays within the exact proof
token carried by its backing.

### Shared object and VMA policy

`SharedObject` retains ordinary `Frame`s. Mapping clones an object frame and
transfers the clone into an Alloc PTE. Fork clones another reference without
COW. Removing one mapping releases only that PTE's reference.

- `Static`: existing kernel-long physical range; copied as static.
- `Private`: zero/source faulting; resident frames use private COW on fork.
- `Shared`: stable object identity; fork preserves writable sharing and futex
  identity.

## Implementation

1. [x] Replace the transitional memory-set/backend model with AddressSpace and
   one xvma VMA owner.
2. [x] Keep one PFN-indexed intrusive reference count and exact PTE ownership
   transfer.
3. [x] Move shared-frame collections and VMA policy out of xmm.
4. [x] Add static lifetime proof tokens and safe kernel hierarchy import.
5. [x] Preserve static proof extent across VMA slicing and fork.
6. [x] Migrate fault, fork, COW, SHM, futex, vDSO, and mmap consumers.
7. [x] Collapse `Page`/`PageRef` into `Frame` and remove `ManagedPage`.
8. [x] Introduce `FrameKind::{Alloc, Static}` and rename the PTE software marker
   to `ALLOC_FRAME` across all architecture encoders.
9. [x] Rename the old externally-backed VMA variant to `Static` and shared storage to
   `Box<[Frame]>`.
10. [x] Close raw-copy permission bypasses with checked VMA/PTE/FrameKind copy
    APIs and temporary loader initialization permissions.
11. [x] Represent `PROT_NONE` with one software PTE bit and preserve Alloc
    ownership in the same PTE across protect, unmap, fork, and teardown.
12. [x] Make `VmSpace::protect` transactional across Private, Shared, Static,
    multiple pages, and multiple VMAs.
13. [x] Remove the derived Alloc-mapping counter and retain teardown checking
    by walking authoritative PTE ownership bits.
14. [x] Remove fixed-size parameters, temporary unmap/teardown vectors,
    affected-VMA clones, and unused helper APIs.
15. [x] Re-run formatting, RISC-V build/clippy, page-table tests, QEMU, review,
    and available guest validation on the final snapshot.
16. [x] Collapse repeated xvma/xmm leaf observations into `mapping_flags` and
    `frame_if_shared`, and batch per-leaf COW permission selection inside
    `ProtectionTransaction` without a second ordinary-protect preflight.
17. [x] Fold the one-method fork module into `VmSpace` lifecycle management and
    keep xvma source files aligned with cohesive responsibilities rather than
    individual operations.

## Trade-offs

### PFN metadata versus `Arc<FrameInner>`

A hardware PTE stores only a physical address. A normal `Arc` would require a
second paddr-to-control-block map or retaining an inaccessible Arc pointer.
PFN-indexed `FrameMeta` makes `Frame` an intrusive physical-frame Arc and lets
the PTE transfer ownership without duplicating resident-frame indexing.

### One Frame type versus allocation typestate

Separate `Page` and `PageRef` types encode publication statically, but expose
two names for the same physical object and force conversions through all fault
paths. One `Frame` plus a uniqueness check moves one atomic load to slow-path
initialization while retaining safe exclusive writes and a smaller API.

### Minimal universal metadata

Page-cache dirty/writeback state, reclaim membership, and DMA pinning stay in
their owning components until a demonstrated cross-subsystem invariant
requires promotion into `FrameMeta`.

## Validation

- V-U-1 Test `FrameMeta` reference transitions and `FrameKind` bit round-trip.
- V-U-2 Test VMA split/merge for Static, Private, and Shared backing.
- V-U-3 Test `ALLOC_FRAME` preservation and sparse leaf walking.
- V-U-4 Audit populated Private/Shared and Static mappings across
  `PROT_NONE`, restore, fork, and unmap; a RISC-V hardware-valid leaf must
  always contain R or X. Exercise these transitions through the public Linux
  ABI in the dedicated RISC-V guest cases.
- V-U-5 Inject a later-page/later-VMA protection failure and verify earlier
  Alloc/Static PTE flags and all VMA flags are unchanged.
- V-I-1 Build and lint the supported RISC-V `fp_simd` configuration.
- V-I-2 Run page-table integration tests and boot QEMU through MM init.
- V-I-3 Run focused guest regressions for anonymous faults, private fork/COW,
  shared fork visibility, cross-VMA `mprotect`, file-backed private faults and
  EOF `SIGBUS`, SysV SHM fork/detach/exit accounting, and static vDSO
  protection transitions. Then run the complete first-party cases profile.
- V-F-1 Audit failed map/remap/shared-map rollback for reference leaks.
- V-F-2 Audit Static/Alloc wrong-kind unmap and TLB-before-release ordering.
- V-F-3 Record unavailable host/unit-test prerequisites without claiming a
  pass, and keep deterministic later-apply failure injection as an explicit
  gap if it is not implemented.
