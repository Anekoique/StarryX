
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
