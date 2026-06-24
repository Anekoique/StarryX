#!/bin/sh
# run-iozone.sh — run the standard OS-COMP iozone sequence.
#
# Eight invocations: automatic mode plus seven multi-process (-t 4) throughput
# modes (write/read, random-read, read-backwards, stride-read, fwrite/fread,
# pwrite/pread, pwritev/preadv). Mirrors the contest's iozone runner.
#
# We cd into a scratch dir under /tmp first: the -t modes drop their work files
# in the current directory, and writing inside the iozone binary's own
# directory can make the kernel's ext4 lookup miss the binary afterwards
# (resolve returns ENOENT). Running from /tmp sidesteps that.
#
# Reports one [PASS]/[FAIL] per stage plus a summary; never aborts the run.

set -u

IOZONE_DIR="${1:-/root/tests/iozone}"
BIN="$IOZONE_DIR/iozone"
SCRATCH="${TMPDIR:-/tmp}/iozone-scratch"

if [ ! -x "$BIN" ]; then
    echo "no iozone binary at $BIN"
    exit 0
fi

mkdir -p "$SCRATCH"
cd "$SCRATCH" || { echo "[FAIL] iozone: cannot cd $SCRATCH"; exit 0; }

pass=0
fail=0

# run <label> <iozone args...> — run one stage, record pass/fail.
run() {
    label=$1
    shift
    echo "---- iozone: $label ----"
    "$BIN" "$@"
    rc=$?
    if [ $rc -eq 0 ]; then
        echo "[PASS] iozone $label"
        pass=$((pass + 1))
    else
        echo "[FAIL] iozone $label exit=$rc"
        fail=$((fail + 1))
    fi
    rm -f "$SCRATCH"/iozone.tmp* "$SCRATCH"/iozone.DUMMY* 2>/dev/null
}

run automatic        -a -r 1k -s 4m
run write-read       -t 4 -i 0 -i 1  -r 1k -s 1m
run random-read      -t 4 -i 0 -i 2  -r 1k -s 1m
run read-backwards   -t 4 -i 0 -i 3  -r 1k -s 1m
run stride-read      -t 4 -i 0 -i 5  -r 1k -s 1m
run fwrite-fread     -t 4 -i 6 -i 7  -r 1k -s 1m
run pwrite-pread     -t 4 -i 9 -i 10 -r 1k -s 1m
run pwritev-preadv   -t 4 -i 11 -i 12 -r 1k -s 1m

cd / && rm -rf "$SCRATCH" 2>/dev/null

echo "[summary] iozone: $pass passed, $fail failed"
exit 0
