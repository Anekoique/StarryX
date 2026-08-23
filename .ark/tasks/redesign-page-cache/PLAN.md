# `redesign-page-cache` PLAN

## Approach

Introduce one coherent cached representation for supported regular files while
keeping ownership at four narrow boundaries:

- `xvfs` supplies an optional per-file `CacheSlot` attachment point and raw
  file I/O.
- `xmm` supplies counted `Frame`s and bounded byte copies. It has no file or
  reclaim state.
- `xcache` owns cached pages, writeback, invalidation and clean-page reclaim.
  It is safe Rust and independent of filesystems, VMAs and kernel policy.
- `xkernel` adapts VFS files to `xcache`, cached pages to `xvma`, and starts the
  single maintenance worker.
- `xvma` retains all VMA, PTE, COW, shared-write guard and TLB policy.

```text
xvfs raw I/O + identity       xmm Frame       xtask WaitQueue
             \                  |                  /
              +-------------- xcache -------------+
                                 |
                           neutral page API
                                 v
                               xvma
                                 ^
                                 |
                    xkernel adapters + worker
```

There is no dependency from `xcache` to `xfs`, `xvfs`, `xvma`, `xprocess`,
`xruntime` or `xkernel`. The design learns stable page identity, separated
dirty/writeback state and clean-only reclaim from Linux, but does not reproduce
folios, XArray, rmap, MGLRU, swap, NUMA policy or a process-owned reclaim
daemon.

### File identity and routing

File identity is the object, not a value: every live alias of one ext4 file
incarnation shares one `Arc<CacheSlot>`, handed out by the filesystem's
`ino → Weak<CacheSlot>` map, so a reused inode number always receives a fresh
slot. The slot stores an opaque `Weak` attachment whose concrete type only
`xkernel` knows; open upgrades it or creates a mapping and installs it with a
compare-and-attach that resolves concurrent creation (the loser releases its
mapping). A cache backing retains the node and therefore the slot for as long
as the mapping exists, so reopening a dirty, registry-pinned file converges on
the same mapping.

`FileMapping` ids come from `xvma`'s single object-id allocator: file caches
and anonymous shared `VmObject`s draw from one boot-global, monotonic,
never-reused counter, so any two mappable objects differ by construction and
no crate hard-codes a namespace split. The id serves the `VmObject`/futex
namespace and registry pinning only — it is never a lookup key.

`FileNodeOps::cache_slot() -> Option<&Arc<CacheSlot<M>>>` is the complete VFS
cache contract. `Some(slot)` selects the coherent path. `None` makes pseudo
files, devices and unsupported filesystems bypass the cache. No
filesystem-domain token, classification enum or xcache-specific key DTO is
needed, and `xvfs` carries no cache dependency.

The manager registry owns `Arc<FileMapping>` values keyed by the raw ID. It
retains dirty pages after the final fd closes. An empty mapping with no external
owner may be removed. After a successful last-link unlink, xkernel may discard
an unowned mapping; failed unlink and removal of one hard-link alias never
discard data. Ext4 keeps an open-unlinked inode alive until the last fd, VMA or
cache backing releases it.

### Page loading

Each mapping indexes 4-KiB pages by page number:

```rust
enum PageSlot {
    Loading(Arc<LoadAttempt>),
    Resident(Arc<CachedPage>),
}
```

The first miss installs one `LoadAttempt`; concurrent misses wait on that exact
attempt. The loader allocates and performs raw I/O without the page-tree lock.
It publishes one terminal result before notifying waiters and removes only a
still-matching failed slot. `LoadOwner` publishes `EAGAIN` if the loader exits
early. This per-attempt completion prevents a replacement loader from changing
the result observed by old waiters.

`CachedPage` owns one `Frame`. `PageLease` holds a temporary cache user.
Mapping the page into a PTE clones the `Frame`. Therefore clean reclaim can
require `Frame::is_unique()` without adding cache metadata to `FrameMeta`.

