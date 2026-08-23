# `redesign-page-cache` PRD

---

[**What**]

Replace the unused `xcache` prototype with a coherent, reclaimable file page
cache whose pages are owned by `xmm::Frame`, then integrate it through
`xkernel` so buffered file I/O, file-backed mmap, writeback, truncate, and
memory-pressure reclaim operate on one stable cached representation per file.

[**Why**]

The current `xcache` stores copyable raw physical addresses in an unbounded
per-file LRU, performs backing I/O without a safe single-flight publication
protocol, cannot represent redirty during writeback, and manually frees pages
outside the `xmm::Frame` ownership model. It is therefore unsafe to reconnect
to the kernel and cannot provide bounded memory use or failure-atomic reclaim.

StarryX also has two incompatible file-memory paths: ordinary file operations
bypass `xcache`, while file `MAP_SHARED` creates a private eager snapshot that
is not coherent with another mapping or buffered I/O and is never written back.
The page cache must become the single file-data mechanism without moving file,
VMA, task, or process policy into `xmm`.

The design is informed by Linux's stable mapping/index identity, separated
writeback and reclaim, and watermarked background progress, and by Asterinas's
page-level single-flight and snapshot writeback. It deliberately does not copy
Linux folios, XArray, reverse mapping, NUMA/zone policy, MGLRU, or the complete
Asterinas VMO/frame-metadata design. The source survey and StarryX-specific
reasoning are recorded in `research/page-cache-design.md`.

[**Outcome**]

1. Each stable regular-file identity resolves to one live `FileMapping`; all
   opens, aliases, buffered I/O, and file mappings observe the same cached
   pages, while pseudo files and devices use an explicit bypass path.
2. `xcache` owns cached data through `xmm::Frame` and contains no raw-page
   allocation or deallocation. Concurrent misses publish one loading slot,
   perform one backing read, and wake all waiters without exposing partial data.
3. Cached-page state correctly represents dirty, writeback, redirty, terminal
   writeback error, isolation, and invalidation. No cache/index/page lock is
   held across backing I/O, sleep, or an `xvma` callback.
4. Buffered `read`, `write`, positioned/vector I/O, append, file-size changes,
   `fsync`, and `fdatasync` use the same mapping. Partial-page writes, EOF,
   extension holes, truncate tail zeroing, concurrent append, and error
   propagation have deterministic semantics.
5. File-backed `MAP_PRIVATE` reads may share cached Frames and continue to use
   `xvma` COW on write. Separate `MAP_SHARED` mappings and buffered I/O are
   mutually coherent; `msync`, truncate invalidation, fork, and mapping teardown
   preserve page lifetime and writeback correctness.
6. One worker created by `xkernel` performs batched writeback and reclaim using
   configurable free-page and dirty-page watermarks. Direct reclaim only
   isolates clean, unpinned pages after allocator locks are released. Cache
   memory remains bounded and failed reclaim/writeback never loses a page.
7. Subsystem boundaries remain one-way: `xmm` supplies Frame/PTE mechanisms,
   `xcache` supplies cache mechanisms, `xvma` owns VMA/fault/COW policy,
   `xfs/xvfs` supply raw backing I/O and identity, `xtask` only executes/wakes
   work, `xkernel` composes adapters, and `xprocess` is not a dependency.
8. Deterministic unit and guest tests cover loading races, redirty, writeback
   errors, truncate/invalidation, buffered/mmap coherence, reclaim races,
   low-memory progress, and accounting. Forced reclaim and teardown return
   cache and Frame counts to their expected baselines without leaks.
9. All existing first-party cases and every supported OS-COMP testsuit complete
   without weakened expectations, reduced workloads, hidden skips, hangs, or
   unreaped descendants. File contents remain correct across sync, remount, and
   the supported restart boundary.
10. The unchanged iozone workload from
    `docs/benchmarks/iozone-no-page-cache.md` runs as three independent fresh
    boots under the recorded environment. The three-run median of every one of
    the 33 valid recorded metrics is strictly greater than its historical
    value; no aggregate score, tolerance, workload change, or smoke profile may
    replace this per-metric gate.

[**Related Specs**]

- `specs/features/kernel/mm/redesign-mm-subsystem/SPEC.md` — preserves Frame
  reference ownership, PTE lifetime, `PROT_NONE`, COW, and the separation of
  page-cache state from `FrameMeta` and `AddressSpace`.
- `specs/features/xtest/redesign-xtest-framework/SPEC.md` — uses immutable run
  artifacts, exact case selection, host/guest timeout semantics, and truthful
  terminal reports for correctness and benchmark evidence.
- `specs/features/xtest/port-oscomp-suites/SPEC.md` — retains the supported
  OS-COMP suite manifests and native verdict adapters; page-cache work may not
  modify suite workloads or quarantine failures to satisfy acceptance.

[**SPEC Path**]

`kernel/fs/redesign-page-cache`
