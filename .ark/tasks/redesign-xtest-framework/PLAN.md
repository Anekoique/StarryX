# xtest framework redesign PLAN

> Status: Implemented and verified
> Feature: `redesign-xtest-framework`
> Owner: Executor

---

## Summary

Publish the verified Rust host runner, POSIX guest runner, target-compiled guest
supervisor, first-party cases, profiles, and eleven testsuit packages from the
standalone `Anekoique/xtest` repository. StarryX loads that repository at
`xtest/` through one gitlink and retains only the kernel/QEMU seam, normal-init
bundle dispatch, and Linux process semantics required by the supervisor. The
host resolves conventional first-party cases and package-local testsuit
manifests into an immutable plan and bundle, injects the bundle into a copied
ext4 rootfs without privileged mounts, launches the selected StarryX checkout,
validates versioned serial events, and produces JSON/TAP reports.

> Deep tier: REVIEW findings are folded into this PLAN in place before EXECUTE — there is no iteration history to track here.

---

## Spec

[**Goals**]

- G-1: Define stable case, plan, bundle, event, and report contracts.
- G-2: Run StarryX system tests through one reliable host command.
- G-3: Separate first-party cases from package-local external testsuit integrations.
- G-4: Build test images without privileged mounts or base-image mutation.
- G-5: Support focused and profiled runs on RISC-V and LoongArch.
- G-6: Version and release the complete framework independently from StarryX,
  with StarryX consuming an explicit published xtest gitlink.
- G-7: Preserve buildable/selectable integrations for all eleven prior
  testsuits without introducing their names or policies into generic host or
  guest framework code.

[**Non-goals**]

- NG-1: Do not put concrete suite names, output parsing, compatibility patches,
  or quarantine policy in generic `src/` or `guest/` framework code.
- NG-2: Do not replace Rust unit tests, doc tests, or crate integration tests.
- NG-3: Do not introduce provider plugins, multiple VM backends, or Windows support.
- NG-4: Do not make xtest responsible for publishing, merging, or otherwise
  managing the selected kernel repository.

[**Architecture**]

```text
          STANDALONE Anekoique/xtest REPOSITORY

  cases/                 testsuits/                 profiles/
  first-party C    11 package-local manifests      declarative case/
                        + sources/adapters          testsuit selection
       └───────────────────┬────────────────────────────┘
                           ▼
                   ┌───────────────┐
                   │ Rust host     │
                   │ resolve/build │
                   │ bundle/image  │
                   │ QEMU/report   │
                   └───────┬───────┘
                           │ XTEST_KERNEL_ROOT
                           ▼
                STARRYX REPOSITORY / WORKTREE

        xtest/ gitlink    Make _xtest_run     starry init guard
                │                │                    │
                └────────────────┴──────────┬─────────┘
                                           ▼
                              copied rootfs + QEMU guest
                                           │
                                  runner → supervisor
                                           │
                                  XTEST/1 serial events
                                           ▼
                              serial.log + JSON + TAP
```

The framework is one standalone host crate in its own Git repository, not part
of the StarryX bare-metal Cargo workspace. Its modules own configuration,
resolution, building, image injection, process lifecycle, protocol validation,
and reports. Module boundaries are internal organization, not extension points.
The host keeps two explicit roots: `framework_root` is the xtest checkout that
contains sources and profiles; `kernel_root` is the StarryX checkout/worktree
that provides the base rootfs and `_xtest_run` Make seam. In a StarryX checkout
the usual relationship is `<kernel_root>/xtest == <framework_root>`, but the
host does not derive one root by string-concatenating the other.

`cases/` in the standalone repository contains StarryX-owned system cases. A `.c` file is a case by
convention; its relative path without `.c` is its stable ID. An optional
same-name `.toml` sidecar overrides arguments, timeout, or architecture
support.

`testsuits/<name>/` contains a package-local `xtest.toml`, build entry, required
source/data, provenance, and any suite-specific compatibility or result
normalization. The eleven retained package names are `basic`, `busybox`,
`cyclictest`, `iozone`, `iperf`, `libcbench`, `libctest`, `lmbench`, `lua`,
`netperf`, and `unixbench`. The framework applies one generic package contract
uniformly; generic `src/` and `guest/` contain no suite names, patches, output
parsers, or skip policy. StarryX vendors none of these files because the entire
standalone repository is consumed as a submodule.

`profiles/` contains declarative case and testsuit selection. `*` selects
all first-party cases and `<group>/*` selects one first-party group. Every other
case ID is exact. Testsuit names are exact subdirectory names. Architecture
restrictions and default quarantine live in manifests/profiles; every retained
suite remains selectable through an explicit profile even when it is excluded
from the ordinary batch profile.

The host resolves a profile and optional CLI case filter before building. It
then creates a unique run under
`target/xtest/<arch>/<profile>/<run-id>.partial/`, materializes all inputs and
results there, and atomically renames it to `<run-id>/` only after terminal
reports are written. Previous run directories are immutable and never reused.
The StarryX checkout uses `target/xtest-host/` as the host Cargo target so host
compiler artifacts cannot be confused with test-run artifacts. Each run
contains:

