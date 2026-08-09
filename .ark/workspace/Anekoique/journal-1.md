# Anekoique — Journal 1

## Session 1: redesign xtest as a test-rootfs pipeline

**Date**: 2026-05-06
**Slug**: redesign-xtest
**Branch**: `feat/redesign-xtest`
**Base Branch**: `main`
**Start Head**: `ff9bfb8`
**Closing Commit**: <PENDING:redesign-xtest>

### Summary

Replaced the dead `xtest/` sdcard pipeline with a Docker-driven test-rootfs producer that bakes first-party C tests into a copy of the upstream Alpine rootfs and boots a kernel embedding `src/test.sh` via a new `init-test` cargo feature.

### Main Changes

| Area | Description |
|------|-------------|
| `xtest/` layout | Removed `Makefile`, `Makefile.sub`, `config/`, `fix-redis.patch`, `scripts/git_testcode.sh`. New layout `xtest/{c,scripts,Makefile,README.md}` with Docker-driven build pipeline. |
| First-party C tests | Five tests under `xtest/c/{syscall,signal,mm,fs}/` (getpid, clone_basic, kill_self, mmap_anon, open_close) plus shared `assert.h`. Cross-compiled static-musl in the contest Docker image (digest-pinned). |
| Build switch | New `init-test` cargo feature on the root crate; two `#[cfg(feature)]`-gated `include_str!` arms in `src/main.rs` choose between `init.sh` and `test.sh`. Threaded via a new `ROOT_FEATURES` Make variable in `cargo.mk`'s `cargo_build` macro. `make run` is unaffected. |
| Marker convention | `src/init.sh` and `src/test.sh` carry `# id: starry-init` / `# id: starry-test` markers so the embedded script is mechanically verifiable via `strings` on the kernel ELF. |
| `make tests` / `make run-tests` | New top-level targets. `tests` rebuilds `tests-rootfs-$ARCH.img`; `run-tests` boots the kernel against it via recursive `make` that overrides `DISK_IMG=$(TESTS_ROOTFS_IMG)` and reuses the existing `run` target — no new qemu macros. |
| `bake-image.sh` | Builds a fresh ext4 image with `mkfs.ext4` (avoids `resize2fs` failing on the upstream image's unsupported features), tar-pipes upstream rootfs contents in, copies the staged tree onto `/root/tests`. |
| Test-suite scope | OS-COMP suites (basic, busybox, libc-test, libcbench, lua, iozone, iperf, netperf, cyclictest, lmbench, ltp) were vendored, cross-built with substantial musl-vs-glibc patching, and exercised end-to-end on rv64 — surfacing real kernel issues (LTP OOM, basic/clone signal=11, libc-test mass failures). At user direction the suite half was withdrawn from this iteration and deferred to a follow-up. Per-suite cross-musl patches captured in VERIFY.md V-002 for the follow-up. |
| Documentation | Updated `AGENTS.md` Testing section; new `xtest/README.md` describing the pipeline and how to add C tests. |

### Git Commits

| Hash | Message |
|------|---------|
| _(none)_ |   |

## Session 2: Add vDSO support

**Date**: 2026-05-06
**Slug**: vdso-support
**Branch**: `feat/vdso-support`
**Base Branch**: `main`
**Start Head**: `dda046d`
**Closing Commit**: <PENDING:vdso-support>

### Summary

Added a Linux-compatible vDSO (`linux-vdso.so.1`) mapped into every user address space, serving `clock_gettime` / `gettimeofday` / `clock_getres` / `time` from a kernel-published seqlock data page (no syscall trap on the supported clocks) and replacing the legacy `SIGNAL_TRAMPOLINE` mapping with `__vdso_rt_sigreturn` inside the same image.

### Main Changes

| Area | Description |
|------|-------------|
| User-side vDSO crate | New workspace-excluded `xmodules/xvdso/` (`cdylib`, per-arch JSON target spec with `dynamic-linking = true` + `relocation-model: pic`, custom linker scripts producing a single PT_LOAD segment, `LINUX_2.6` Verdef). Built once via `make regenerate-vdso-blobs`; resulting `.so` blobs committed under `xcore/src/vdso/blobs/` and embedded in the kernel via `.incbin` inside `global_asm!`. |
| Kernel-side `xcore::vdso` | Four files: `image.rs` (blob + symbol-offset cache), `data.rs` (shared `VDSO_DATA` page + seqlock writer + boot-CPU timer-tick hook), `install.rs` (per-`execve` mapping), `mod.rs` (wiring). Public surface: `install` + `VdsoBinding`. |
| Mapping on execve | `install` maps the data page (R\|U) by phys-addr via `map_linear` — single shared page across all processes — and the code page (R-X\|U) alloc-backed-and-copied per-process; patches a `VDSO_DATA_ADDR` slot in the code page so vDSO code can find the data page position-independently. Auxv gains `AT_SYSINFO_EHDR`. |
| Time fast path | `mult/shift` derived once at boot from `axhal::time::timer_frequency()`; `__vdso_clock_gettime` and friends seqlock-read the snapshot and compute `(rdtime() * mult) >> shift` without trapping. Unsupported clock IDs fall through to `ecall`/`syscall 0` inside the vDSO image. |
| Signal trampoline migration | Atomic single-commit removal of `SIGNAL_TRAMPOLINE`, `map_trampoline`, the call site, the per-arch `signal_trampoline` asm in all four arches, and `signal_trampoline_address()`. `ProcessSignalManager.default_restorer` becomes `AtomicUsize` with `set_default_restorer`; `xcore::vdso::install` publishes the per-process `__vdso_rt_sigreturn` address. |
| auxv widening | `kernel_elf_parser` exposes `pub const AUXV_LEN: usize = 18`; `auxv_vector` and `xcore::mm::init::map_elf` reference the const so future entries are a one-line bump. |
| Timer-tick hook | `axruntime` defines `VdsoTickIf` (via `crate_interface`); `xcore::vdso::data::VdsoTickImpl` provides the impl. Boot-CPU only by `axhal::cpu::this_cpu_is_bsp()` so the seqlock stays single-writer. |
| Tests | `xtest/c/time/{vdso_clock_monotonic,vdso_gettimeofday,vdso_clock_getres,vdso_rt_sigreturn}.c`. Picked up automatically by the `xtest` pipeline's `find` discovery; no SPEC change. RV64 boot under `make run-tests` confirmed clean. |
| Simplification pass | After initial implementation, adopted the starry-vdso pattern: prebuilt blobs committed in tree (`.incbin`), single shared `VdsoData` mirror on each side (no third crate), one merged module with four files. Removed `xmodules/xvdso-data`, `xcore/build.rs`, `scripts/check-vdso-verdef.sh`, the `vdso-blob` build prerequisite. Net: ~200 lines removed, no critical-path build pipeline. |
| Documentation | New `docs/StarryX/vdso.md`; AGENTS.md Testing section updated to reference the vDSO subsystem. |

### Git Commits

| Hash | Message |
|------|---------|
| <PENDING:vdso-support> |   |

## Session 3: Port OS-COMP testsuites into xtest

### Summary

Vendored 11 OS-COMP suites into `xtest/testsuites/` with a generic build/run pipeline; both arches boot and run all suites end-to-end.

### Main Changes

| Area | Description |
|------|-------------|
| Vendored suites | `xtest/testsuites/{basic,busybox,libctest,lua,unixbench,lmbench,libcbench,iperf,netperf,cyclictest,iozone}` — each minimally vendored with a `BUILD.sh` + upstream `_testcode.sh`; iozone moved here from the bespoke `xtest/iozone/`. LTP excluded. Provenance + patches + run results + known-fails in `UPSTREAM.md`. |
| Build pipeline | `build-suites.sh` dispatches each `BUILD.sh` in Docker via a shared `scripts/build/lib/suite.sh` (suite_init/enter/stage/need/retry); `stage.sh` adds a busybox shim + `.arch` marker per suite; `bake-image.sh` symlinks the la64 musl loader the contest binaries request. |
| Run pipeline | `run-suite.sh` drives each suite under a process-group-aware `lib/timeout.sh` and maps native results to `[PASS]/[FAIL]` via per-suite adapters in `lib/suite-adapters.sh` (GROUP markers stripped, never scored); `run-all.sh` iterates and arch-skips iperf on rv64. |
| Results | rv64 + la64 run all 11 end-to-end: libctest 217/217 both; iperf 6/6 la64; netperf 4/5. cyclictest/unixbench/lmbench quarantined (uniprocessor scheduling); rv64 iperf3 server hang documented. No kernel `src/` changes. |

## Session 4: Redesign xtest as a standalone framework

**Date**: 2026-08-09
**Slug**: redesign-xtest-framework
**Branch**: `feat/redesign-xtest-framework`
**Base Branch**: `main`
**Start Head**: `914a979`
**Closing Commit**: `6a69433`

### Summary

Published a safe Rust/QEMU test framework and made StarryX consume it as one pinned submodule.

### Main Changes

| Area | Description |
|------|-------------|
| Framework | Added typed plans, ext4 injection, QEMU ownership, and JSON/TAP reports. |
| Guest runtime | Added monotonic timeouts, process groups, and descendant reaping. |
| Testsuits | Moved 11 packages behind manifests in the standalone xtest repo. |
| StarryX seam | Added one gitlink, normal-init dispatch, and a private QEMU target. |

### Git Commits

| Hash | Message |
|------|---------|
| `6a69433` | feat(xtest): publish standalone test framework |
