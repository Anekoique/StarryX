# xtest — StarryX test environment

`xtest/` produces a test rootfs image that boots StarryX with first-party
C tests under `/root/tests/c` plus the vendored OS-COMP testsuites under
`/root/tests/testsuites/<suite>`. The kernel side is selected at build time via
the `init-test` cargo feature, so `make run` (the normal boot path) is unaffected.

## Layout

```
xtest/
├── Makefile              build pipeline (Docker-driven)
├── README.md
├── .gitignore            ignores build/
├── c/                    first-party C tests (one .c per test)
│   ├── common/           shared header (xtest_assert / xtest_eq)
│   ├── syscall/          getpid, clone_basic, ...
│   ├── signal/           kill_self, ...
│   ├── mm/               mmap_anon, ...
│   └── fs/               open_close, ...
├── testsuites/           vendored OS-COMP suites (one dir per suite)
│   ├── UPSTREAM.md       provenance: URL + commit + per-suite SPDX + patches
│   ├── basic/            rCore-style syscall ELFs   (BUILD.sh + driver)
│   ├── busybox/          run-only (uses rootfs /bin/busybox)
│   ├── libctest/         musl libc-test
│   ├── lua/              static lua + .lua scripts
│   ├── unixbench/        BYTE UnixBench pgms
│   ├── lmbench/          lmbench_all (+ bundled libtirpc)
│   ├── libcbench/        single static libc-bench
│   ├── iperf/            iperf3 (loopback)
│   ├── netperf/          netperf + netserver (loopback)
│   ├── cyclictest/       rt-tests cyclictest + hackbench (+ bundled numactl)
│   └── iozone/           iozone filesystem benchmark
├── scripts/
│   ├── build/
│   │   ├── build-c.sh       in-Docker: cross-compile xtest/c/**/*.c
│   │   ├── build-suites.sh  in-Docker: run each testsuites/<s>/BUILD.sh
│   │   ├── stage.sh         assemble the staged tree
│   │   └── bake-image.sh    in-Docker: build a fresh ext4 image and rsync
│   ├── lib/
│   │   ├── timeout.sh       process-group-aware bounded-exec helper
│   │   └── suite-adapters.sh per-suite native-result → PASS/FAIL extractors
│   ├── run-all.sh        in-guest: drive the whole run
│   ├── run-c.sh          in-guest: iterate /root/tests/c/
│   └── run-suite.sh      in-guest: drive one testsuites/<suite>/ dir
└── build/                gitignored: per-arch outputs and the test rootfs image
```

## Quick start

Requires Docker (the build runs inside the contest image) **and** a host
musl cross toolchain (the kernel build's `lwext4_rust` invokes
`riscv64-linux-musl-gcc` / `loongarch64-linux-musl-gcc` on the host —
same constraint that `make rv` / `make la` already have).

```sh
make tests       ARCH=riscv64           # build xtest/build/riscv64/tests-rootfs-riscv64.img
make run-tests   ARCH=riscv64           # build kernel with init-test feature, build image, boot
```

The kernel embeds `src/test.sh` instead of `src/init.sh` when built with
`ROOT_FEATURES=init-test`. `make run` does not set `ROOT_FEATURES` and is
unaffected by anything in this directory.

## Adding a first-party C test

Drop a single-file program under `xtest/c/<group>/<name>.c`. It will be
cross-compiled statically against musl by `build-c.sh` and run by
`run-c.sh`. Test names must be unique across `xtest/c/` (we flatten to
`c/<basename>` in the staged tree). A non-zero exit prints `[FAIL] <name>
exit=<n>`; a signal-kill prints `[FAIL] <name> signal=<NAME>`.

```c
// xtest/c/syscall/getpid.c
#include "common/assert.h"
#include <unistd.h>
int main(void) {
    pid_t p = getpid();
    xtest_assert(p > 0);
    return 0;
}
```

## Make targets

```
make -C xtest all          ARCH=riscv64|loongarch64
make -C xtest build-c      ARCH=...
make -C xtest build-suites ARCH=...
make -C xtest stage        ARCH=...
make -C xtest bake-image   ARCH=...
make -C xtest clean        ARCH=...
make -C xtest docker-shell                      # interactive shell in the contest image
```

## testsuites

Each `xtest/testsuites/<suite>/` is a minimally-vendored OS-COMP suite (sources
trimmed to what the build + run driver need; upstream license preserved — see
`testsuites/UPSTREAM.md` for provenance, SPDX, and per-suite patches). A suite
ships:

- a `BUILD.sh` — cross-compiles the suite in the contest Docker image. It is
  invoked with `ARCH`, `MUSL_CC`, `PREFIX`, `SUITE_SRC`, `OUT_DIR` and drops its
  runnable artifacts plus the vendored `<suite>_testcode.sh` into `OUT_DIR`.
  Binaries link plain `-static` (the contest musl toolchain makes that a
  loadable static-PIE; do **not** force `-static-pie`/`-fPIE` — it faults the
  loader near address 0). busybox is the one exception: a no-op build that runs
  against the rootfs `/bin/busybox`.
- the upstream `<suite>_testcode.sh` run driver (vendored verbatim where
  possible). `run-suite.sh` runs it under a process-group-aware timeout and
  feeds its output through the suite's adapter in `scripts/lib/suite-adapters.sh`,
  which maps the suite's native result token to plain `[PASS]/[FAIL]/[TIMEOUT]`
  (the `#### OS COMP TEST GROUP ####` markers are stripped, never scored).
  Benchmark suites with no per-case token (lmbench, unixbench, cyclictest,
  libcbench, iozone's per-stage form) report one verdict per driver invocation.

A failing/crashing/timing-out test never aborts the run; `run-suite.sh` and
`run-all.sh` always exit 0. iperf/netperf use loopback only — `run-suite.sh`
brings `lo` up and verifies `127.0.0.1` before running them.

To add a suite: drop `xtest/testsuites/<name>/` with its trimmed sources, a
`BUILD.sh`, the `<name>_testcode.sh` driver, and a license file; add an
`adapt_<name>` to `scripts/lib/suite-adapters.sh` if its result format is new;
record it in `testsuites/UPSTREAM.md`.

## Docker image

Pinned by digest:

```
docker.educg.net/cg/os-contest@sha256:742479b5cd11b24501e2eccf5d409b78b76ba7aabcb87f815bbd5908a288313b
```

Cross compilers used:

- `riscv64`: `/opt/riscv64-linux-musl-cross/bin/riscv64-linux-musl-gcc`
- `loongarch64`: `/opt/loongarch64-linux-musl-cross/bin/loongarch64-linux-musl-gcc`

Image is `linux/amd64` — on Apple Silicon hosts Docker emulates with
qemu-x86_64 (slow but correct). Native amd64 hosts (CI) run at full speed.