### Dirty data and writeback

`PageState` uses monotonic sequences:

```rust
struct PageState {
    leases: u32,
    dirty_seq: u64,
    submitted_seq: u64,
    persisted_seq: u64,
    writeback_seq: Option<u64>,
    failed_seq: Option<u64>,
    shared_guard_groups: u32,
}
```

Every write advances `dirty_seq`. Writeback copies the page into an owned
4-KiB byte buffer, records the submitted sequence, drops every cache lock and
then calls the raw backing. Success advances `persisted_seq` only through the
submitted sequence, so a concurrent redirty remains dirty. Failure records the
failed sequence, leaves the page resident and dirty, wakes waiters and advances
the mapping error sequence. Background writeback does not spin on the same
failed sequence; explicit sync may retry.

Each open file owns a `WritebackCursor`. A synchronous `fsync`, `fdatasync` or
`msync` writes the selected pages, commits logical length, syncs the backing and
reports the newest unseen mapping error to that cursor once. Asynchronous
`msync` only requests worker progress.

`Frame::read_bytes` and `Frame::write_bytes` perform bounds checks and keep
interrupts and preemption disabled for the complete one-hart copy. They expose
no Rust reference or slice into direct-mapped physical memory. Unpublished
initialization still uses unique-only `Frame::try_write_at`.

### VM objects and file-backed VMAs

`xvma` models backing with two independent decisions rather than syscall- or
file-specific variants:

```rust
struct Backing { source: Source, private: bool }
enum Source { Zero, Static { frames: StaticFrameRange }, Object {
    object: Arc<dyn VmObject>, offset: usize,
} }
```

`VmObject` is the only page-source seam. It supplies identity, length, one
`Frame`, an optional opaque writable-mapping guard and optional sync behavior.
Both anonymous shared memory and cached files implement it; xvma contains no
file type, cache type, observer registry or self-reference.
An object ID is globally unique and one `(id, page index)` retains the same
physical frame while any returned Frame or guard lives.

- `MAP_PRIVATE` maps an object frame read-only and deep-copies it on the first
  write fault.
- `MAP_SHARED` for a guarded object maps read-only. A write fault obtains the
  guard before publishing WRITE in the PTE. Ordinary shared objects need no
  guard and may remain writable.
- fork clones a guard group; unmap, mprotect, invalidation and Drop remove
  WRITE and flush the TLB before releasing the final guard.
- failed fault, fork or protection transactions preserve the previous PTE and
  guard state.

`xkernel::FileVmObject` is only the `xcache -> VmObject` page adapter. A
per-address-space `MappedFiles` registry owns one xcache observer registration
per mapped file and forwards generic object-range invalidation to a weak
`VmSpace`. This is the only module that knows both interfaces.

### Truncate and invalidation

Each address space containing a file VMA owns one registration token for that
file. The token owns the observer while `FileMapping` stores only a `Weak`, so
registration lifetime follows the live VMA tree without a strong cycle.
Enrollment enters the same manager and mapping admission gates used by resize;
it therefore either completes before shrink captures observers or fails after
the gate closes. mmap failure, MAP_FIXED replacement, munmap, exec and exit
prune tokens against the authoritative VMA tree.

Shrink is a two-phase transaction:

1. close admission and wait for admitted mapping operations;
2. capture live observers and validate every affected VMA without mutation;
3. after all validations succeed, remove affected PTEs using allocation-free,
   infallible invalidation callbacks;
4. wait for loads, leases, writeback, shared guards and PTE frame clones;
5. call raw `set_len`, the only irreversible step;
6. zero the retained tail, remove pages beyond EOF, publish the new size and
   reopen admission using infallible operations.

If validation or drain setup fails, the old PTEs, VMA layout, cached data and
backing length remain unchanged. A fault wholly beyond EOF becomes a bus fault.

### Reclaim and lifecycle

