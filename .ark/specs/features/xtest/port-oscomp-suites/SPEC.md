
[**Goals**]

- G-1: `xtest/testsuites/<suite>/` vendors ten OS-COMP suites, license-preserved.
- G-2: Each suite exposes a `BUILD.sh` that cross-builds it in the contest image.
- G-3: `build-suites.sh` builds every suite, attempting all before non-zero exit.
- G-4: `run-suite.sh` drives each suite in-guest, never aborting, with timeouts.
- G-5: `make tests`/`make run-tests` bake and run the suites on rv64 and la64.
- G-6: A per-suite adapter maps native results to `[PASS]/[FAIL]/[TIMEOUT]`.

[**Non-goals**]

- NG-1: LTP is not vendored or run this iteration (deferred to a follow-up).
- NG-2: No OS-COMP scoring markers in console output (inherits redesign-xtest NG-8).
- NG-3: No deep multi-subsystem kernel rewrites; deep bugs are quarantined, not fixed.

[**Architecture**]

```
xtest/
├── Makefile                       # +build-suites target; all: += build-suites
├── testsuites/                    # NEW — vendored upstream suites, one dir per suite
│   ├── UPSTREAM.md                # URL + commit + import date + per-suite SPDX + patches
│   ├── basic/                     # rCore-style syscall ELFs (build basic/user/)
│   │   ├── <upstream sources>     #   preserves upstream tree + README/license
│   │   ├── oscomp_shim.h          #   StarryX: reconcile rCore-isms with musl headers
│   │   ├── BUILD.sh               #   cross-build via shim; flatten ELFs to out dir
│   │   └── basic_testcode.sh      #   vendored upstream run driver
│   ├── busybox/                   # run-only: uses Alpine's /bin/busybox (no Kbuild)
│   │   ├── busybox_cmd.txt        #   applet list (vendored)
│   │   ├── BUILD.sh               #   no-op build (records "use system busybox")
│   │   └── busybox_testcode.sh
│   ├── libctest/                  # musl libc-test (entry-static/dynamic + runtest)
│   ├── lua/                       # static lua interpreter + *.lua scripts
│   ├── unixbench/                 # BYTE UnixBench pgms/*
│   ├── lmbench/                   # lmbench_all + libtirpc-1.3.6 (bundled, built static)
│   ├── libcbench/                 # single static libc-bench binary
│   ├── iperf/                     # iperf3 (autotools, static)
│   ├── netperf/                   # netperf-2.7.0.tar.gz → netperf + netserver
│   └── cyclictest/                # rt-tests-2.7: cyclictest + hackbench + numactl-2.0.14
├── scripts/
│   ├── build/
│   │   ├── build-suites.sh        # NEW — dispatch each testsuites/<s>/BUILD.sh in Docker
│   │   ├── lib/suite.sh           # NEW — shared BUILD.sh helpers (suite_init/stage/need/retry)
│   │   ├── stage.sh               # EDIT — stage suite outputs + busybox shim + .arch marker
│   │   ├── build-c.sh             # (unchanged)
│   │   └── bake-image.sh          # EDIT — la64 dynamic-loader symlink (interp name mismatch)
│   ├── lib/
│   │   ├── timeout.sh             # NEW — process-group-aware bounded-exec (POSIX sh)
│   │   └── suite-adapters.sh      # NEW — per-suite native-token → PASS/FAIL extractors
│   ├── run-all.sh                 # EDIT — iterate testsuites/* (+arch-skip via .arch marker)
│   ├── run-suite.sh               # NEW — drive one suite dir; run _testcode.sh + adapter
│   └── run-c.sh                   # (unchanged)
└── build/<arch>/testsuites/<s>/   # gitignored — per-suite built outputs

(iozone is now a suite under testsuites/iozone/ — its old bespoke build-iozone.sh
 + run-iozone.sh + the `build-iozone` Make target were removed.)

(stage.sh also drops a `busybox` into each staged testsuites/<s>/ dir, because
 every upstream `<suite>_testcode.sh` invokes `./busybox` cwd-relative; and a
 `.arch` marker file because the kernel's uname(2) does not fill utsname.machine.)

(root Makefile, src/test.sh, src/init.sh: unchanged — `make tests` already
 routes to `make -C xtest all`; run-tests already sets BLK=y NET=y.)
```

Each `testsuites/<suite>/BUILD.sh` is invoked with `ARCH`, `MUSL_CC`, the
cross-tool `PREFIX` (e.g. `riscv64-linux-musl-`), `SUITE_SRC` (its own dir),
and `OUT_DIR` (`xtest/build/<arch>/testsuites/<suite>/`). It builds and drops
runnable artifacts + the vendored `<suite>_testcode.sh` (and any `*.lua` /
`*.txt` / `*.sh` assets the driver needs) under `OUT_DIR`. Build is a no-op
only for `busybox` (records intent to use the system busybox).