```text
<run-id>/
├── bundle/xtest/
│   ├── runner.sh
│   ├── supervisor
│   ├── run-id
│   ├── plan/
│   │   ├── 0000/
│   │   │   ├── id
│   │   │   ├── timeout
│   │   │   ├── program
│   │   │   └── argv/{0000,0001,...}
│   │   └── 0001/...
│   └── bin/{cases,testsuits}/...
├── rootfs.img
├── serial.log
├── report.json
└── report.tap
```

The directory-shaped plan avoids shell evaluation and escaping. The guest
constructs argv from numbered files and never evaluates plan content as shell
source. Case IDs are restricted to lowercase ASCII letters, digits, `.`, `_`,
`-`, and `/`, with no empty, absolute, or parent components.

First-party programs are installed at `/xtest/bin/cases/<case-id>`. A testsuit
build writes one sealed package tree beneath its isolated `XTEST_OUT`.
Entrypoints and optional working directories declared in
`testsuits/<name>/xtest.toml` must be normalized relative paths beneath that
tree. Before copying the package to `/xtest/bin/testsuits/<name>/`, the host
rejects absolute or empty paths, `..` components, symlinks, sockets/devices,
and any canonical path that escapes `XTEST_OUT`. This lets complex suites carry
their scripts and data without teaching the framework their layout. Argument
values may be empty or contain spaces, but must be UTF-8 and contain no NUL,
CR, LF, or other ASCII control characters; numbered argument files preserve
boundaries.

Selected testsuit source and build commands are trusted host/guest code, not a
sandbox boundary. `XTEST/1` is reserved for the framework, and test programs
must not emit lines beginning with it.

Before copying the base image, the host preflights `debugfs` and `e2fsck`
versions and required operations against a tiny disposable fixture and the
selected base image. It copies the immutable base rootfs to the partial run directory and
uses scripted e2fsprogs operations to create `/xtest` and write the complete
bundle. After injection it runs `e2fsck -pf`; only exit statuses 0 (clean) and
1 (corrected) are accepted. It then verifies the repaired final image:
`/xtest/runner.sh` and `/xtest/supervisor` are regular files with mode `0755`,
the runner has the expected SHA-256, and every required plan and program path
has the expected type and mode. The shared base image is never opened for
writing.

The normal embedded init script contains one test-bundle dispatch guard: when
`/xtest/runner.sh` is a regular executable file and `/xtest/plan` and
`/xtest/run-id` exist with the expected types, it executes that runner;
otherwise it follows the normal init path. Test selection is therefore a
rootfs property, not a Cargo feature. The separate `init-test` feature and test
init script do not exist.

The guest emits only the following machine-event forms:

```text
XTEST/1 <run-id> run_start <case-count>
XTEST/1 <run-id> case_start <case-id>
XTEST/1 <run-id> case_end <case-id> pass 0
XTEST/1 <run-id> case_end <case-id> fail <exit-code>
XTEST/1 <run-id> case_end <case-id> timeout <exit-code>
XTEST/1 <run-id> run_end <passed> <failed> <timed-out>
XTEST/1 <run-id> run_error <code>
```

Only events with the generated run ID are protocol input. All other serial
lines are preserved as diagnostics unless they begin with the reserved
`XTEST/1` prefix. The host validates order, counts, unique terminal outcomes,
and the final summary before accepting a run. Any malformed reserved line,
wrong-run event, duplicate event, or valid event before `run_start` is a
protocol error rather than case output.

At startup the guest runner requires the injected static supervisor. For each
case the supervisor enables `PR_SET_CHILD_SUBREAPER`, forks the program into a
new session, and polls it against `CLOCK_MONOTONIC` without shell job control.
On timeout it writes a unique marker, sends TERM then KILL to the complete
process group, and waits for direct and adopted descendants. Normal completion
also kills and reaps any residual descendants before the runner starts the next
case. A missing supervisor emits terminal `run_error` without starting a case.
The host wall-clock deadline remains the final authority over the complete
Make/QEMU process group.

StarryX therefore implements `PR_SET_CHILD_SUBREAPER` and
`PR_GET_CHILD_SUBREAPER`, reparents an orphan to its nearest live subreaper
before init, implements signal-zero existence checks and Linux-compatible
group-kill success results, and checks the child-exit predicate while entering
the `wait4` wait queue to avoid a lost wakeup.

The guest runner exits after `run_end` in batch mode. Returning from StarryX's
init task terminates the QEMU machine through the existing platform shutdown
path. The host waits for that exit and forcibly terminates the QEMU process
group only on deadline, protocol failure, or shutdown-grace expiry.

[**Data Structure**]