The manager stores weak clock candidates. A hit only sets an atomic referenced
bit; one scan clears it and a later scan may reclaim. Direct reclaim performs
no allocation, wait or I/O and accepts only a clean page with no lease, guard
or writeback whose cache-owned `Frame` is unique. Failure leaves the page and
candidate usable.

One xkernel worker calls `CacheManager::run_worker`, supplying the allocator's
current free-page count. xcache applies free-page and dirty-page watermarks,
bounded writeback and clean reclaim internally. It does not depend on task or
process abstractions beyond `WaitQueue`.

`CacheManager::shutdown` implements `Running -> Closing -> Closed`: reject new
operations, drain active operations, perform one finite writeback/reclaim pass
and return an error if dirty or resident pages remain. A started worker owns the
pass and publishes its result; if startup has not run yet, shutdown performs
the same pass inline and a later worker exits immediately. Drop never performs
I/O or silently discards data. Separate public maintenance commands,
statistics DTOs and shutdown reports are intentionally absent because no
independent caller needs them.

### Lock order

No cache lock crosses raw I/O, sleeping, observer callbacks, frame allocation
or allocator accounting. Required nesting follows:

1. manager lifecycle/admission;
2. manager registry;
3. mapping admission;
4. mapping page tree or observer registry;
5. cached-page state;
6. reclaim candidates;
7. waiter notification after state publication.

Observer callbacks may acquire a VmSpace lock only after xcache has released
mapping, page and candidate locks.

## Affected Files

- `xcore/xmm/src/frame.rs`: safe bounded frame byte-copy primitives.
- `xcore/xtask/src/wait_queue.rs`: fallible waiter reservation.
- `xmodules/xvfs/src/node/file.rs`: `CacheSlot` and the optional attachment
  contract.
- `xcore/xfs/src/fs/ext4/**`, `crates/lwext4_rust/src/**`: shared identity,
  open-unlinked lifetime and persistent sync.
- `xmodules/xcache/**`: coherent cache mechanisms.
- `xmodules/xvma/src/{object,backend,area,fault,space}.rs`: generic object-page
  seam, private COW, shared guards, sync and object-range invalidation.
- `xkernel/src/fs/cache.rs` plus fd/syscall/mm/task paths: adapters, coherent
  routing, worker, truncate, mmap/msync and teardown.
- `starry/src/{entry,main}.rs`: worker initialization and shutdown.
- `xtest`: focused guest cases and isolated two-boot report support.
- boundary/performance scripts and architecture/benchmark documentation.

## Risks

- A raw-I/O bypass for a coherent file would split data views. Centralize all
  selection in `xkernel::fs::cache::mapping_for` and scan syscall routing.
- A writable shared PTE without a guard could lose dirty data. Reserve guard
  capacity before PTE mutation and flush before release.
- A multi-address-space truncate could partially unmap. Require all observer
  validations before any invalidation.
- A permanent backing error leaves dirty pages resident by design. Sync and
  shutdown surface the error; no cleanup path discards it.
- Strict per-metric performance gates are noise-sensitive. Preserve the exact
  workload, three fresh boots, artifact hashes and medians; do not introduce a
  tolerance, aggregate score or smoke workload.

## Implementation Steps

1. Add bounded Frame byte copies and fallible wait registration.
2. Add one boot-global VFS file ID and stable ext4 alias/open-unlinked lifetime.
3. Replace the disconnected cache prototype with mapping/page state machines,
   single-flight loads, writeback cursors, invalidation and clean reclaim.
4. Route supported regular-file I/O through one xkernel adapter; leave
   unsupported objects on an explicit `None` path.
5. Unify xvma page sources behind `VmObject`, keep source and private/shared
   policy orthogonal, and put file observer registration only in xkernel.
6. Start one xkernel worker and enforce explicit shutdown before poweroff.
7. Add focused correctness, low-memory, leak and two-boot persistence cases.
8. Run the unchanged iozone workload on three fresh boots and require all 33
   medians to exceed the recorded no-cache baseline.
