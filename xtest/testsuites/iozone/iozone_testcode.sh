#!/bin/sh
# iozone_testcode.sh — standard OS-COMP iozone sequence (in-guest run driver).
#
# Eight invocations: automatic mode plus seven multi-process (-t 4) throughput
# modes (write/read, random-read, read-backwards, stride-read, fwrite/fread,
# pwrite/pread, pwritev/preadv).
#
# The -t modes drop work files in the current directory. Writing inside the
# iozone binary's own dir can make the kernel's ext4 lookup miss the binary
# afterwards (resolve returns ENOENT), so we run from a scratch dir under /tmp.
#
# Emits `testcase iozone <label> success|fail` per stage (parsed by
# adapt_iozone); bracketed by the OS-COMP GROUP markers (stripped by run-suite).

set -u

# The binary sits next to this script (the suite dir is the cwd run-suite uses).
BIN="$(CDPATH= cd "$(dirname "$0")" && pwd)/iozone"
SCRATCH="${TMPDIR:-/tmp}/iozone-scratch"

echo "#### OS COMP TEST GROUP START iozone ####"

if [ ! -x "$BIN" ]; then
    echo "no iozone binary at $BIN"
    echo "#### OS COMP TEST GROUP END iozone ####"
    exit 0
fi

mkdir -p "$SCRATCH"
cd "$SCRATCH" || { echo "testcase iozone scratch fail"; echo "#### OS COMP TEST GROUP END iozone ####"; exit 0; }

run() {
    label=$1
    shift
    if "$BIN" "$@"; then
        echo "testcase iozone $label success"
    else
        echo "testcase iozone $label fail"
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

echo "#### OS COMP TEST GROUP END iozone ####"
exit 0