```rust
struct RunConfig {
    framework_root: PathBuf,
    kernel_root: PathBuf,
    output_root: PathBuf,
    arch: Arch,
    profile: String,
    case: Option<String>,
}

enum Arch {
    Riscv64,
    LoongArch64,
}

struct CaseMetadata {
    args: Vec<String>,
    timeout_secs: u64,
    architectures: Vec<Arch>,
}

struct Profile {
    cases: Vec<String>,
    testsuits: Vec<String>,
    run_timeout_secs: u64,
}

struct TestsuitManifest {
    schema: u32,
    name: String,
    build: CommandSpec,
    cases: Vec<TestsuitCase>,
}

struct CommandSpec {
    program: String,
    args: Vec<String>,
}

struct TestsuitCase {
    id: String,
    program: String,
    working_directory: Option<String>,
    args: Vec<String>,
    timeout_secs: u64,
    architectures: Vec<Arch>,
}

struct ResolvedCase {
    id: String,
    program: PathBuf,
    args: Vec<String>,
    timeout_secs: u64,
    origin: CaseOrigin,
}

enum CaseOrigin {
    FirstParty(PathBuf),
    Testsuit {
        package: String,
        package_root: PathBuf,
        program: PathBuf,
        working_directory: Option<PathBuf>,
    },
}

struct TestPlan {
    version: u32,
    run_id: String,
    arch: Arch,
    cases: Vec<PlanCase>,
}

struct PlanCase {
    id: String,
    program: String,
    args: Vec<String>,
    timeout_secs: u64,
}

enum TestEvent {
    RunStart { count: usize },
    CaseStart { id: String },
    CaseEnd { id: String, outcome: CaseOutcome },
    RunEnd { passed: usize, failed: usize, timed_out: usize },
    RunError { code: String },
}

enum CaseOutcome {
    Pass,
    Fail(i32),
    Timeout(i32),
}

struct RunSummary {
    run_id: String,
    arch: Arch,
    profile: String,
    outcomes: Vec<CaseResult>,
    status: RunStatus,
}

enum RunStatus {
    Passed,
    TestFailed,
    HostTimeout,
    ProtocolError,
    GuestError,
    UnexpectedQemuExit,
    ShutdownTimeout,
}
```

[**API Surface**]

Canonical command:

```text
make test ARCH=<riscv64|loongarch64> [PROFILE=smoke] [CASE=<id>]
          [INTERACTIVE=0|1] [XTEST_TIMEOUT=<seconds>]
```

Host-tool commands:

```text
xtest build --kernel-root <path> --arch <arch> [--profile <name>]
            [--case <id>]
xtest run   --kernel-root <path> --arch <arch> [--profile <name>]
            [--case <id>]
            [--interactive] [--timeout <seconds>]
xtest clean [--arch <arch>]
```

Internal module seams:

```rust
fn resolve_cases(config: &RunConfig) -> Result<Vec<ResolvedCase>>;
fn build_bundle(config: &RunConfig, cases: &[ResolvedCase]) -> Result<TestPlan>;
fn inject_rootfs(config: &RunConfig, plan: &TestPlan) -> Result<PathBuf>;
fn run_qemu(config: &RunConfig, plan: &TestPlan, image: &Path) -> Result<RunSummary>;
fn write_reports(config: &RunConfig, summary: &RunSummary) -> Result<()>;
```

Testsuit build environment:

```text
XTEST_ARCH=<riscv64|loongarch64>
XTEST_CC=<architecture musl C compiler>
XTEST_OUT=<absolute package output directory>
XTEST_FRAMEWORK_ROOT=<absolute standalone xtest checkout>
```

The host runs package build commands from `testsuits/<name>/`. Package-specific
toolchain containers are permitted only through that build command and must be
non-privileged; the generic host image builder never invokes Docker. The build
must leave a self-contained regular-file/directory tree in `XTEST_OUT`.

Internal Make seam:

```text
make _xtest_run ARCH=<arch> XTEST_DISK_IMG=<absolute copied image>
```

`_xtest_run` is the only framework-to-kernel/QEMU boundary and is invoked with
`make -C <kernel-root>`. Its recipe performs
one recursive invocation equivalent to
`$(MAKE) ARCH=$(ARCH) BLK=y NET=y FEATURES=$(QEMU_FEATURES)
DISK_IMG=$(XTEST_DISK_IMG) run`, ensuring that kernel features, device model,
and disk image are configured consistently for both build and launch. The host
never invokes `build` and `justrun` separately. A dry-run validation inspects
the final QEMU argv and proves that it contains the disposable image and the
required block/network devices.

[**Constraints**]

- C-1: @source-scan: `#![forbid(unsafe_code)] @ xtest/src/**/*.rs`
  The host framework contains no unsafe Rust.
- C-2: @test-binding: plan_rejects_duplicate_case_ids
  Every resolved case ID is valid and globally unique before any build starts.
- C-3: @test-binding: plan_argv_round_trip
  The directory plan preserves argument boundaries without shell evaluation.
