# xtest framework runtime research

## Scope

This research covers only the generic StarryX system-test framework. It does
not design, wrap, patch, or interpret OS-COMP or any other concrete external
test suite.

## Current-state findings

- `xtest/scripts/build/bake-image.sh` rebuilds an ext4 image through a
  privileged Docker container, loop-device setup, mount, rsync, and cleanup.
- `make run-tests` selects `starry/src/test.sh` through the `init-test` Cargo
  feature, while the guest script always falls through to an interactive shell.
- QEMU uses the ordinary `-nographic` path. The host has no wall-clock timeout,
  completion event, automated termination, serial-log ownership, or test-failure
  exit-code propagation.
- Suite-specific build, skip, timeout, and output parsing have leaked into the
  framework. Those policies belong to test sources, not framework runtime.

## 1. Rootfs injection

### Evidence

`debugfs` operates directly on an ext2/3/4 filesystem held in a regular image
file and supports scripted writable commands. `mke2fs -d` can instead create a
new filesystem and populate it from a directory or tarball. Both approaches
avoid loop devices and host mounts.

Sources:

- https://man7.org/linux/man-pages/man8/debugfs.8.html
- https://man7.org/linux/man-pages/man8/mke2fs.8.html

### Adopt

Copy the pinned base rootfs image into `target/xtest/<arch>/<profile>/rootfs.img`
and inject one prepared `/xtest` bundle into the copy with scripted `debugfs -w`
commands. Run `e2fsck -fn` after injection. The framework treats the base image
as immutable and never mutates `rootfs-<arch>.img` in place.

The bundle boundary is a directory tree, not a suite-specific staging layout:

```text
/xtest/
  plan
  bin/
  guest-runner.sh
```

### Adapt

Keep `mke2fs -d <bundle-root>` as a backend for a future standalone test disk
or fully rebuilt rootfs. It is not the first implementation because the current
StarryX boot path already depends on an Alpine base image whose complete
contents must be preserved.

### Reject

- Privileged Docker, `losetup`, and mount/umount for ordinary test-image builds.
- Injecting files one suite at a time; assemble one host bundle first, then
  perform one image mutation phase.
- Mutating the shared base rootfs in place.

## 2. QEMU lifecycle

### Evidence

QEMU documents `-chardev stdio`/serial backends, `logfile`, `-no-reboot`, and
the QMP `quit` command. Rust `std::process::Command` supports piped child I/O;
`Child::kill` plus `wait` provides explicit termination and reaping.

Sources:

- https://qemu.readthedocs.io/en/v9.2.4/system/invocation.html
- https://qemu.readthedocs.io/en/v9.2.4/interop/qemu-qmp-ref.html
- https://doc.rust-lang.org/std/process/index.html
- https://doc.rust-lang.org/std/process/struct.Child.html

### Adopt

The Rust host runner owns the QEMU child process, pipes serial output, mirrors
it to the terminal, and writes `serial.log`. It applies a host wall-clock
deadline independent of guest scheduling.

The runner stops on one of four terminal states:

1. a valid `run_end` event: record result, request QEMU termination, wait;
2. QEMU exits first: fail as `unexpected_qemu_exit` unless success was complete;
3. deadline expires: terminate QEMU, wait, fail as `host_timeout`;
4. malformed/conflicting terminal events: terminate QEMU and fail as
   `protocol_error`.

Prefer graceful QMP `quit` when a QMP channel is configured; always retain
`Child::kill` followed by `wait` as the bounded fallback. Use `-no-reboot` so a
guest reboot cannot silently start a second run.

Batch mode is canonical and must return non-zero when any test fails. An
explicit interactive mode may inherit stdin and leave QEMU attached, but it is
not a verification path.

### Reject

- Parsing human boot logs as success criteria.
- Guest-only timeout enforcement.
- Always dropping into a shell after a test run.
- Assuming QEMU process exit alone describes test success.

## 3. TestPlan and TestEvent

### Adopt: generated TestPlan

The host resolves sources and profiles into an immutable, line-oriented plan
installed into the bundle. The guest never discovers source trees or parses
TOML. Each plan entry contains only execution data:

```text
case <escaped-id> <timeout-seconds> <escaped-program> [escaped-args...]
```

The host validates unique IDs, architecture support, executable paths, timeout
bounds, and argument encoding before boot. Use a deliberately small escaping
grammar rather than `eval`; alternatively emit one argv file per case if POSIX
shell decoding would otherwise become complex.

### Adopt: minimal wire events

Use a StarryX-owned serial protocol with a unique prefix and version:

```text
XTEST/1 run_start <count>
XTEST/1 case_start <id>
XTEST/1 case_end <id> pass 0
XTEST/1 case_end <id> fail <exit-code>
XTEST/1 case_end <id> timeout 124
XTEST/1 run_end <passed> <failed> <timed-out>
```

Only prefixed lines are machine events; all other serial data remains diagnostic
output. The host validates event order and exactly one terminal event per case.

### Adapt: TAP 13

TAP 13 is a useful report/export format, but not the serial wire format. TAP
requires a syntactically constrained stream; arbitrary kernel logs and test
stdout on the same serial channel would otherwise be invalid TAP. Convert the
validated internal events to `report.tap` and `report.json` on the host.

Source:

- https://testanything.org/tap-version-13-specification.html

### Reject

- Per-suite stdout adapters in the framework.
- Treating arbitrary `[PASS]` text as a protocol.
- Making the guest runner understand source metadata, profiles, or build rules.

## 4. Host implementation language

### Adopt

Use one standalone Rust host crate and one POSIX guest script.

Rust is justified for child-process ownership, concurrent serial streaming and
deadline handling, structured validation, deterministic reports, and reliable
cleanup. Keep it as one crate with modules, not a plugin or multi-crate system.

The POSIX guest runner remains intentionally small: read the generated plan,
execute each installed binary, enforce best-effort per-case timeout, emit events,
and either finish batch mode or enter an explicitly requested interactive mode.

### Reject

- Rebuilding the current orchestration as another large shell framework.
- Provider/plugin traits, multiple VM backends, or suite-specific Rust modules.
- Moving builds into the guest.

## Recommended framework boundary

```text
source cases / opt-in testsuits
        |
        v
Rust host: resolve -> build -> plan -> bundle -> image -> QEMU -> report
                                      |
                                      v
POSIX guest: execute plan -> XTEST/1 events
```

The standalone xtest framework knows only cases, plans, bundles, QEMU, and
events. Directories under `testsuits/` are package-local integrations accepted
through the same generic contract; no external suite name appears in framework
code. StarryX consumes the complete xtest repository through one gitlink.
