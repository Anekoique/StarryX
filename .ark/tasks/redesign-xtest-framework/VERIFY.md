# xtest framework redesign VERIFY

> Status: Verified after standalone-repository migration
> Feature: `redesign-xtest-framework`
> Target Task: `redesign-xtest-framework`
> Tier: `deep`
>
> Each checklist item resolves to PASS | FAIL (with explanation) | N/A (with explanation).

---

## Project Spec Compliance

- [x] `INDEX.md` enumerates all children of `specs/project/`: PASS — the directory contains only `INDEX.md`; there are no leaf project SPECs.

## Related Feature Spec Compliance

- [x] `specs/features/redesign-xtest/SPEC.md`: N/A — the PRD supersedes its Make/shell layout, privileged image mutation, `init-test` feature, and unconditional-success runner. It preserves copied rootfs isolation, static first-party cases, both architectures, and ordinary boot when `/xtest` is absent.
- [x] `specs/features/xtest/port-oscomp-suites/SPEC.md`: PASS — all eleven prior suites remain present under the standalone repository's `testsuits/` tree with package-local manifests, provenance, sources, build entries, and explicit architecture/quarantine metadata. StarryX-side vendoring and central adapters are superseded by the published gitlink contract.

## PRD Constraints

- (none separately registered): N/A — the PRD outcome is represented by the PLAN goals and constraints.

## Plan Fidelity

- [x] G-1: PASS — typed TOML models, directory-shaped argv files, strict `XTEST/1` transitions, the static guest supervisor, and JSON/TAP generation are implemented and tested.
- [x] G-2: PASS — `make test` owns build, copied ext4 injection, QEMU, deadlines, serial validation, reports, cleanup, and exit status. A real RISC-V run passed.
- [x] G-3: PASS — first-party cases live under the standalone repository's `cases/`; external integrations live under `testsuits/` behind one generic manifest/package contract. StarryX contains only a gitlink and kernel-owned runtime seams.
- [x] G-4: PASS — build-only run `6a7873bd-0c35ee48-7ae1` copied and injected the base using e2fsprogs, remained `e2fsck -fn` clean, and contained regular `0755` runner and supervisor files. Post-guest filesystem state is recorded in V-004.
- [x] G-5: PASS — selectors, `smoke`/`full`/`guest-descendant-reap` profiles, and both compiler seams are implemented. All eleven cases and the supervisor compile statically for both ISAs; current LoongArch runtime is limited by V-003.
- [x] G-6: PASS — standalone xtest commit `9435be9330039578dd978bc97d558cf07dbb7d8e` is published on both remote `main` and `feat/redesign-xtest-framework`; StarryX records that exact commit as mode-160000 `xtest` with the public GitHub URL.
- [x] G-7: PASS — exactly eleven package manifests are present. Ten RISC-V packages build through `oscomp`; the declared LoongArch-only iperf package also builds. `oscomp-smoke` executes basic, BusyBox, and Lua successfully under real RISC-V QEMU, while the remaining packages stay explicitly selectable and timeout-bounded.

## SPEC Drift

- [x] Modified feature SPECs have CHANGELOG entries: N/A — no existing feature SPEC was modified. The deep-task replacement SPEC will be extracted from the final PLAN by `/ark:commit`.

## Findings

### V-001 Real ext4 injection and RISC-V/QEMU smoke

- **Severity:** RESOLVED
- **Location:** `xtest/src/image.rs`, `xtest/src/qemu.rs`
- **Result:** PASS — e2fsprogs 1.47.4 created and verified a disposable copied ext4 image; QEMU 11.0.0 booted the current RISC-V kernel from it. Run `6a7874f4-2b0156a0-8dcb` emitted three ordered passes and `run_end 3 0 0`, wrote matching JSON/TAP, and returned host status 0.
- **Resolution:** CLOSED — this replaces the earlier host-tooling limitation.

### V-002 Guest descendant cleanup lacked runtime proof

- **Severity:** RESOLVED
- **Location:** `xtest/guest/supervisor.c`, `xmodules/xprocess/src/process.rs`
- **Result:** PASS — real-QEMU run `6a7874cd-0eb6e050-8a6e` timed out `process/00_timeout_descendant` with 124, then `process/01_descendant_reaped` verified the recorded grandchild PID returned `ESRCH` and passed. The terminal summary was `run_end 1 0 1`.
- **Resolution:** CLOSED — the overall Make status is intentionally non-zero because the first case is the timeout stimulus; the second pass is the descendant-reaping assertion.

### V-003 LoongArch kernel build is blocked by the existing external vDSO set