- C-4: @test-binding: protocol_rejects_invalid_transition
  A run passes only after one valid ordered terminal event for every planned case.
- C-5: @test-binding: image_builder_preserves_base_image
  Image preparation never writes to the shared base rootfs.
- C-6: @tool: `dash -n xtest/guest/runner.sh`
  The guest runner remains POSIX sh and contains no source or build discovery.
- C-7: @source-scan: `OS-COMP|oscomp|basic|busybox|cyclictest|iozone|iperf|libcbench|libctest|lmbench|lua|netperf|unixbench @ xtest/src xtest/guest`
  Framework code contains no concrete external-suite knowledge.
- C-8: @source-scan: `testsuites @ xtest Makefile AGENTS.md`
  The obsolete vendored directory name is absent from live framework paths.
- C-9: @tool: `git submodule status`
  Root submodule metadata describes every retained Git link without errors.
- C-10: @test-binding: host_timeout_reaps_process_group
  A timed-out batch run terminates and waits for the complete QEMU process group.
- C-11: @test-binding: batch_failure_sets_nonzero_status
  Any failed, timed-out, or invalid run returns a non-zero host status.
- C-12: @test-binding: report_tap_matches_json
  TAP and JSON reports represent the same validated internal outcomes.
- C-13: @source-scan: `ROOT_FEATURES|init-test @ Makefile starry scripts/make`
  Test boot selection does not use the Cargo feature graph.
- C-14: @test-binding: normal_init_ignores_absent_test_bundle
  An ordinary rootfs without `/xtest/runner.sh` follows the existing init path.
- C-15: @source-scan: `losetup|mount |--privileged @ xtest/src xtest/guest`
  The generic framework requires no privileged image-building operation.
  Package-local builds may select a non-privileged pinned toolchain container.
- C-16: @judgment
  Completed runs live under immutable
  `target/xtest/<arch>/<profile>/<run-id>/` directories; partial runs use the
  sibling `.partial` suffix and host Cargo artifacts live under
  `target/xtest-host/`. All remain untracked.
- C-17: @judgment
  A guest timeout terminates and reaps the complete case process group before
  the next case starts.
- C-18: @test-binding: testsuit_artifact_stays_within_out
  A testsuit package tree, program, and working directory cannot escape
  `XTEST_OUT` through syntax, symlinks, special files, or non-regular
  entrypoints; copied assets preserve their relative package layout.
- C-19: @test-binding: run_artifacts_are_immutable
  A repeated run creates a new atomic run directory and never overwrites a
  prior terminal report.
- C-20: @test-binding: make_seam_uses_disposable_image
  The internal Make seam propagates the selected architecture, required
  features/devices, and exact disposable image into the final QEMU invocation.
- C-21: @test-binding: image_tools_preflight_required_contract
  Image construction fails before touching a base image unless debugfs/e2fsck
  versions and required fixture operations are supported.
- C-22: @test-binding: reap_to_nearest_child_subreaper
  Orphaned descendants attach to the nearest enabled child subreaper before
  falling back to init.
- C-23: @judgment
  The guest supervisor observes Linux-compatible child-subreaper, signal-zero,
  process-group signal, and wait/reap semantics in a real StarryX guest.
- C-24: @test-binding: testsuit_install_path_uses_local_case_id_once
  A testsuit's global case ID is `testsuit/<name>/<local-id>`; its declared
  entrypoint and package assets are installed exactly once beneath
  `/xtest/bin/testsuits/<name>/` without repeating the global prefix.
- C-25: @judgment
  StarryX contains a root `.gitmodules` entry and gitlink for
  `https://github.com/Anekoique/xtest.git`; the referenced commit exists on the
  remote before the StarryX gitlink is published.
- C-26: @test-binding: all_supported_testsuits_have_manifests
  The standalone repository contains valid package manifests for exactly the
  eleven retained suites: basic, busybox, cyclictest, iozone, iperf, libcbench,
  libctest, lmbench, lua, netperf, and unixbench.
- C-27: @source-scan: `suite-adapters|SUITE_SKIP|run-suite @ xtest/src xtest/guest`
  Suite-specific build/result/quarantine logic remains confined to
  `xtest/testsuits/<name>/` and declarative profiles.

---

## Runtime

[**Main Flow**]

1. `make test` in StarryX ensures only the immutable architecture base rootfs
   exists, then invokes the host crate from the `xtest/` gitlink with kernel
   `RUSTFLAGS` removed, an explicit kernel root, and
   `CARGO_TARGET_DIR=target/xtest-host`.
2. The host independently canonicalizes its standalone framework root and the
   selected StarryX kernel root, then validates architecture, profile, optional
   case filter, compiler, base rootfs, and every selected package manifest. The
   non-privileged image-tool contract is preflighted before the base image is copied.
3. First-party cases are discovered from `<framework-root>/cases/**/*.c`;
   sidecars and selected testsuit manifests are parsed and merged into globally
   unique cases.
