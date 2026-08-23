
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

The host resolves a profile and optional CLI case filter before building. A
case sidecar or testsuit manifest may set `boot_count` from 1 through 8. A case
with `boot_count > 1` must be the only selected case and cannot run in
interactive mode. The host then creates a unique run under
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
├── serial.log                    # one-boot run
├── serial.boot-{1,2,...}.log     # multi-boot run
├── report.json
└── report.tap
```

For a multi-boot case, the host launches QEMU `boot_count` times against the
same disposable `rootfs.img`. It does not rebuild, reinject, copy, or finalize
the image between boots. Every boot starts a fresh protocol state machine and
must pass before the next boot begins. The first non-passing boot stops the
sequence. Host deadlines and process-group termination/reaping apply
independently to every QEMU launch. The run is terminally passed only when all
declared boots pass; only then are the combined JSON/TAP reports written and
the `.partial` directory atomically finalized. The JSON report contains one
`boots[]` entry per attempted boot, including status, QEMU exit, detail, and
case results; TAP includes one diagnostic line per boot.

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
    boot_count: u32,
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
    boot_count: u32,
    architectures: Vec<Arch>,
}

struct ResolvedCase {
    id: String,
    program: PathBuf,
    args: Vec<String>,
    timeout_secs: u64,
    boot_count: u32,
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
    boot_count: u32,
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
    boot_count: u32,
    outcomes: Vec<CaseResult>,
    status: RunStatus,
    boots: Vec<BootSummary>,
}

struct BootSummary {
    boot: u32,
    outcomes: Vec<CaseResult>,
    status: RunStatus,
    detail: Option<String>,
    qemu_exit: Option<i32>,
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
- C-28: @test-binding: multi_boot_case_requires_an_isolated_run
  `boot_count` is in `1..=8`; a value greater than one requires that case to be
  the run's sole selection and forbids interactive execution.
- C-29: @judgment
  A multi-boot run reuses exactly one disposable image, starts a fresh protocol
  state and process group per boot, stops on the first non-pass, and finalizes
  one immutable run only after writing boot-indexed serial logs and aggregate
  JSON/TAP reports.
- C-30: @test-binding: page_cache_persist
  The two-boot page-cache persistence case writes and syncs on boot one and
  validates the same image on boot two; both boot summaries must pass.

[**CHANGELOG**]

- 2026-08-17: Added bounded isolated multi-boot cases, same-image execution,
  per-boot serial artifacts, and boot-aware JSON/TAP reporting for persistent
  storage validation.

---
