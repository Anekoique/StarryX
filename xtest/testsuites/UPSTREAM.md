# Vendored OS-COMP testsuites

Sources are vendored (minimally — trimmed to what each `BUILD.sh` + driver
needs) from the OS-COMP kernel testsuites repo. LTP is intentionally excluded
this iteration.

- **Upstream:** https://github.com/oscomp/testsuits-for-oskernel
- **Branch:** `pre-2025`
- **Commit:** `3bcbbc06d2ebaee3d0d46e4d82a5617656685673`
- **Imported:** 2026-06-24

Each suite preserves its upstream license file(s); suites whose upstream tree
ships no standalone license file carry the authoritative text vendored in (basic,
libcbench, netperf — extracted from the tarball). Build outputs live under the
gitignored `xtest/build/<arch>/testsuites/`.

## Suites

| Suite | Upstream path | SPDX | Local patches / notes |
|-------|---------------|------|-----------------------|
| basic | `basic/` | GPL-3.0-or-later OR MIT | rCore-style; `oscomp_shim.h` reconciles rCore-isms with musl; `-include` shim, plain `-static`. |
| busybox | `busybox/` | GPL-2.0-only | No build — `./busybox` shim execs the rootfs `/bin/busybox`; only `busybox_cmd.txt` + driver vendored. |
| libctest | `libc-test/` | MIT | `LDFLAGS += -Wl,-z,notext`; keep upstream `-rdynamic` on `entry-dynamic` (needed for the `dlopen` self-introspection test); pass cross `OBJCOPY=${PREFIX}objcopy` (Makefile derives host objcopy from empty `PREFIX=`); flat-stage the `*_dso.so`; retry-loop the compile (transient emulated-gcc segfaults). |
| lua | `lua/` | MIT | Static build; `*.lua` scripts + `test.sh` vendored. |
| unixbench | `UnixBench/` | GPL-2.0-or-later | `mkdir pgms` before build; assets `multi.sh`/`sort.src`/`tst.sh`. |
| lmbench | `lmbench_src/` | GPL-2.0 (+ results clause) | Bundled `libtirpc-1.3.6` built static; extracted on tmpfs + symlinked (bind-mount FS rejects tar perms); vendored `compat-include/sys/queue.h` (musl omits it, libtirpc needs it); builds last. |
| libcbench | `libc-bench/` | MIT | Trivial static build; MIT text vendored in. |
| iperf | `iperf/` | BSD-3-Clause | `--enable-static-bin --disable-shared`; binary `src/iperf3`. Keep `examples/` + `iperf3.spec.in` (referenced by `configure.ac` `AC_CONFIG_FILES`). |
| netperf | `netperf/` | netperf/HP license | Unpack `netperf-2.7.0.tar.gz`; `--build=$(gcc -dumpmachine)`; license vendored from tarball. |
| cyclictest | `rt-tests-2.7/` | GPL-2.0 (numactl LGPL-2.1) | Bundled `numactl-2.0.14` static, extracted on tmpfs + symlinked (bind-mount FS rejects tar perms); `-DLOONGARCH_MUSL` on rv64 musl; `-latomic -static -Wl,-z,notext`. |
| iozone | iozone.org `iozone3_506` | permissive custom (Norcott/Capps) | Filesystem benchmark; `linux-AMD64`-style defines, plain `-static`. Moved here from `xtest/iozone/` so all OS-COMP suites live under `testsuites/`. |

## Run results (both arches, single-CPU QEMU)

| Suite | riscv64 | loongarch64 |
|-------|---------|-------------|
| c (first-party) | 9/9 | 9/9 |
| basic | 32/32 | 32/32 |
| busybox | 53/53 | 53/53 |
| iozone | 8/8 | 8/8 |
| libcbench | 1/1 | 1/1 |
| libctest | 217/217 | 217/217 |
| lua | 9/9 | 9/9 |
| netperf | 4/5 | 4/5 |
| iperf | skipped (rv64 server hang) | 6/6 |
| cyclictest / unixbench / lmbench | quarantined | quarantined |

Arch-specific notes:
- **iperf**: passes 6/6 on loongarch64. On riscv64 the iperf3 server **blocks
  forever inside its socket setup** — it prints the first banner line but never
  reaches "Server listening" (confirmed: the server process stays alive but does
  not listen after 15 s), so the client gets "Connection refused". Not a build,
  loopback, or IPv4/IPv6 issue (tested `&` vs `-D`, `-4`, `-B 127.0.0.1`; netperf
  proves loopback TCP/UDP works on rv64). It is a riscv64 kernel socket-path gap
  (`bind`/`listen`/`setsockopt` blocking). `run-all.sh` skips iperf on riscv64
  only (via the staged `.arch` marker — the kernel does not fill
  `utsname.machine`, so `uname -m` is unreliable) so a hung server cannot
  linger; it runs normally on la64.