4. First-party cases are statically cross-compiled with the architecture musl
   compiler. Each selected testsuit receives the same generic build environment
   and writes a sealed package tree only into its isolated `XTEST_OUT`. The host
   validates and copies that tree without interpreting suite output.
5. The host creates a unique run ID and `.partial` run directory, materializes
   the directory-shaped TestPlan, copies validated binaries, cross-compiles
   `guest/supervisor.c`, copies `guest/runner.sh`, and seals the TestBundle.
6. The host copies `rootfs-<arch>.img`, creates `/xtest`, injects the bundle with
   e2fsprogs, and checks the disposable image.
7. The host invokes `make -C <kernel-root> _xtest_run` with the selected
   architecture and exact disposable image. That single Make seam recursively
   builds and runs with `BLK=y`, `NET=y`, and `FEATURES=$(QEMU_FEATURES)` in a
   new process group.
8. The normal embedded init script detects `/xtest/runner.sh` and transfers
   control to it; ordinary rootfs images continue down the existing init path.
9. The guest runs plan entries through per-case supervisors in dedicated
   process groups. Each supervisor enforces the timeout, reaps direct and
   adopted descendants, and returns only after the case group is gone. The
   runner continues after ordinary case failure/timeout and emits run-ID-scoped
   events while ordinary case output remains diagnostic.
10. After `run_end`, the guest exits. StarryX returns from its init task and uses
    the existing platform terminate path, causing QEMU and its Make parent to exit.
11. The host validates QEMU lifecycle and event state, writes serial/TAP/JSON
    artifacts, atomically renames the completed `.partial` directory, prints a
    concise summary, and exits according to `RunStatus`.

[**Failure Flow**]

1. Invalid configuration, duplicate IDs, missing tools, a missing/malformed
   testsuit package, an unavailable package toolchain, or a case build failure
   stops before the base rootfs is copied.
2. Bundle or image-injection failure leaves only the identifiable `.partial`
   run directory; the base rootfs and previous completed reports remain unchanged.
3. A guest case failure records `fail`, increments the summary, and continues to
   later cases. A supervisor-enforced per-case timeout terminates and reaps the
   case process group and adopted descendants, records `timeout`, and continues.
4. Invalid, duplicate, early, out-of-order, wrong-run-ID, reserved-prefix, or
   summary-mismatched events
   make the host terminate the QEMU process group and report `ProtocolError`.
5. Guest preflight failure emits `run_error` and reports `GuestError`; QEMU exit
   before any valid terminal event reports `UnexpectedQemuExit`.
6. Host wall-clock expiry terminates and waits for the QEMU process group and
   reports `HostTimeout`, independent of guest scheduling state.
7. A valid `run_end` without QEMU exit inside the shutdown grace period reports
   `ShutdownTimeout` after bounded process-group cleanup.
8. Interactive mode preserves console ownership and does not create a passing
   verification result merely because the user exits QEMU.

[**State Transitions**]

- `Configured → Resolved` after all tools, manifests, profiles, filters, and IDs validate.
- `Resolved → Built` after every selected case produces its declared artifact.
- `Built → Bundled` after the immutable plan tree and payload are materialized
  inside a unique `.partial` directory.
- `Bundled → ImageReady` after injection and filesystem validation succeed.
- `ImageReady → Running` after the QEMU process group and serial reader start.
- `Running → Completed` after valid `run_end` and bounded QEMU shutdown.
- `Running → Failed` on protocol failure, host timeout, or unexpected QEMU exit.
- `Completed → Reported` after JSON/TAP/serial artifacts are written and the
  run directory is atomically renamed to its immutable final name.

---

## Implementation

[**Phase 1 — Extract the verified framework into the standalone repository**]

- Treat the current `feat/redesign-xtest-framework` worktree implementation as
  the source baseline; do not regress to the previously pushed suite-aware
  shell framework.
- Move the standalone Cargo crate, guest runner/supervisor, first-party cases,
  profiles, README, and tests from the worktree's ordinary `xtest/` directory to
  the root of `Anekoique/xtest`.
- Split `RunConfig.repo_root` into canonical `framework_root`, `kernel_root`, and
  `output_root` values. Derive `framework_root` from the crate location and
  accept `--kernel-root`/`XTEST_KERNEL_ROOT` explicitly.
- Preserve `#![forbid(unsafe_code)]`, immutable run directories, protocol state
  validation, bounded process-group ownership, JSON/TAP generation, and all
  existing unit tests.

Acceptance: the host crate passes test/clippy both when invoked from the xtest
repository and through a StarryX `xtest/` checkout; no code assumes a nested
`<repo>/xtest/xtest` path.

[**Phase 2 — Make StarryX consume xtest only as a gitlink**]

- Replace StarryX's ordinary `xtest/` files with a gitlink to
  `https://github.com/Anekoique/xtest.git`; retain root `.gitmodules` metadata
  for both xtest and the existing lwext4 gitlink.
