
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
- NG-2 Do not place file, page-cache, writeback, reclaim, or observer state in
  `xmm` or `xvma`; those mechanisms belong to xcache and xkernel integration.
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
- `xvma::Backing` separates page source from private/shared write policy.
- `xvma::VmObject` is the generic page-source boundary. `SharedObject` and a
  cached-file adapter both implement it, but xvma stores no file/cache state.
- `xcache` may retain `Frame`s and its own cache-specific state; it does not
  extend `FrameMeta`.

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

struct Backing { source: Source, private: bool }
enum Source {
    Zero,
    Static { frames: StaticFrameRange },
    Object { object: Arc<dyn VmObject>, offset: usize },
}

pub struct Backend {
    backing: Backing,
    page_size: PageSize,
    populate: bool,
}

pub type VmPageGuard = Arc<dyn Any + Send + Sync>;
pub struct VmPage {
    frame: Frame,
    guard: Option<VmPageGuard>,
}
pub trait VmObject: Send + Sync {
    fn id(&self) -> u64;
    fn byte_len(&self) -> LinuxResult<u64>;
    fn page(&self, index: u64, write: bool) -> LinuxResult<VmPage>;
    fn sync(&self, range: Range<u64>, wait: bool) -> LinuxResult { Ok(()) }
    fn requires_write_guard(&self) -> bool { false }
}

pub struct SharedObject {
    frames: Box<[Frame]>,
}
```

`Source` determines where pages come from; `private` independently selects COW
or write-through behavior. Anonymous private memory is `{ Zero, true }`;
anonymous shared memory and files both use `Object`. A guarded object may
publish shared WRITE only while `VmSpace` owns the returned guard. Static PTEs
never contribute a reference count.

Callers create every area through `VmSpace::map(..., Backend)`, replacing the
kind-specific `map_alloc`, `map_static`, `map_file`, and `map_shared` surface.
One-shot creation policy is converted into the private persistent `Backing`.
The closed `Source` enum provides crate-private static dispatch for VMA
behavior; only page supply is dynamic. This avoids per-area boxed backends,
file-specific variants, and duplicated private/shared policy.

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
    fn is_unique(&self) -> bool;
    fn read_bytes(&self, offset: usize, destination: &mut [u8]) -> XResult;
    fn write_bytes(&self, offset: usize, source: &[u8]) -> XResult;
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
    fn validate_alloc_range(&self, va: VirtAddr, size: usize) -> XResult;
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
    fn shared_object_at(&self, va: VirtAddr) -> Option<(u64, usize)>;
    fn maps_object(&self, id: u64) -> bool;
    fn validate_object_range(&self, id: u64, range: &Range<u64>) -> XResult;
    fn unmap_object_range(&mut self, id: u64, range: &Range<u64>);
    fn sync_object_range(&self, range: VirtAddrRange, wait: bool) -> LinuxResult;
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

`Frame::read_bytes` and `Frame::write_bytes` perform bounded copies on published
allocator frames. Each call is available only in the supported one-hart model,
holds `NoPreemptIrqSave` for the complete copy, and never exposes a Rust slice
or reference to frame bytes. Ordinary `Frame` values cannot represent DMA-owned
storage.

`AddressSpace::read_bytes` checks READ on every leaf and rejects device memory;
`write_alloc_bytes` additionally requires WRITE and `FrameKind::Alloc` on every
leaf. `VmSpace::read_bytes`/`write_bytes` first enforce authoritative VMA
permissions. ELF construction temporarily maps Alloc frames writable,
initializes them through this checked path, and restores final permissions
before the address space becomes user-visible. No safe raw-copy API may bypass
`StaticFrameRange` access or alias proofs.

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
- C-19 `Frame` byte-copy methods serialize each bounded copy on one hart
  without exposing a Rust reference to published frame storage.
- C-20 The xvma/xmm boundary avoids repeated observations of one leaf:
  page-fault dispatch obtains resident flags once, shared-frame cloning occurs
  only when required, and transactional protection computes per-leaf target
  flags inside one batch rather than issuing per-page transaction calls.
- C-21 xvma contains no file/cache type or observer registration. One generic
  `VmObject` supplies stable `(id, index)` frames; source and private/shared
  policy remain orthogonal.
- C-22 A guarded object page becomes writable only after guard storage is
  reserved. Unmap, protect, invalidation and Drop remove WRITE and flush the TLB
  before releasing the final guard.
- C-23 Generic object invalidation validates all Alloc leaves before the
  integration layer invokes its infallible removal phase.

[**CHANGELOG**]

- 2026-08-20: Replaced file-specific xvma backing/observer state with the
  generic `VmObject` page source, orthogonal source/private policy, and opaque
  shared-write guards; file registration remains in xkernel.
- 2026-08-17: Added bounded one-hart byte copies for published allocator Frames,
  while retaining the single-refcount FrameMeta and unique-only unpublished
  initialization contract; the execution guard stays private to each copy.