9. Remove forwarding DTOs, duplicate identity layers and integration wrappers;
   update documentation and verify the frozen tree. Stop before `ark-commit`.

## Verification

### Static and build

- `scripts/check-page-cache-boundary` checks unsafe/dependency boundaries and
  coherent routing inventory.
- RISC-V check, canonical release build and targeted clippy pass.
- changed Rust files pass rustfmt; scripts pass syntax checks; diff check passes.
- `cargo test --manifest-path xtest/Cargo.toml` passes host plan/report/image/
  timeout/process-reaping contracts.

### Guest correctness

- `fs/page_cache`: buffered, positioned, append, truncate, stat, sync,
  hard-link and open-unlinked coherence.
- `fs/page_cache_unlink` under 128 MiB: 4096 dirty close/unlink cycles and a
  sentinel verify no cache/inode leak.
- `mm/file_mmap`: private COW, shared coherence, msync, truncate invalidation
  and beyond-EOF fault behavior.
- `fs/page_cache_pressure` under 128 MiB: pressure progress, data integrity and
  final accounting.
- `fs/page_cache_persist`: one isolated case, two boots over the same disposable
  image, write+fsync then read verification.
- full first-party cases and the unmodified supported OS-COMP profile report an
  honest terminal result and reap descendants. A harness timeout is incomplete,
  never rewritten as a pass.

### Performance

Run exactly three fresh instances of:

```text
make test ARCH=riscv64 CASE=testsuit/iozone/run SMP=1 MEM=1G LOG=off MODE=release
```

The workload remains iozone 3.506 with the historical command set recorded in
`docs/benchmarks/iozone-page-cache.md`. The comparator rejects missing metrics,
duplicate runs and provenance mismatches, computes all 33 three-run medians and
requires every median to be strictly greater than its no-cache baseline.

## Spec

[**Goals**]

- G-1: One coherent cached representation for supported regular-file buffered
  I/O and file-backed mmap.
- G-2: Preserve Frame, VMA/PTE, filesystem and task/kernel boundaries.
- G-3: Make load, writeback, invalidation, reclaim and shutdown finite and
  failure-safe.
- G-4: Bound cache memory and strictly improve all 33 recorded iozone metrics.

[**Non-goals**]

- NG-1: No folios/XArray/rmap/MGLRU, swap, NUMA or hardware dirty harvesting.
- NG-2: No cache state in `FrameMeta`, VMA policy in `xmm`, filesystem types in
  `xcache`, or cache lifecycle in `xprocess`.
- NG-3: No caching for objects returning no VFS cache identity.
- NG-4: No workload weakening to obtain a pass.

[**Data Structure**]

