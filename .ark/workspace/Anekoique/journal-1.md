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