- Keep only StarryX-owned integration in the kernel worktree: the normal-init
  `/xtest` bundle guard, `_xtest_run` Make seam, public `make test` wrapper, and
  child-subreaper/signal/wait semantics.
- Make the wrapper invoke `cargo run --manifest-path $(XTEST_DIR)/Cargo.toml`
  with the StarryX checkout supplied as kernel root and host Cargo output routed
  under the StarryX `target/` tree.
- Remove the alternate `starry/src/test.sh`, `init-test`, `ROOT_FEATURES`, and
  old `make tests`/`make run-tests` pipeline.

Acceptance: `git submodule status xtest` resolves to a commit present on the
public xtest remote; ordinary `make run` remains unchanged and `make test`
reaches the standalone host runner through the gitlink.

[**Phase 3 — Generalize the testsuit package contract**]

- Keep one build command per selected package and one or more manifest cases,
  but treat `XTEST_OUT` as a sealed package tree rather than a single binary.
- Add optional case working-directory metadata relative to the package root.
  Materialize it in the directory-shaped plan and make the supervisor `chdir`
  only after validating the injected absolute guest path.
- Recursively validate/copy package directories and regular files; reject
  symlinks, sockets, devices, traversal, non-executable entrypoints, and output
  root replacement.
- Keep package scripts/data at `/xtest/bin/testsuits/<name>/...`; a declared
  entrypoint is resolved beneath that fixed guest root exactly once.
- Extend unit/failure tests for package assets, cwd containment, entrypoint
  containment, repeated builds, and cleanup after package build timeout.

Acceptance: a local fixture containing an executable plus scripts/data builds,
injects, runs from its declared cwd, preserves empty/spaced argv, and cannot
escape `XTEST_OUT` or the guest package root.

[**Phase 4 — Port all eleven testsuits as package-local integrations**]

- Rename the old `testsuites/` spelling to `testsuits/` and retain source
  provenance and license material in the standalone repository.
- Convert the existing build knowledge for `basic`, `busybox`, `cyclictest`,
  `iozone`, `iperf`, `libcbench`, `libctest`, `lmbench`, `lua`, `netperf`, and
  `unixbench` into one `xtest.toml` and package-local build/run entry per suite.
- Reuse already validated cross-build flags and patches, including libtirpc,
  numactl, dynamic-loader, static-link, and cross-architecture clean-build
  requirements. Package-local scripts may normalize their suite's exit status;
  the generic host/guest code may not parse suite output.
- Use a single optional non-privileged pinned toolchain-container helper for
  packages that cannot build natively on the host. It receives only the package
  source plus isolated `XTEST_OUT`; rootfs image injection never enters it.
- Add declarative `oscomp-smoke`, `oscomp`, and `oscomp-quarantined` profiles.
  RISC-V iperf restrictions and cyclictest/unixbench/lmbench quarantine live in
  manifests/profiles, never in framework source.

Acceptance: every manifest validates, all eleven package builds are attempted,
and every successfully built package produces a contained runnable entrypoint.
The previously green suites retain their observed RISC-V behavior; quarantined
suites remain explicitly selectable and bounded by the supervisor.

[**Phase 5 — Preserve image, guest, QEMU, and report guarantees**]

- Preserve non-privileged e2fsprogs preflight/injection, copied base-image
  immutability, inode/mode/hash verification, and atomic run publication.
- Preserve the POSIX runner, target supervisor, child-subreaper semantics,
  per-case monotonic timeout, descendant cleanup, and `XTEST/1` state machine.
- Invoke `make -C <kernel-root> _xtest_run` in its own process group, stream
  serial output, enforce host timeout and shutdown grace, and produce reports
  from one validated `RunSummary`.
- Update standalone xtest and StarryX documentation around repository ownership,
  initialization, commands, manifests, package assets, and failure semantics.

Acceptance: the existing real ext4/QEMU smoke and guest-descendant-reaping
scenarios pass from a StarryX checkout whose xtest path is a gitlink.

[**Phase 6 — Cross-repository release and verification**]

- Run formatting, standalone xtest test/clippy, shell syntax, source scans,
  package-manifest validation, root Cargo checks, and the available kernel build.
- Re-run successful, failing, timeout, protocol, repeated-run, package traversal,
  and ext4 fixture checks after extraction.
- Build every testsuit for each declared architecture (ten RISC-V packages and
  the LoongArch-only iperf package); boot the ordinary smoke and bounded
  `oscomp-smoke` profiles. Record runtime quarantine and architecture
  restrictions honestly.
- Push the standalone xtest commit first, verify it through `git ls-remote`, then
  update and publish the StarryX gitlink.
- Verify a fresh recursive clone initializes xtest and reaches the same host
  tests without relying on a local-path submodule remote.
- Refresh VERIFY so historical in-tree evidence is distinguished from
  post-extraction proof and no new item remains pending before commit.

Acceptance: every validation row below is resolved with reproducible evidence,
both repositories are clean, and the StarryX gitlink is remotely reachable.

