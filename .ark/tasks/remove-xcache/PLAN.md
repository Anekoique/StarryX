# `remove-xcache` PLAN

> Status: Approved for Implementation
> Feature: `remove-xcache`
> Owner: Executor

---

## Summary

Remove every runtime page-cache attachment from `xkernel`, restore one direct
filesystem path for file and mapping operations, retain the standalone
`xcache` component unchanged, and capture a reproducible ext4-backed iozone
baseline. The test harness receives only the scratch-path correction needed to
avoid benchmarking the `/tmp` memory filesystem.

> Deep tier: REVIEW findings are folded into this PLAN in place before EXECUTE — there is no iteration history to track here.

---

## Spec

> This section is the durable design record. On deep-tier commit, it is copied **verbatim** into `specs/features/<slug>/SPEC.md`. Keep it tight: the SPEC is what future readers consult to understand what was built, not why each step happened. Why-explanations belong in `## Trade-offs`. Implementation steps belong in `## Implementation`. The Spec is the contract.

[**Goals**]

> One line per bullet, ≤80 chars, verb-led, capability-oriented (the *what*, not the *how*). Soft cap: 5. If you have more goals, you are listing implementation steps — promote them to Constraints or drop them.
>
> Good: `G-1: ark context prints a JSON snapshot of git + tasks + specs.`
> Bad:  `G-1: Two flags control output: --scope {session|phase} and --for {design|...} ...`  ← that's a Constraint.

- G-1: Route kernel file and mapping operations directly to the filesystem.
- G-2: Retain `xcache` as a disconnected component for later redesign.
- G-3: Run iozone against ext4 and preserve a no-cache performance baseline.
- G-4: Preserve the supported first-party and OS-COMP test behavior.

[**Non-goals**]

> Only list when a reasonable reader would assume the item is in scope. Skip blanket exclusions of features nobody requested. Soft cap: 3.

- NG-1: Redesign or modify the implementation under `xmodules/xcache`.
- NG-2: Add a replacement page cache, readahead, or writeback subsystem.
- NG-3: Tune iozone, ext4, block devices, or QEMU for a better score.

[**Architecture**]

> Show the design, not just where code lives: name the components, what each owns, and how data / control flows between them — put shared state at the top, label the edges. Prefer a fenced-ASCII component diagram (or a layered stack / call graph). A bare file→responsibility tree is the weakest form — if you use one, add the arrows. A short module map may follow.
>
> ```
>            ┌──────── shared Layout (paths) + task.toml ────────┐
>            │                                                   │
>            ▼                                                   ▼
>      ┌───────────┐      ┌───────────┐                    ┌───────────┐
>      │  verify   │      │ spec_x    │  extract `## Spec` │  commit   │
>      │ gate: no  │─ok─▶ │ deep tier │─▶ features INDEX ─▶│ stage +   │
>      │ PENDING   │      │ → SPEC.md │   upsert (leaf→root)│ git commit│
>      └───────────┘      └───────────┘                    └─────┬─────┘
>            ▲                                                   │
>            └──────────── scoped rollback ◀── on failure ───────┘
> ```

```
 userspace read/write/mmap/truncate/stat/fsync
                      │
                      ▼
                ┌───────────┐
                │  xkernel  │  owns syscall and VmFile adaptation
                └─────┬─────┘
                      │ direct FsFile/FileNodeOps calls
                      ▼
                ┌───────────┐
                │ xfs / VFS │
                └─────┬─────┘
                      ▼
                  ext4 rootfs

 xmodules/xcache ── retained in workspace, no runtime edge from xkernel

 xtest iozone ── /var/tmp/iozone-scratch ──▶ ext4 rootfs
             three fresh boots ──▶ raw reports ──▶ median baseline document
