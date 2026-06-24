#!/bin/sh
# BUILD.sh — cross-build the stock Lua suite (MIT).
#
# Build src/ directly (PLAT=generic, plain -static — the contest musl `-static`
# is a loadable static-PIE; never -static-pie/-fPIE). `make clean` first: the
# lua source tree is shared across arches, so stale objects would otherwise
# yield a wrong-arch binary (faults InstructionNotExist on the target).

set -u
. "$SUITE_LIB"
suite_init lua

enter lua
make -C src clean >/dev/null 2>&1 || true
say "building lua ($ARCH)"
make -C src CC="$MUSL_CC" AR="${PREFIX}ar rcu" RANLIB="${PREFIX}ranlib" \
    MYCFLAGS=-static MYLDFLAGS=-static PLAT=generic
need src/lua

stage src/lua lua
stage_files "$SUITE_SRC"/*.lua "$SUITE_SRC/test.sh"
stage_driver
say "staged into $OUT_DIR"
