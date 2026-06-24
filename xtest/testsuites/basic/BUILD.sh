#!/bin/sh
# BUILD.sh — cross-build the `basic` rCore-lineage syscall suite against musl.
#
# Each src/*.c is a standalone program -> one ELF per syscall. oscomp_shim.h
# (force-included) reconciles the rCore-isms with musl. Plain `-static` (the
# contest musl `-static` is a loadable static-PIE; never -static-pie/-fPIE).
# Every source is attempted before exiting non-zero so all failures show at once.

set -u
. "$SUITE_LIB"
suite_init basic

shim="$SUITE_SRC/oscomp_shim.h"
failures=""; built=0
for src in "$SUITE_SRC"/src/*.c; do
    [ -e "$src" ] || continue
    base=$(basename "$src" .c)
    say "cc $base"
    if "$MUSL_CC" -static -O2 -w -include "$shim" -o "$OUT_DIR/$base" "$src"; then
        built=$((built + 1))
    else
        echo "[basic] FAILED: $base" >&2
        failures="$failures $base"
    fi
done

# Per-case harness (run-all.sh), group driver, and the data file the
# open/read/fstat tests read.
stage_files "$SUITE_SRC/src/run-all.sh" "$SUITE_SRC/src/text.txt"
stage_driver

# The openat/mount/umount tests target `./mnt`; provide it (a .keep marker keeps
# the dir through `cp -a` staging + the ext4 bake).
mkdir -p "$OUT_DIR/mnt"; : > "$OUT_DIR/mnt/.keep"

say "built $built program(s) for $ARCH"
[ -z "$failures" ] || die "FAILURES:$failures"
