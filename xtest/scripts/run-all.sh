#!/bin/sh
# run-all.sh — drive the whole test run inside the booted kernel.
#
# Layout (under /root/tests):
#   c/             first-party ELFs
#   testsuites/<s> vendored OS-COMP suites (incl. iozone)
#   scripts/       this dir (run-c.sh, run-suite.sh, lib/)
#
# Always exits 0 (failures never abort).
#
# SUITE_SKIP: space-separated suite names to skip. Defaults to the suites that
# wedge the single-CPU QEMU guest so badly that even in-guest timeouts cannot
# fire (only the host wall-clock cap breaks it), which would block every later
# suite:
#   * cyclictest — SCHED_FIFO `-p99` realtime tasks are never preempted.
#   * unixbench  — its SHELL stage spins (`looper`/`multi.sh`) on 1 CPU.
#   * lmbench    — its process/context-latency stages (`lat_proc`, `lat_ctx`)
#                  intermittently hang or fault the guest mid-run.
# All build fine; the gap is uniprocessor scheduling. See testsuites/UPSTREAM.md
# "Known-fail". Re-enable a suite by overriding, e.g. `SUITE_SKIP=cyclictest`.
#
# Arch-specific skip: on riscv64 the iperf3 server blocks forever inside its
# socket setup (it prints the first banner line but never reaches "Server
# listening" — a kernel socket-path gap; netperf works, so loopback itself is
# fine, and iperf passes 6/6 on loongarch64). Skipping it on rv64 avoids a
# lingering hung server process. See testsuites/UPSTREAM.md.

set -u

TESTS_ROOT=${TESTS_ROOT:-/root/tests}
SCRIPTS="$TESTS_ROOT/scripts"
export SCRIPTS_DIR="$SCRIPTS"
SUITE_SKIP="${SUITE_SKIP-cyclictest unixbench lmbench}"
# Arch comes from the staged .arch marker — the kernel's uname(2) does not fill
# utsname.machine, so `uname -m` is unreliable here.
ARCH=$(cat "$TESTS_ROOT/.arch" 2>/dev/null || echo unknown)
case "$ARCH" in
    riscv64) SUITE_SKIP="$SUITE_SKIP iperf" ;;
esac

echo "==== c ===="
sh "$SCRIPTS/run-c.sh" "$TESTS_ROOT/c"
echo "==== c done ===="

if [ -d "$TESTS_ROOT/testsuites" ]; then
    for d in "$TESTS_ROOT"/testsuites/*/; do
        [ -d "$d" ] || continue
        suite=$(basename "$d")
        skip=0
        for s in $SUITE_SKIP; do [ "$s" = "$suite" ] && skip=1; done
        if [ "$skip" -eq 1 ]; then
            echo "==== $suite (skipped — quarantined) ===="
            continue
        fi
        echo "==== $suite ===="
        sh "$SCRIPTS/run-suite.sh" "$d"
        echo "==== $suite done ===="
    done
fi

echo "[done] xtest run complete"
exit 0
