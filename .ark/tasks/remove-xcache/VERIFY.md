# `remove-xcache` VERIFY

> Status: Verified with documented pre-existing platform/test limitations.
> Feature: `remove-xcache`
> Target Task: `remove-xcache`
> Tier: `standard`

## Project Spec Compliance

### Index integrity

- [x] PASS — `.ark/specs/project/` contains only `INDEX.md`; there are no
  unlisted project SPEC directories.

### Leaf SPECs

- N/A — no project-level leaf SPEC exists.

## Related Feature Spec Compliance

- [x] PASS — `specs/features/xtest/redesign-xtest-framework/SPEC.md`:
  the selector, immutable evidence directory, protocol result, committed-input
  staging, timeout continuation, and submodule ownership contracts were kept.
- [x] PASS — `specs/features/xtest/port-oscomp-suites/SPEC.md`:
  all eight iozone invocations and labels were preserved; only the scratch
  filesystem changed from MemoryFs-backed `/tmp` to ext4-backed `/var/tmp`.

## Plan Fidelity

- [x] PASS — G-1: descriptor I/O, positional I/O, file-backed mapping reads,
  truncate, sync, and metadata now call `FsFile`/VFS directly.
- [x] PASS — G-2: `xmodules/xcache` remains a workspace component and has no
  diff, while runtime manifests and sources contain no xcache edge.
- [x] PASS — G-3: three fresh-boot ext4 iozone runs passed and
  `docs/benchmarks/iozone-no-page-cache.md` records raw values and medians.
- [x] PASS — G-4: RISC-V first-party cases passed 9/9; the complete OS-COMP
  profile reached `run_end`, with its two pre-existing unsupported outcomes
  isolated and reported below.

## Constraint Evidence

| Constraint | Result | Evidence |
| --- | --- | --- |
| C-1 | PASS | Source scan found no `xcache`, `PAGE_CACHE_MANAGER`, `PageCache`, or `InodeWrapper` in `xkernel/**` or `starry/**`. |
| C-2 | PASS | `git diff --exit-code -- xmodules/xcache` returned 0. |
| C-3 | PASS | xtest commit `59faed8281fd17234d682144a7fcd70accb0a6ad` changes only the iozone scratch default/comment and is pushed to `origin/main`. |
| C-4 | PASS | `cargo test` in xtest: 29 passed, 0 failed; all relevant shell syntax checks passed. |
| C-5 | PARTIAL | RISC-V built; LoongArch is blocked by the documented missing external vDSO image (V-002). |
| C-6 | PASS | RISC-V `cases`: 9 passed, 0 failed, 0 timed out; evidence `target/xtest/riscv64/cases/6a78d6f0-0bc9f990-3269/`. |
| C-7 | BLOCKED | LoongArch cannot reach guest execution until a compatible `vdso_loongarch64.so` provider is supplied (V-002). |
| C-8 | PASS | Full RISC-V OS-COMP reached `run_end`: 8 passed, 1 failed, 1 timed out; evidence `target/xtest/riscv64/oscomp/6a78d922-03b2de70-905f/`. |
| C-9 | PASS | Three targeted iozone runs each passed 1/1 with guest/QEMU exit 0 (V-IT-4). |
| C-10 | PASS | Benchmark note records revisions, host/QEMU/guest settings, 35 primary metrics, all three values, medians, evidence paths, and checksums. |

## Validation Runs

- PASS — `cargo fmt --all -- --check`.
- PASS — `make clippy ARCH=riscv64`; only unrelated pre-existing warnings.
- PASS — `make build ARCH=riscv64`.
- BLOCKED — `make build ARCH=loongarch64`; external vDSO provider lacks the
  required architecture image.