```rust
pub struct CacheSlot<M> {
    attachment: Mutex<M, Option<Weak<dyn Any + Send + Sync>>>,
}

pub trait Backing: Send + Sync {
    fn byte_len(&self) -> LinuxResult<u64>;
    fn read_at(&self, offset: u64, dst: &mut [u8]) -> LinuxResult<usize>;
    fn write_at(&self, offset: u64, src: &[u8]) -> LinuxResult<usize>;
    fn set_len(&self, len: u64) -> LinuxResult;
    fn sync(&self, data_only: bool) -> LinuxResult;
}

enum PageSlot { Loading(Arc<LoadAttempt>), Resident(Arc<CachedPage>) }
struct LoadAttempt {
    page_index: u64,
    result: Mutex<Option<LinuxResult<Arc<CachedPage>>>>,
    wait: WaitQueue,
}
struct CachedPage {
    mapping: Weak<FileMapping>,
    index: u64,
    frame: Frame,
    state: Mutex<PageState>,
    wait: WaitQueue,
    referenced: AtomicBool,
}
struct PageState {
    leases: u32,
    dirty_seq: u64,
    submitted_seq: u64,
    persisted_seq: u64,
    writeback_seq: Option<u64>,
    failed_seq: Option<u64>,
    shared_guard_groups: u32,
}
pub struct PageLease { page: Arc<CachedPage> }
struct SharedWriteGuard { page: Arc<CachedPage> }
pub struct WritebackCursor { seen_sequence: u64 }

enum Lifecycle { Running, Closing, Closed }
struct ManagerState {
    lifecycle: Lifecycle,
    worker_running: bool,
    worker_result: Option<LinuxResult>,
}
pub struct CacheManager {
    policy: CachePolicy,
    lifecycle: Mutex<ManagerState>,
    registry: Mutex<BTreeMap<u64, Arc<FileMapping>>>,
    candidates: Mutex<VecDeque<Weak<CachedPage>>>,
    resident_pages: AtomicUsize,
    dirty_pages: AtomicUsize,
    // admission, progress and wait state
}
pub struct FileMapping {
    id: u64,
    manager: Weak<CacheManager>,
    backing: Arc<dyn Backing>,
    logical_size: AtomicU64,
    pages: Mutex<BTreeMap<u64, PageSlot>>,
    observers: Mutex<WeakMap<u64, Weak<dyn InvalidationObserver>>>,
    errors: Mutex<(u64, Option<LinuxError>)>,
    // admission, append and wait state
}
pub struct ObserverRegistration {
    mapping: Weak<FileMapping>,
    observer_id: u64,
    _observer: Arc<dyn InvalidationObserver>,
}

pub trait InvalidationObserver: Send + Sync {
    fn validate(&self, range: &Range<u64>) -> LinuxResult;
    fn invalidate(&self, range: &Range<u64>);
}

pub type VmPageGuard = Arc<dyn Any + Send + Sync>;
pub struct VmPage {
    pub frame: Frame,
    pub guard: Option<VmPageGuard>,
}
pub trait VmObject: Send + Sync {
    fn id(&self) -> u64;
    fn byte_len(&self) -> LinuxResult<u64>;
    fn page(&self, index: u64, write: bool) -> LinuxResult<VmPage>;
    fn sync(&self, range: Range<u64>, wait: bool) -> LinuxResult { Ok(()) }
    fn requires_write_guard(&self) -> bool { false }
}

struct Backing { source: Source, private: bool }
enum Source {
    Zero,
    Static { frames: StaticFrameRange },
    Object { object: Arc<dyn VmObject>, offset: usize },
}
```

[**API Surface**]

```rust
impl Frame {
    fn read_bytes(&self, offset: usize, dst: &mut [u8]) -> XResult;
    fn write_bytes(&self, offset: usize, src: &[u8]) -> XResult;
}

impl CacheManager {
    fn new(policy: CachePolicy) -> LinuxResult<Arc<Self>>;
    fn create_mapping(self: &Arc<Self>, id: u64, backing: Arc<dyn Backing>)
        -> LinuxResult<Arc<FileMapping>>;
    /// Re-pins a mapping revived through a weak reference; ids are never
    /// lookup keys.
    fn ensure_registered(&self, mapping: &Arc<FileMapping>) -> LinuxResult;
    fn discard_unowned(&self, id: u64) -> bool;
    fn run_worker(&self, available_pages: impl Fn() -> usize);
    fn shutdown(&self) -> LinuxResult;
}

impl FileMapping {
    fn id(&self) -> u64;
    fn release_hint(&self);
    fn size(&self) -> u64;
    fn read_at(self: &Arc<Self>, dst: &mut [u8], offset: u64)
        -> LinuxResult<usize>;
    fn write_at(self: &Arc<Self>, src: &[u8], offset: u64)
        -> LinuxResult<usize>;
    fn append(self: &Arc<Self>, src: &[u8]) -> LinuxResult<(usize, u64)>;
    fn acquire_page(self: &Arc<Self>, index: u64) -> LinuxResult<PageLease>;
    fn resize(self: &Arc<Self>, new_len: u64) -> LinuxResult;
    fn register_observer(self: &Arc<Self>, observer: Arc<dyn InvalidationObserver>)
        -> LinuxResult<ObserverRegistration>;
    fn new_cursor(&self) -> WritebackCursor;
    fn sync_range(&self, range: Range<u64>, data_only: bool, wait: bool,
                  cursor: &mut WritebackCursor) -> LinuxResult;
}

impl PageLease {
    fn frame(&self) -> Frame;
    fn shared_write_guard(&self) -> LinuxResult<Arc<dyn Any + Send + Sync>>;
}

impl VmSpace {
    fn maps_object(&self, id: u64) -> bool;
    fn validate_object_range(&self, id: u64, range: &Range<u64>) -> XResult;
    fn unmap_object_range(&mut self, id: u64, range: &Range<u64>);
    fn sync_object_range(&self, range: VirtAddrRange, wait: bool) -> LinuxResult;
}
```

