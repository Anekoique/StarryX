# `remove-xcache` PRD

---

[**What**]

Disconnect the existing `xcache` page-cache implementation from `xkernel`
while retaining the standalone component unchanged, restore direct filesystem
I/O paths, and establish a reproducible no-page-cache iozone baseline.

[**Why**]

The current page cache is attached at selected file-descriptor call sites
rather than owned by stable VFS inode objects. Its inode-only global key,
path-based pseudo-filesystem exclusions, split buffered/direct/append paths,
and incomplete truncate/writeback lifecycle make it unsuitable as the baseline
for the planned storage-stack redesign. Removing the kernel integration first
creates a correctness-oriented direct-I/O baseline and a measurable reference
for a later complete page-cache implementation.

[**Outcome**]

- `xmodules/xcache/**` remains byte-for-byte unchanged and stays in the workspace.
- `xkernel` no longer imports, adapts, or calls `xcache`, `PageCache`, or
  `PAGE_CACHE_MANAGER`; its Cargo dependency is removed.
- Ordinary file I/O, truncate, sync, metadata, ELF/file-backed mapping reads,
  and allocator behavior use their direct pre-cache backing operations.
- `starry` no longer carries an unused direct dependency on `xcache`.
- Documentation describes `xcache` as retained but currently disconnected,
  without claiming that StarryX runtime I/O is page-cached.
- RISC-V and LoongArch kernels build, and the passing first-party `cases`
  profile completes on both architectures.
- The complete RISC-V `oscomp` profile completes successfully; unrelated
  pre-existing failures are reported rather than silently fixed in this task.
- The xtest iozone driver uses `/var/tmp/iozone-scratch` on the ext4 rootfs,
  rather than the `MemoryFs` mounted at `/tmp`, so the workload exercises the
  storage stack that a future page cache will optimize.
- The xtest fix is committed and pushed, and StarryX pins the resulting
  submodule commit instead of retaining an unrecorded gitlink difference.
- RISC-V `testsuit/iozone/run` completes in three fresh guest boots, with raw
  evidence paths, environment, per-metric results, and medians recorded as the
  no-page-cache baseline.

[**Related Specs**]

- `specs/features/xtest/redesign-xtest-framework/SPEC.md` — its stable profile,
  selector, immutable evidence, and report contracts define system-test and
  iozone baseline execution.
- `specs/features/xtest/port-oscomp-suites/SPEC.md` — its packaged iozone
  workload defines the retained OS-COMP storage benchmark sequence; this task
  corrects its scratch filesystem without changing the sequence.

[**SPEC Path**]

N/A — standard-tier task.