---

## Trade-offs

- T-1: Rust host runner vs shell. Rust adds a small standalone build but gives
  typed configuration, safe process ownership, concurrent output handling,
  deterministic reports, and testable cleanup; another shell framework would
  recreate the current failure mode.
- T-2: Rootfs runtime dispatch vs Cargo feature. A four-line `/xtest` presence
  guard slightly changes the embedded normal script but removes test-only product
  features and lets one kernel binary boot either rootfs without behavior change
  when the bundle is absent.
- T-3: `debugfs` injection vs privileged mount. e2fsprogs becomes an explicit host
  dependency, but image creation is non-root, non-Docker, auditable, and does not
  require loop-device cleanup. `mke2fs -d` remains a future standalone-disk option.
- T-4: Custom serial events vs TAP on the wire. The small versioned protocol can
  coexist with arbitrary kernel/test logs; strict TAP cannot. TAP remains a
  standards-based host report generated from validated events.
- T-5: One standalone xtest gitlink vs framework files in StarryX. A single
  published revision keeps kernel history and checkout size small and lets the
  framework evolve independently, at the cost of cross-repository release
  ordering and an explicit recursive initialization step.
- T-6: Generic testsuit manifest vs provider plugins. A fixed command-and-cases
  schema is enough to contribute test data; traits, dynamic discovery, or
  suite-specific modules would expand the framework without a second backend.
- T-7: One `_xtest_run` Make seam vs separately invoking `make build` and
  `make justrun`. The internal recursive target preserves existing platform
  arguments while binding feature, device, architecture, and disposable-image
  configuration to the same build/run invocation owned by the host process group.
- T-8: Package-local suite adapters vs suite logic in the framework. Local
  manifests/scripts preserve difficult upstream build and result semantics while
  keeping the Rust/POSIX core generic; the cost is that each package owns and
  tests its compatibility layer instead of receiving framework special cases.

---

## Validation

[**Unit Tests**]

- V-UT-1: Parse valid/default case metadata and reject invalid IDs, timeouts, and arches.
- V-UT-2: Resolve exact, `*`, and group profile selectors in stable order.
- V-UT-3: Reject duplicate IDs across first-party and testsuit cases.
- V-UT-4: Round-trip arguments through the directory-shaped plan without `eval`.
- V-UT-5: Parse only matching `XTEST/1` run-ID events and reject invalid transitions.
- V-UT-6: Produce equivalent JSON and TAP outcomes from one RunSummary.
- V-UT-7: Preserve the base image while generating a disposable image plan.
- V-UT-8: Map pass, failure, timeout, protocol, QEMU, and shutdown states to exits.
- V-UT-9: Detect absent selected testsuit checkout before executing its build command.
- V-UT-10: Reject testsuit artifact traversal, symlink escape, non-regular output,
  and argument control characters while preserving spaces and empty arguments.
- V-UT-11: Create a new `.partial`/final run path per invocation and never reuse a
  previous run directory.
- V-UT-12: Reject duplicate, early, wrong-run, malformed reserved-prefix, and
  post-terminal protocol events; accept terminal `run_error` as guest failure.
- V-UT-13: Resolve independent framework/kernel roots and reject missing,
  aliased, or malformed kernel seams without assuming a nested checkout.
- V-UT-14: Validate and copy a complete testsuit package tree plus declared cwd,
  rejecting traversal, symlinks, special files, and non-executable entrypoints.
- V-UT-15: Enumerate the exact eleven supported package manifests and reject
  malformed, missing, or duplicate package identities.

[**Integration Tests**]

- V-IT-1: `cargo test --manifest-path xtest/Cargo.toml` passes.
- V-IT-2: xtest clippy and formatting checks pass with warnings denied.
- V-IT-3: `dash -n xtest/guest/runner.sh` passes.
- V-IT-4: First-party cases build statically for each available supported compiler.
- V-IT-5: A disposable ext4 fixture receives the complete bundle without privilege.
- V-IT-6: `git submodule status` succeeds from the repository root.
- V-IT-7: RISC-V smoke run emits valid events, reports, and zero host status.
- V-IT-8: LoongArch smoke run emits valid events, reports, and zero host status.
- V-IT-9: Default StarryX build and ordinary init path remain operational.
- V-IT-10: Framework source scans contain no external-suite-specific behavior.
- V-IT-11: The `guest-descendant-reap` profile times out a child/grandchild case
  under real QEMU, then the following case proves the recorded descendant PID
  no longer exists and passes.
- V-IT-12: `make -n _xtest_run` final QEMU argv contains the selected disposable
  disk, block/network devices, architecture, and required kernel features.
- V-IT-13: Image-tool preflight and post-injection checks pass for a tiny fixture
  and each available architecture base image.
- V-IT-14: A temporary local testsuit fixture receives the generic build
  environment, preserves empty and spaced argv entries, is injected at the
  documented guest path, and passes under real RISC-V QEMU.