[**Performance Contract**]

Every candidate sample is one fresh RISC-V boot created by:

```text
make test ARCH=riscv64 CASE=testsuit/iozone/run SMP=1 MEM=1G LOG=off MODE=release
```

The guest runs iozone 3.506 exactly eight times:

```text
-a -r 1k -s 4m
-t 4 -i 0 -i 1 -r 1k -s 1m
-t 4 -i 0 -i 2 -r 1k -s 1m
-t 4 -i 0 -i 3 -r 1k -s 1m
-t 4 -i 0 -i 5 -r 1k -s 1m
-t 4 -i 6 -i 7 -r 1k -s 1m
-t 4 -i 9 -i 10 -r 1k -s 1m
-t 4 -i 11 -i 12 -r 1k -s 1m
```

The no-cache baselines in KiB/s are authoritative:

| metric | baseline | metric | baseline |
| --- | ---: | --- | ---: |
| auto.write | 19883 | auto.rewrite | 17512 |
| auto.read | 17395 | auto.reread | 17359 |
| auto.random_read | 15368 | auto.random_write | 14165 |
| auto.backward_read | 13549 | auto.record_rewrite | 11671 |
| auto.stride_read | 9899 | auto.fwrite | 7736 |
| auto.frewrite | 9601 | auto.fread | 5813 |
| auto.freread | 6595 | write_read.initial_writers | 14932.09 |
| write_read.rewriters | 14409.15 | write_read.readers | 15909.74 |
| write_read.re_readers | 14011.29 | random.initial_writers | 16475.96 |
| random.rewriters | 17756.47 | random.random_readers | 14813.51 |
| random.random_writers | 12720.95 | backward.initial_writers | 16234.92 |
| backward.rewriters | 12420.24 | backward.reverse_readers | 14715.40 |
| stride.initial_writers | 16874.84 | stride.rewriters | 15491.26 |
| stride.stride_readers | 12764.59 | stdio.fwriters | 34065.80 |
| stdio.freaders | 15157.02 | positional.pwrite_writers | 12099.74 |
| positional.pread_readers | 7222.52 | vector_fallback.initial_writers | 13022.66 |
| vector_fallback.rewriters | 13076.74 | | |

The gate accepts three distinct passed run IDs only when every three-run median
is strictly greater than its baseline. It rejects missing/duplicate metrics,
non-terminal runs, QEMU failure, any workload marker mismatch, dirty xtest
input, or provenance different from: StarryX baseline
`b76f4d7138e1d9bd02d660cf0bbad1c9c611ded6`, xtest
`59faed8281fd17234d682144a7fcd70accb0a6ad`, rootfs SHA-256
`636840a4feda55e10f5d1dce394fd733d16ed626b97668239024d9ac2c5aaef6`,
driver SHA-256
`86973a9de127d1224838e7a13e51219a07bf83263bd630afc4e8cfcda4b6c1a6`,
manifest SHA-256
`26f53f0b5335a44042ccc2866ff19e70b68bcef7948f047187fa7e964ec23a67`,
rustc `1.96.0-nightly (03749d625 2026-03-14)`, QEMU 11.0.0, macOS
`26.5 (25F71)`, and arm64 host.