- **Severity:** LOW
- **Location:** `xmodules/xvdso/build.rs:42`
- **Problem:** The pinned Asterinas `linux_vdso` provider has RISC-V and x86-64 blobs but no `vdso_loongarch64.so` requested by the existing build script.
- **Why it matters:** A full current-tree LoongArch kernel/QEMU smoke cannot reach xtest, although its eleven cases and supervisor compile for LoongArch and its QEMU seam validates.
- **Resolution:** ACCEPTED — this is a pre-existing xvdso provider limitation outside the xtest redesign and needs a dedicated xvdso task.

### V-004 Guest poweroff leaves the disposable ext4 image inconsistent

- **Severity:** MEDIUM
- **Location:** existing StarryX ext4 runtime and shutdown path
- **Problem:** The build-only injected image is `e2fsck -fn` clean, but the same check after QEMU poweroff reports duplicate root entries including `dev`, `etc`, `proc`, and `tmp`, plus inode/block count mismatches.
- **Why it matters:** Serial results and pre-boot injection are valid, but the post-run image cannot be advertised as a clean storage-lifecycle artifact.
- **Recommendation:** Diagnose lwext4 directory updates plus root-filesystem sync/unmount during platform shutdown in a separate storage task.
- **Resolution:** ACCEPTED — this is a newly exposed product filesystem issue, not an xtest injection failure. The disposable image and logs remain evidence.

### V-005 Testsuit install path repeated its global prefix

- **Severity:** RESOLVED
- **Location:** `xtest/src/build.rs`, `xtest/src/plan.rs`
- **Problem:** Initial local validation installed `testsuit/local-smoke/argv` beneath a directory already scoped to `local-smoke`, producing `/xtest/bin/testsuits/local-smoke/testsuit/local-smoke/argv`.
- **Resolution:** CLOSED — `CaseOrigin::Testsuit` now retains the local case ID separately. Final run `6a78798f-26c3eb70-af41` used `/xtest/bin/testsuits/local-smoke/argv`; regression test `testsuit_install_path_uses_local_case_id_once` locks the invariant.

### V-006 Standalone repository extraction and StarryX gitlink

- **Severity:** HIGH
- **Location:** `Anekoique/xtest`, StarryX `.gitmodules`, `Makefile`, and `xtest` gitlink
- **Result:** PASS — the Rust host, POSIX guest, supervisor, cases, profiles, and package integrations were committed independently as `9435be9`; `git ls-remote` reports that hash for public `main` and the feature branch. A depth-one clone from GitHub resolved the same hash and passed all 23 locked tests. StarryX's index records one mode-160000 gitlink at that hash and `.gitmodules` retains both xtest and lwext4 URLs.
- **Resolution:** CLOSED — release ordering and the remote-reachability contract are proven.

### V-007 Complete eleven-testsuit package coverage

- **Severity:** HIGH
- **Location:** standalone `testsuits/`, manifests, package build/run entries, and profiles
- **Result:** PASS — the manifest set is exactly basic, busybox, cyclictest, iozone, iperf, libcbench, libctest, lmbench, lua, netperf, and unixbench. Full RISC-V profile build `6a789c5e-07638290-759d` produced contained packages for all ten architecture-selected suites; standalone LoongArch iperf compilation produced its contained executable. Real-QEMU run `6a789a1e-0dbab988-2952` passed basic, BusyBox, and Lua with `run_end 3 0 0`.
- **Resolution:** CLOSED — package build coverage is complete for declared architectures; runtime quarantine remains explicit rather than hidden in framework code.

## Evidence

### Real ext4/QEMU runs

- PASS — final invocation through the StarryX `xtest` gitlink and public `make test ARCH=riscv64 PROFILE=smoke`: run `6a78a1eb-18a7ff18-f375`, host exit 0, three passes, no failures/timeouts, QEMU exit 0.
- PASS — final direct standalone invocation against the same StarryX worktree: run `6a78a046-172d9fd0-d069`, host exit 0, matching JSON/TAP and three passes.
- PASS — final direct descendant cleanup run `6a78a05d-263ac9f8-d352`: expected timeout 124 for the stimulus followed by pass 0 for `process/01_descendant_reaped`, `run_end 1 0 1`, QEMU exit 0.
- PASS — `oscomp-smoke` run `6a789a1e-0dbab988-2952` passed the basic, BusyBox, and Lua packages under real RISC-V QEMU.
- PASS — `make test ARCH=riscv64 PROFILE=smoke XTEST_TIMEOUT=90` against the exact post-review source: run `6a7874f4-2b0156a0-8dcb`, host exit 0, three passes, no failures or timeouts, QEMU exit 0.
- PASS — `make test ARCH=riscv64 PROFILE=guest-descendant-reap XTEST_TIMEOUT=90`: run `6a7874cd-0eb6e050-8a6e`, expected non-zero test status, timeout 124 followed by descendant check pass 0, QEMU exit 0.
- PASS — JSON and TAP match both serial streams and internal outcomes.
- PASS — build-only image `6a7873bd-0c35ee48-7ae1` passes `e2fsck -fn`; `debugfs stat` reports `/xtest/runner.sh` and `/xtest/supervisor` as regular mode `0755` files.
- Environment — e2fsprogs 1.47.4, QEMU 11.0.0, RISC-V musl GCC 13.3.0.