```

`xkernel::fs::fd::File` owns direct descriptor I/O. `FileWrapper` remains the
`xvma::VmFile` adapter but reads the same backing `FsFile` directly. Truncate
and metadata syscalls no longer perform cache eviction or writeback. xtest
continues to own workload packaging and changes only the iozone scratch path.

[**Data Structure**]

> Public types only. Field names + types + a one-line comment when meaning is non-obvious.

No new public data structures are introduced. `FileWrapper` remains the
existing file-backed mapping adapter; `InodeWrapper` and the kernel-local page
cache manager are removed.

[**API Surface**]

> Public function signatures + one-line semantics. No bodies.

No new public API is introduced. Existing filesystem and syscall APIs retain
their signatures and observable Linux-facing semantics.

[**Constraints**]

> Invariants the implementation must hold, each a two-line bullet. Line 1 is the actuator tag `- C-N: @<kind>[: <arg>]` — `tool`, `source-scan` (`<pattern> @ <glob>`), `test-binding` (a test id), or `judgment`; the arg names a real test or command, never a `V-*` label. Line 2 is one declarative sentence (≤120 chars). The *why* belongs in Trade-offs, not here.
>
> Good:
> - C-1: @test-binding: <your_test_fn_name>
> ark context emits exactly one stdout write per invocation.
>
> Bad (elaboration is the *how*, belongs in Implementation): `ark context emits one stdout write: JSON via a pre-rendered string + newline, text via a single Display write. No interspersed debug prints.`

- C-1: @source-scan: xcache|PAGE_CACHE_MANAGER|PageCache|InodeWrapper @ xkernel/** starry/**
Runtime crates contain no page-cache dependency, adapter, manager, or call.
- C-2: @tool: git diff --exit-code -- xmodules/xcache
The retained xcache component is byte-for-byte unchanged.
- C-3: @tool: git -C xtest diff HEAD^ -- testsuits/iozone/iozone_testcode.sh
The xtest workload change is limited to moving scratch I/O from tmpfs to ext4.
- C-4: @tool: cargo test --manifest-path xtest/Cargo.toml
The xtest host planner, builder, image, QEMU, and report contracts remain valid.
- C-5: @tool: make build ARCH=riscv64 && make build ARCH=loongarch64
Both supported StarryX architectures build without the kernel xcache adapter.
- C-6: @tool: make test ARCH=riscv64 PROFILE=cases
The RISC-V first-party cases profile passes after direct-I/O restoration.
- C-7: @tool: make test ARCH=loongarch64 PROFILE=cases
The LoongArch first-party cases profile passes after direct-I/O restoration.
- C-8: @tool: make test ARCH=riscv64 PROFILE=oscomp
The complete RISC-V OS-COMP profile is executed and its result is preserved.
- C-9: @tool: make test ARCH=riscv64 CASE=testsuit/iozone/run
Three fresh-boot ext4-backed iozone runs produce immutable raw evidence.
- C-10: @judgment
The baseline records exact commits, environment, all reported metrics, and medians.

---

## Runtime

[**Main Flow**]

1. An ordinary syscall resolves an `FsFile` and invokes its direct operation.
2. File-backed mappings use `FileWrapper` to read the same file directly.
3. Filesystem sync, truncate, and metadata operations no longer consult a
   kernel-global cache registry.
4. xtest builds an ext4 image containing iozone and runs each selected case in
   a fresh QEMU guest with scratch files under `/var/tmp`.
5. Three iozone reports are retained and summarized without workload tuning.

[**Failure Flow**]

1. Direct filesystem errors propagate through the existing Linux errno path.
2. xtest records build, timeout, protocol, or case failures in its normal
   immutable evidence directory and returns failure to `make test`.
3. A failing unrelated OS-COMP case is reported as observed; it does not expand
   this storage refactor into an unrelated compatibility repair.

[**State Transitions**]

- Integrated → disconnected when all xkernel adapters and dependencies vanish.
- Tmpfs benchmark → ext4 benchmark when iozone scratch moves to `/var/tmp`.
- Unmeasured → baselined after three successful fresh-boot reports are reduced
  to per-metric medians.

---

## Implementation

[**Phase 1 — Direct filesystem path**]

- Remove `xcache` dependencies from `starry` and `xkernel`.
- Delete the kernel-local page-cache manager and its allocator hook.
- Simplify descriptor I/O, mapping reads, truncate, sync, and metadata paths to
  their existing direct backing operations.
- Remove now-unused pseudo-filesystem and inode adapter seams.

[**Phase 2 — Honest storage workload**]

- Change only the xtest iozone scratch default and documentation from `/tmp`
  to `/var/tmp`, preserving all eight OS-COMP invocations and labels.
- Run xtest host checks, commit and push the standalone xtest change, then pin
  that commit in the StarryX submodule.
- Update StarryX documentation to state that xcache is retained but inactive.

[**Phase 3 — Regression and baseline evidence**]

- Format, source-scan, and build both supported architectures.
- Run first-party cases on RISC-V and LoongArch, then full RISC-V OS-COMP.
- Run targeted RISC-V iozone three times with fresh guests and fixed settings.
- Preserve report directories and publish raw measurements plus medians in a
  repository benchmark note.

---

## Trade-offs

- T-1: Direct I/O sacrifices current cache performance but creates a simpler,
  correctness-oriented control point for measuring the replacement.
- T-2: Retaining `xcache` preserves reference code without letting obsolete
  integration dictate the new cache architecture.
- T-3: Changing xtest is cross-repository work, but it is necessary because
  `/tmp` is `MemoryFs` and cannot measure the ext4 storage path.
- T-4: Three fresh boots cost more time than repeated cases in one guest, but
  avoid warm guest state and make run-to-run variance visible.

---

## Validation

[**Unit Tests**]

- V-UT-1: Run the xtest Rust tests and shell syntax checks.

[**Integration Tests**]

- V-IT-1: Build StarryX for RISC-V and LoongArch.
- V-IT-2: Run `cases` on both architectures.
- V-IT-3: Run full RISC-V `oscomp`.
- V-IT-4: Run RISC-V iozone in three fresh boots and verify all eight stages.

[**Failure / Robustness**]

- V-F-1: Confirm xtest propagates any build, timeout, protocol, or case failure.

[**Edge Cases**]

- V-E-1: Verify file-backed mmap and ELF reads do not retain a hidden cache path.
- V-E-2: Verify pseudo-filesystem files continue through their native direct path.
- V-E-3: Verify truncate changes length directly without stale cached pages.

[**Acceptance Mapping**]

| Goal / Constraint | Validation |
|-------------------|------------|
| G-1, C-1 | Source scan; both builds; both `cases` runs |
| G-2, C-2 | Scoped xcache diff and workspace dependency inspection |
| G-3, C-3, C-9, C-10 | xtest diff; three reports; baseline document |
| G-4, C-4, C-6, C-7, C-8 | xtest tests; cases; full OS-COMP |
| C-5 | RISC-V and LoongArch build commands |
