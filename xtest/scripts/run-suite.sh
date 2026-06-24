#!/bin/sh
# run-suite.sh — drive one vendored OS-COMP suite in-guest.
#
#   run-suite.sh <suite-dir>      e.g. /root/tests/testsuites/lua
#
# Runs the suite's vendored <name>_testcode.sh, feeds its output through the
# suite's adapter (lib/suite-adapters.sh) to extract plain [PASS]/[FAIL] lines
# (GROUP markers stripped, per NG-8), counts them for a [summary], and always
# exits 0 so a failing suite never aborts the overall run (redesign-xtest C-8b).
#
# Each suite dir already contains a `busybox` (staged in) so the upstream
# driver's cwd-relative `./busybox` calls resolve.

set -u

SUITE_DIR=${1:?usage: run-suite.sh <suite-dir>}
SCRIPTS_DIR=${SCRIPTS_DIR:-$(CDPATH= cd "$(dirname "$0")" && pwd)}
LIB="$SCRIPTS_DIR/lib"

. "$LIB/suite-adapters.sh"

suite=$(basename "$SUITE_DIR")

if [ ! -d "$SUITE_DIR" ]; then
    echo "no suite dir at $SUITE_DIR"
    exit 0
fi

driver=$(find "$SUITE_DIR" -maxdepth 1 -name '*_testcode.sh' 2>/dev/null | head -n1)
if [ -z "$driver" ]; then
    echo "[skip] $suite: no <name>_testcode.sh driver"
    exit 0
fi

cd "$SUITE_DIR" || { echo "[FAIL] $suite: cannot cd $SUITE_DIR"; exit 0; }

# Net suites use loopback (127.0.0.1). Best-effort bring `lo` up via whatever
# the rootfs provides (busybox ip/ifconfig), then let the suite run — its own
# client/server reveal whether loopback works. We do NOT gate on `ping`: busybox
# ping needs raw sockets that may be unavailable, which would wrongly skip a
# working loopback. (axnet provides a loopback iface; per C-16.)
case "$suite" in
    iperf | netperf)
        busybox ip link set lo up 2>/dev/null \
            || busybox ifconfig lo up 2>/dev/null \
            || ip link set lo up 2>/dev/null \
            || ifconfig lo up 2>/dev/null || true
        ;;
esac

# Resolve the adapter for this suite; fall back to invocation granularity.
adapter="adapt_$suite"
command -v "$adapter" >/dev/null 2>&1 || adapter=adapt_invocation

# Run the driver under a per-suite wall-clock bound, capture its output and rc,
# stream the raw log to the console (so benchmark numbers remain visible), and
# extract verdicts via the adapter. The driver's own per-invocation timeouts
# (it bounds long benchmarks itself) plus our outer cap keep a hang bounded.
OUT=$(DRC=0; sh "$LIB/timeout.sh" "${SUITE_TIMEOUT:-120}" sh "$driver" 2>&1; echo "__RC__=$?")
drc=$(printf '%s\n' "$OUT" | sed -n 's/^__RC__=//p' | tail -n1)
log=$(printf '%s\n' "$OUT" | grep -v '^__RC__=')

# Echo the raw driver log (markers stripped) so the console keeps the detail.
printf '%s\n' "$log" | grep -v '#### OS COMP TEST GROUP'

# Extract verdicts; count [PASS]/[FAIL] for the summary.
verdicts=$(printf '%s\n' "$log" | DRC="$drc" "$adapter" "$suite")
printf '%s\n' "$verdicts"

if [ "$drc" = "124" ]; then
    echo "[TIMEOUT] $suite (driver exceeded ${SUITE_TIMEOUT:-120}s)"
fi

pass=$(printf '%s\n' "$verdicts" | grep -c '^\[PASS\]' || true)
fail=$(printf '%s\n' "$verdicts" | grep -c '^\[FAIL\]' || true)
echo "[summary] $suite: ${pass:-0} passed, ${fail:-0} failed"

exit 0