- V-IT-15: The standalone xtest repository passes test/clippy both directly and
  when invoked through StarryX's `xtest/` gitlink with an explicit kernel root.
- V-IT-16: All eleven retained testsuit packages are attempted for RISC-V and
  produce either a contained runnable package or an explicitly recorded
  architecture/toolchain limitation.
- V-IT-17: A fresh `git clone --recurse-submodules` of StarryX resolves the
  published xtest revision and passes layout/host tests without local remotes.
- V-IT-18: The ordinary first-party smoke profile and the bounded
  `oscomp-smoke` profile (basic, BusyBox, and Lua) execute under RISC-V QEMU;
  every other retained package builds for its declared architecture and remains
  explicitly selectable under the supervisor's bounded runtime contract.

[**Failure / Robustness**]

- V-F-1: A failing case is reported, later cases run, and the host exits non-zero.
- V-F-2: A hanging run reaches host timeout, reaps the process group, and leaves logs.
- V-F-3: Malformed or mismatched events fail as protocol errors and terminate QEMU.
- V-F-4: QEMU exit before `run_end` fails as unexpected exit.
- V-F-5: Missing compiler, e2fsprogs, rootfs, profile, or testsuit fails before boot.
- V-F-6: Image-injection failure leaves the base hash and completed runs unchanged.
- V-F-7: `run_end` followed by shutdown stall fails after bounded cleanup.
- V-F-8: A missing guest supervisor emits `run_error`, starts no case, and
  returns `GuestError` rather than a protocol pass.
- V-F-9: Unsupported debugfs operations, runner inode/mode/hash mismatch, or
  e2fsck status greater than 1 fail before boot.
- V-F-10: A testsuit package that replaces `XTEST_OUT`, emits a symlink/special
  file, declares an escaping cwd, or exceeds build timeout fails before image copy.

[**Edge Cases**]

- V-E-1: Empty profile and unmatched `CASE` selection fail rather than pass vacuously.
- V-E-2: Same basename in different groups produces distinct path-derived IDs.
- V-E-3: Arguments containing spaces and empty arguments preserve boundaries.
- V-E-4: Wrong-run-ID lookalike console lines remain diagnostics.
- V-E-5: Interactive exit never generates a passing batch report.
- V-E-6: Repeated runs do not overwrite another profile or architecture's artifacts.
- V-E-7: Two runs with the same arch/profile retain distinct immutable final
  directories and use a separate `target/xtest-host` Cargo cache.
- V-E-8: Direct xtest checkout invocation and StarryX-submodule invocation use
  the same framework sources while keeping their selected kernel/output roots distinct.

[**Acceptance Mapping**]

| Goal / Constraint | Validation |
|-------------------|------------|
| G-1 | V-UT-1, V-UT-4, V-UT-5, V-UT-6, V-UT-10, V-UT-12 |
| G-2 | V-IT-7, V-IT-8, V-IT-11, V-F-1, V-F-2, V-F-8 |
| G-3 | V-UT-3, V-UT-9, V-UT-10, V-IT-6, V-IT-10, V-IT-14 |
| G-4 | V-UT-7, V-IT-5, V-IT-13, V-F-6, V-F-9 |
| G-5 | V-UT-2, V-IT-4, V-IT-7, V-IT-8, V-E-1 |
| G-6 | V-UT-13, V-IT-15, V-IT-17, V-E-8 |
| G-7 | V-UT-15, V-IT-16, V-IT-18 |
| C-1 | V-IT-1, V-IT-2 |
| C-2 | V-UT-1, V-UT-3 |
| C-3 | V-UT-4, V-E-3 |
| C-4 | V-UT-5, V-UT-12, V-F-3, V-F-4 |
| C-5 | V-UT-7, V-IT-5, V-F-6 |
| C-6 | V-IT-3 |
| C-7 | V-IT-10 |
| C-8 | V-IT-10 |
| C-9 | V-IT-6 |
| C-10 | V-F-2, V-F-7 |
| C-11 | V-UT-8, V-F-1, V-F-2, V-F-3 |
| C-12 | V-UT-6, V-IT-7, V-IT-8 |
| C-13 | V-IT-9, V-IT-10 |
| C-14 | V-IT-9 |
| C-15 | V-IT-5, V-IT-10 |
| C-16 | V-UT-11, V-E-6, V-E-7 |
| C-17 | V-IT-11, V-F-8 |
| C-18 | V-UT-10, V-UT-14, V-IT-14, V-F-10 |
| C-19 | V-UT-11, V-E-6, V-E-7 |
| C-20 | V-IT-12 |
| C-21 | V-IT-13, V-F-9 |
| C-22 | V-IT-11 |
| C-23 | V-IT-11 |
| C-24 | V-IT-14 |
| C-25 | V-IT-17 |
| C-26 | V-UT-15, V-IT-16 |
| C-27 | V-IT-10, V-IT-16 |