[**Data Structure**]

No Rust types. The contracts are file/dir shapes and script env:

```
build-suites.sh env (per BUILD.sh):   ARCH, MUSL_CC, PREFIX, SUITE_SRC, OUT_DIR
staged tree:                          stage/root/tests/testsuites/<suite>/{artifacts,<suite>_testcode.sh,assets}
run-suite.sh args:                    run-suite.sh <suite-dir>
timeout.sh args:                      timeout.sh <seconds> <cmd> [args...]   → exit 124 on timeout
```

UPSTREAM.md per-suite row: `name | upstream path | SPDX | local patches`.

[**API Surface**]

`xtest/Makefile` — new public target + `all` extension:

```
make -C xtest build-suites ARCH=riscv64|loongarch64    # cross-build all suites in Docker
all: build-c build-iozone build-suites stage bake-image
```

`xtest/scripts/build/build-suites.sh` contract:

```
Inputs (env):  ARCH, MUSL_CC, ROOT_DIR (=/code in Docker)
Behaviour:     for each xtest/testsuites/<s> with a BUILD.sh, derive PREFIX from
               MUSL_CC, export {ARCH,MUSL_CC,PREFIX,SUITE_SRC,OUT_DIR}, run BUILD.sh.
               Records failures; attempts every suite; exits non-zero if any failed
               (C-8a — image not baked on build failure).
Outputs:       xtest/build/<arch>/testsuites/<s>/* per suite.
```

`xtest/scripts/run-suite.sh` contract:

```
Args:          run-suite.sh <suite-dir>            (e.g. /root/tests/testsuites/lua)
Behaviour:     cd into the suite dir (which now contains a `busybox`); run its
               <name>_testcode.sh and capture stdout. Dispatch the captured output
               through the suite's adapter in lib/suite-adapters.sh, which extracts
               per-case verdicts from that suite's NATIVE token (not the GROUP
               markers — those are only stripped). Each adapter emits
               [PASS]/[FAIL] <suite>/<case> [exit=<n>|signal=<X>] and a
               [summary] <suite>: P passed, F failed. Always exits 0 (C-8b).
Granularity:   suites with a per-case token (basic "Testing X:"+exit, busybox
               "testcase ... success|fail", libctest runtest.exe lines, lua per-
               script, iperf "begin/end: success|fail") yield one verdict per case;
               benchmark suites with no per-case token (cyclictest, lmbench,
               unixbench, libcbench) yield ONE verdict per driver invocation =
               "the invocation exited 0". The unit is documented per adapter.
```

`xtest/scripts/lib/suite-adapters.sh` contract:

```
Provides:      adapt_<suite>() functions, one per suite, each reading the driver's
               captured output on stdin + the driver exit code, emitting the
               [PASS]/[FAIL]/[TIMEOUT] lines + the [summary]. The GROUP-START/END
               markers are dropped (grep -v), never used as a verdict source.
```

`xtest/scripts/lib/timeout.sh` contract:

```
Args:          timeout.sh <seconds> <cmd> [args...]
Behaviour:     run cmd in its own process group; on timeout SIGTERM then SIGKILL the
               whole group (kill -- -PGID) so a driver's self-backgrounded helpers
               (hackbench, netserver) are reaped too; exit 124 on timeout.
               Pure POSIX sh (no coreutils `timeout`); ash-compatible.
Granularity:   run-suite.sh applies timeout.sh PER driver-invocation inside the
               adapter, NOT around a whole self-reaping driver — drivers that reap
               their own children by signal (cyclictest `kill -2`) keep that
               behaviour; only the bounded sub-invocations are wrapped.
```

`xtest/scripts/run-all.sh` contract (extended):

```
After the c/ block and the optional iozone/ block:
  for d in $TESTS_ROOT/testsuites/*/ ; do
    [ -d "$d" ] || continue
    suite=$(basename "$d")
    echo "==== $suite ===="
    sh "$SCRIPTS/run-suite.sh" "$d"
    echo "==== $suite done ===="
  done
Still exits 0 unconditionally.
```

[**Constraints**]

