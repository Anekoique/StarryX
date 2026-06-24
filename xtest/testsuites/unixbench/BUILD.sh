#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# BUILD.sh — cross-build the UnixBench (BYTE v5.1.3) suite.
#
# Builds the pgms/ benchmark binaries (driver runs them cwd-relative). Plain
# `-static` via UB_GCC_OPTIONS (the contest musl `-static` is a loadable
# static-PIE; never -static-pie/-fPIE). `make clean` + fresh pgms/ first: the
# source tree is shared across arches, so stale objects would yield wrong-arch
# binaries (faults InstructionNotExist on the target).

set -u
. "$SUITE_LIB"
suite_init unixbench

enter UnixBench
make clean >/dev/null 2>&1 || true
rm -rf pgms; mkdir -p pgms          # `programs` links straight into ./pgms

say "building pgms ($ARCH)"
UB_GCC_OPTIONS="-static -O2 -w" make CC="$MUSL_CC" ARCH="$ARCH" all || die "make failed"

# Stage every built program; skip the non-binary helpers that ship in pgms/.
copied=0
for bin in pgms/*; do
    [ -f "$bin" ] || continue
    case "$(basename "$bin")" in *.sh|*.logo|index.base|gfx-x11) continue ;; esac
    stage "$bin" "$(basename "$bin")"
    copied=$((copied + 1))
done
[ "$copied" -gt 0 ] || die "no programs built"

stage_files "$SUITE_SRC/multi.sh" "$SUITE_SRC/sort.src" "$SUITE_SRC/tst.sh"
stage_driver
say "staged $copied program(s) into $OUT_DIR"