- PASS — `cargo test` in xtest: 29/29.
- PASS — xtest shell syntax checks.
- PASS — `make test ARCH=riscv64 PROFILE=cases`: 9/9.
- PASS — three invocations of
  `make test ARCH=riscv64 CASE=testsuit/iozone/run SMP=1 MEM=1G LOG=off MODE=release`:
  - `target/xtest/riscv64/oscomp/6a78d70c-1ad69ee8-3ab2/`
  - `target/xtest/riscv64/oscomp/6a78d7a9-1d8e5090-5160/`
  - `target/xtest/riscv64/oscomp/6a78d823-38ee3800-6779/`
- OBSERVED — `make test ARCH=riscv64 PROFILE=oscomp`: the runner completed all
  ten cases and returned 8 passed, 1 failed, 1 timed out (V-003).

## SPEC Drift

- N/A — no feature SPEC was modified, so no CHANGELOG entry is required.

## Findings

### V-001 iozone previously measured MemoryFs

- **Severity:** HIGH
- **Location:** `xtest/testsuits/iozone/iozone_testcode.sh`
- **Problem:** The default scratch directory was `/tmp`, which StarryX mounts
  as `MemoryFs`; the old result did not exercise ext4 storage.
- **Why it matters:** It could not serve as a page-cache performance baseline.
- **Recommendation:** Use an ext4-backed scratch directory for both baseline
  and future comparison runs.
- **Resolution:** FIXED in xtest commit `59faed8`; scratch defaults to
  `/var/tmp/iozone-scratch`.

### V-002 LoongArch external vDSO image is unavailable

- **Severity:** MEDIUM
- **Location:** `xmodules/xvdso/build.rs`, `docs/StarryX/vdso.md`
- **Problem:** The pinned Asterinas provider contains `vdso_riscv64.so` but no
  `vdso_loongarch64.so`; the LoongArch build panics before compiling xkernel.
- **Why it matters:** LoongArch build and guest cases cannot validate this
  change with the repository's default dependency configuration.
- **Recommendation:** Supply a Linux-6.8-compatible LoongArch provider through
  `XVDSO_SOURCE_DIR` in a separate vDSO/platform task.
- **Resolution:** ACCEPTED — pre-existing, documented external dependency;
  unrelated to the xcache storage path and not bypassed with a wrong-arch blob.

### V-003 Full OS-COMP contains unsupported cases

- **Severity:** MEDIUM
- **Location:** `target/xtest/riscv64/oscomp/6a78d922-03b2de70-905f/`
- **Problem:** cyclictest returned 125 because scheduler parameters are
  unsupported; lmbench exceeded its declared 600-second deadline after its
  known `mmap: Bad file descriptor` output.
- **Why it matters:** The full profile correctly returns non-zero even though
  the storage-related basic, iozone, libc, lua, netperf, and unixbench cases run.
- **Recommendation:** Address scheduler compatibility and split or tune the
  lmbench workload in separate test-compatibility tasks.
- **Resolution:** ACCEPTED — both are outside the page-cache removal; the
  supervisor reaped lmbench and continued through all later cases.

### V-004 iozone vector modes are unavailable

- **Severity:** LOW
- **Location:** `docs/benchmarks/iozone-no-page-cache.md`
- **Problem:** iozone 3.506 reports `Selected test not available on the version`
  for `-i 11 -i 12`, then emits only initial writer/rewriter values.
- **Why it matters:** Those two values are not pwritev/preadv measurements.
- **Recommendation:** Keep them labeled as fallback, or upgrade both sides of
  a future comparison to the same workload revision.
- **Resolution:** ACCEPTED — explicitly labeled in the baseline; the retained
  OS-COMP command sequence was not silently changed.

## Notes

- RISC-V clippy warnings in `xerrno`, `xdriver`, and `kernel_elf_parser` predate
  this task and do not touch the modified storage paths.
- The xtest submodule is clean at `59faed8` and matches `origin/main`.
- Raw benchmark evidence remains under ignored `target/`; the committed
  benchmark note preserves the comparison dataset and evidence checksums.