### Local testsuit build and guest run

- PASS — a temporary `local-smoke` checkout supplied the documented schema-1 manifest and a generic `/bin/sh build.sh` command; it received `XTEST_ARCH=riscv64`, `XTEST_CC=riscv64-linux-musl-gcc`, and an isolated absolute `XTEST_OUT`.
- PASS — the produced executable was a static RISC-V ELF, copied to `/xtest/bin/testsuits/local-smoke/argv`, injected as a regular mode `0755` inode, and referenced by that exact path in the generated plan.
- PASS — the directory plan preserved a zero-byte first argument and the second argument `argument with spaces`; the guest fixture checked both values before printing `[local-testsuit/argv] OK`.
- PASS — real-QEMU run `6a78798f-26c3eb70-af41` reported `testsuit/local-smoke/argv` pass 0, `run_end 1 0 0`, matching JSON/TAP, QEMU exit 0, and host exit 0.
- PASS — the temporary checkout and profile were removed after the run; no fixture source was vendored into the final tree.

### Automated checks

- PASS — standalone `cargo fmt --all`, `make check`, and shell syntax checks.
- PASS — `cargo test --locked --manifest-path xtest/Cargo.toml`: 23 passed both in the StarryX gitlink and in a fresh GitHub clone of `9435be9`.
- PASS — `cargo clippy --locked --manifest-path xtest/Cargo.toml --all-targets -- -D warnings` both before publication and through the gitlink.
- PASS — `cargo test -p xprocess`: 23 integration tests passed, including `reap_to_nearest_child_subreaper`.
- PASS — `dash -n` and `sh -n` for `xtest/guest/runner.sh`.
- PASS — all eleven first-party cases and `guest/supervisor.c` compile with `-static -O2 -Wall -Wextra -Werror` for RISC-V and LoongArch; `file` identifies statically linked ELF64 binaries for both ISAs.
- PASS — `git diff --check HEAD`; no unresolved paths.
- PASS — `git submodule status xtest` resolves public commit `9435be9`; retained lwext4 metadata remains in root `.gitmodules`.
- PASS — source scans found no suite names or obsolete `testsuites` spelling in generic `xtest/src`; `guest/` references BusyBox only as its required POSIX utility provider. StarryX contains no obsolete `ROOT_FEATURES|init-test` path and xtest image construction has no privileged mount commands.
- PASS — `xtest/src/main.rs` has `#![forbid(unsafe_code)]`; the host framework contains no unsafe Rust.
- PASS — all eleven manifests validate; full RISC-V package build produced ten selected package roots and declared LoongArch-only iperf built separately.
- PASS — the final `_xtest_run` dry-run includes the selected absolute disposable disk plus virtio block/network devices and the resolved kernel feature list.

### Review checks

- PASS — final code review: APPROVE; zero CRITICAL/HIGH findings after fixing candidate-subreaper exit synchronization, negative-PID overflow, and the duplicated testsuit install prefix.
- PASS — earlier security review remains applicable to Make injection, path containment, bounded logs, process-group ownership, and testsuit build timeout. The target-compiled supervisor adds no host command-construction surface.

## Notes

- Completed runs use immutable `target/xtest/<arch>/<profile>/<run-id>/` directories; construction occurs in a sibling `.partial` directory and host Cargo state uses `target/xtest-host/`.
- The `guest-descendant-reap` profile is intentionally excluded from ordinary `full`, because a correct execution contains one timeout and therefore returns non-zero.
- V-001 through V-005 preserve the earlier in-worktree design/runtime audit.
  V-006 and V-007 record the post-extraction publication, gitlink, package,
  fresh-clone, and real-QEMU evidence that closes the amended scope.
- The superseded ordinary xtest tree remains recoverable from Git history; the
  final StarryX tree contains only the published gitlink.