- C-1: @source-scan: `BUILD.sh @ xtest/testsuites/*/BUILD.sh`
  Every vendored suite dir (except busybox's no-op) ships an executable `BUILD.sh`.
- C-2: @source-scan: `mkfs.ext4 @ xtest/scripts/build/build-suites.sh`
  All suite cross-compilation runs inside the Docker image; `build-suites.sh` invokes no host compiler.
- C-3: @judgment
  `xtest/testsuites/` sources are vendored verbatim with their upstream license file(s); `build/` stays gitignored.
- C-4: @source-scan: `UPSTREAM.md @ xtest/testsuites/UPSTREAM.md`
  `UPSTREAM.md` records URL + commit + import date + per-suite SPDX + per-suite patch summary.
- C-5: @tool: `dash -n` over every `xtest/**/*.sh` and `src/test.sh`
  All run-time and build-time shell is POSIX `sh`, ash-compatible, lint-clean under `dash -n`.
> Constraint-ID note (lineage): this Spec's local `C-6`/`C-7` restate
> redesign-xtest's `C-8a`/`C-8b` (build-time fail-no-bake / run-time
> never-abort); this Spec's local `C-8` restates redesign-xtest's `NG-8`
> (no scoring markers). The local IDs are authoritative here; the parentheticals
> name the inherited rule.

- C-6: @judgment
  Build-time: a suite build error is recorded and forces non-zero exit so no image is baked; every suite is still attempted (= redesign-xtest C-8a).
- C-7: @judgment
  Run-time: a failing/crashing/timing-out test logs `[FAIL]`/`[TIMEOUT]` and the run continues; `run-suite.sh`/`run-all.sh` always exit 0 (= redesign-xtest C-8b).
- C-8: @judgment
  `run-suite.sh` emits only `[PASS]/[FAIL]/[TIMEOUT]` + a `[summary]` line; `#### OS COMP TEST GROUP ####` markers are stripped, never reach the console (= redesign-xtest NG-8).
- C-9: @judgment
  Each suite's verdicts come from a per-suite adapter mapping its native success token to `[PASS]/[FAIL]`; GROUP markers are not a verdict source.
- C-10: @judgment
  Benchmark suites without a per-case token (cyclictest, lmbench, unixbench, libcbench) yield one verdict per driver invocation (`invocation exited 0`).
- C-11: @judgment
  Every suite binary is static-PIE loadable by the StarryX loader: recipes link plain `-static` through the contest musl toolchain (as iozone does), never forcing `-static-pie`/`-fPIE`/`-no-pie`.
- C-12: @judgment
  `stage.sh` places a `busybox` in each `testsuites/<suite>/` dir so each vendored `_testcode.sh`'s cwd-relative `./busybox` resolves; no Kbuild busybox is built.
- C-13: @judgment
  lmbench links the bundled `libtirpc-1.3.6` built static; cyclictest links the bundled `numactl-2.0.14` built static.
- C-14: @judgment
  cyclictest is built with `-DLOONGARCH_MUSL` on rv64 musl too (upstream forces it) and `-latomic -static`; libctest adds `-Wl,-z,notext` and neutralizes `entry-dynamic`'s `-rdynamic`; netperf configures `--build=$(gcc -dumpmachine)`.
- C-15: @judgment
  lmbench's glibc-`sys/`-headers-into-sysroot step writes only to a throwaway sysroot copy or suite-local include path; it must not mutate the shared toolchain sysroot the sibling suites compile against.
- C-16: @judgment
  Net suites (iperf/netperf) use loopback `127.0.0.1`; `run-suite.sh` (or `src/test.sh`) brings `lo` up / verifies `127.0.0.1` reachability before running them.
- C-17: @judgment
  `make tests`/`make run-tests` reach `build-suites` through `xtest`'s `all` target; the root `Makefile`, `src/init.sh`, and `src/test.sh` are unchanged.
- C-18: @judgment
  Suites lacking an in-dir license file (basic, libcbench, netperf-in-tarball) get the authoritative license text vendored into the suite dir; `UPSTREAM.md` records the resolved SPDX (C-4).
- C-19: @judgment
  Deep kernel bugs a suite surfaces are recorded in `UPSTREAM.md`'s per-suite notes as known-fail and not fixed in this task; only shallow fixes land here.
- C-20: @source-scan: `SUITE_LIB @ xtest/scripts/build/lib/suite.sh`
  Each `BUILD.sh` sources the shared `scripts/build/lib/suite.sh` (helpers `suite_init`/`enter`/`stage`/`stage_driver`/`need`/`retry`) instead of repeating env/staging boilerplate.
- C-21: @judgment
  Arch-specific runtime behaviour keys off a staged `.arch` marker, not `uname -m` (the kernel does not populate `utsname.machine`); iperf is skipped on riscv64 (server-socket hang) and runs on loongarch64.
- C-22: @judgment
  `bake-image.sh` symlinks the la64 musl dynamic-loader path the contest binaries request (`/lib64/ld-musl-loongarch-lp64d.so.1`) to the loader the rootfs ships, so dynamically-linked test binaries exec.
- C-23: @judgment
  Build-time transient failures of the x86-emulated cross-gcc are absorbed by a retry loop (libctest) and by treating quarantined-suite build failures as non-fatal (`SUITE_BUILD_OPTIONAL`).

---