- **libctest**: 217/217 on both arches (clean run). One test,
  `ftello_unflushed_append`, is mildly **flaky** under the slow x86-emulated
  QEMU (timing-dependent buffered-I/O) — it passes in a clean run but can FAIL
  intermittently. The `dlopen` test (which does
  `dlopen(0)` + `dlsym(self, "dlopen_main")`, introspecting the main executable's
  dynamic symbol table) needs `--export-dynamic`. We restore the upstream
  `-rdynamic` on the `dynamic:` recipe (`entry-dynamic.exe` is a genuinely
  dynamic binary — using the rootfs loader — so `-rdynamic` links cleanly and
  exports its symbols; it was wrongly stripped earlier on the assumption it
  broke a static-PIE link). la64's dynamic half also required a bake-time loader
  symlink — see below — without which `entry-dynamic.exe` exec'd with "No such
  file or directory".
- **la64 dynamic loader (bake-image.sh)**: the contest la64 musl gcc emits the
  ELF interpreter `/lib64/ld-musl-loongarch-lp64d.so.1`, but the Alpine la64
  rootfs ships `/lib/ld-musl-loongarch64.so.1`. `bake-image.sh` symlinks the
  requested path to the shipped loader (la64 only; rv64's name already matches),
  which makes every dynamically-linked test binary exec.
- **Cross-arch build hygiene**: suites that compile in a shared source tree
  (`lua`, `unixbench`) MUST `make clean` first in their `BUILD.sh` — otherwise a
  prior arch's objects are reused and the staged binary is built for the wrong
  ISA (faults `InstructionNotExist` and, before the kernel hardens user SIGILL,
  panics the kernel). The other in-tree suites use arch-aware `configure`/recipes.
- **Build under x86 emulation**: on non-amd64 hosts the emulated la64/rv64
  cross-gcc intermittently dies with `internal compiler error: Segmentation
  fault`. `libctest/BUILD.sh` retries up to 5× (make resumes from built objects);
  the quarantined suites' build failures are non-fatal (`SUITE_BUILD_OPTIONAL`).

## Known-fail (quarantined this iteration)

- **cyclictest** — skipped by default (`run-all.sh` `SUITE_SKIP=cyclictest`). Its
  `cyclictest -p99` SCHED_FIFO realtime tasks wedge the single-CPU QEMU guest:
  the realtime task is never preempted, so even the in-guest `busybox timeout`
  wrapper (and `run-suite.sh`'s outer timeout) cannot fire — only the host
  wall-clock cap breaks it, which would block every later suite. The suite
  builds fine (`cyclictest` + `hackbench` produced); the gap is uniprocessor
  realtime scheduling under the kernel. Re-enable once SCHED_FIFO preemption /
  SMP is in place: boot with `SUITE_SKIP= `. The driver is already hardened
  (per-call `busybox timeout 15`, `hackbench -l 1000`, dropped `-a` affinity).
- **unixbench** — skipped by default. Its SHELL stage (`looper 20 ./multi.sh N`)
  spins on the single-CPU guest and the in-guest timeout cannot fire, same class
  as cyclictest. Builds fine (all `pgms/*` produced). Re-enable once the SHELL
  stage is bounded / SMP is in place.
- **lmbench** — skipped by default. `lat_syscall` stages run (syscall/read/write
  latencies print) but a later stage (`lat_proc`/`lat_ctx`) intermittently either
  `Terminated (core dumped)` or hangs the guest, so it cannot be relied on to not
  block later suites. Builds fine (`lmbench_all` + `hello`). Re-enable once
  process/context-switch latency paths are stable / SMP is in place.

## Partial (runs, not fully green — left in the run for visibility)

- **libcbench** — runs every benchmark (malloc/string/regex/pthread all print
  their timing) but the binary's own exit code is the last forked child's raw
  `wait()` status (an upstream quirk), which is non-zero on StarryX. The driver
  therefore judges by completion (counts the `time:` lines), so it reports a
  single `[PASS]`.
- **netperf** — 4/5 pass over loopback (`127.0.0.1`): UDP_STREAM, TCP_STREAM,
  UDP_RR, TCP_RR succeed. TCP_CRR (connect-rate / rapid connect-close churn)
  fails — a more demanding TCP connection-recycling path. This is the proof that
  guest loopback TCP/UDP works (on both arches).

## Shebang-exec note

The kernel exec does not honour the `#!` shebang for a cwd-relative
`./script.sh` (busybox `execvp` returns "not found"). The basic/lua/libctest
drivers therefore invoke their nested scripts via `busybox sh <script>` rather
than `./<script>`. A kernel fix (honour `#!` on relative-path exec) would let the
contest drivers run verbatim — tracked as follow-up.