[**Constraints**]

- C-1: @source-scan: `#![forbid(unsafe_code)] @ xmodules/xcache/src/lib.rs`
  xcache is safe Rust and owns no raw physical allocation.
- C-2: @source-scan: `xfs|xvfs|xvma|xprocess|xruntime|xkernel @ xmodules/xcache/Cargo.toml`
  xcache has no filesystem, VMA, process, runtime or kernel dependency.
- C-3: @judgment A file incarnation has one shared per-inode slot and one
  boot-global, monotonic, never-reused object id from the single `xvma`
  allocator; aliases share both, recreated files get fresh ones, and no crate
  hard-codes an id-namespace split. The registry pins every mapping holding
  pages; a mapping revived through the slot's weak reference is re-registered
  before use, and unlink discard decides under the slot lock.
- C-4: @judgment One index has one current load attempt or resident Frame;
  waiters observe the exact attempt they captured.
- C-5: @judgment Cache locks do not cross I/O, sleep, callbacks or allocation,
  and nested locks obey the declared order.
- C-6: @judgment Every dirty transition increments its sequence; writeback never
  clears a newer redirty.
- C-7: @judgment Failed writeback remains dirty and finite; each cursor reports
  the newest unseen mapping error once.
- C-8: @test-binding: page_cache Buffered, positioned, append, truncate, stat,
  sync, alias and open-unlinked operations remain coherent.
- C-9: @judgment Observer enrollment shares resize admission; every token owns
  its observer, and all validations precede any infallible invalidation and raw
  truncate.
- C-10: @test-binding: file_mmap Private mappings COW; shared mappings retain
  guards, coherence, sync and EOF fault semantics.
- C-11: @judgment Shared file WRITE is visible only while VmSpace owns a guard;
  TLB invalidation precedes final release.
- C-12: @judgment Direct reclaim never sleeps, performs no I/O, and makes no
  allocation required for progress; its only allocator call is a fallible
  candidate-list reservation whose failure drops the candidate safely. It
  removes only clean, idle pages with unique cache Frame ownership. Dropping
  the last reference to a fully unlinked file's mapping may chain into the
  filesystem's deferred inode release after all cache locks are released.
- C-13: @test-binding: page_cache_pressure Low-memory execution makes progress,
  preserves data and restores expected accounting.
- C-14: @judgment Shutdown is explicit and finite with or without a started
  worker; unresolved data is returned as an error and never silently discarded.
- C-15: @source-scan: `mapping_for|cache_slot @ xkernel/src/fs xkernel/src/syscall`
  coherent routing is centralized and bypass objects never enter xcache.
- C-16: @test-binding: multi_boot_case_requires_an_isolated_run Multi-boot uses
  one isolated case and one disposable image before report finalization.
- C-17: @test-binding: page_cache_persist Two boots prove ext4 persistence.
- C-18: @tool: `cargo test --manifest-path xtest/Cargo.toml` Host contracts pass.
- C-19: @tool: `scripts/check-page-cache-boundary` Boundary inventory passes.
- C-20: @tool: `scripts/bench/compare-page-cache-iozone <r1> <r2> <r3> --output docs/benchmarks/iozone-page-cache.json`
  all 33 exact-workload medians strictly exceed baseline.
- C-21: @tool: `make test PROFILE=oscomp ARCH=riscv64 SMP=1 MEM=1G LOG=off MODE=release`
  OS-COMP is unmodified and reports an honest terminal result.
- C-22: @judgment `FrameMeta` remains one refcount; bounded Frame copies expose
  no direct-map reference and all cache state stays outside xmm.
- C-23: @test-binding: page_cache_unlink Repeated dirty unlink cycles leak no
  cache Frames, registry entries or filesystem inodes.
- C-24: @source-scan: `VmFile|FileInvalidation|xcache @ xmodules/xvma/src`
  xvma exposes only generic `VmObject` policy and contains no file/cache
  registration state; xkernel is the sole cross-module adapter.
