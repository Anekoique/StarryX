#!/bin/sh
# BUILD.sh — cross-build the netperf 2.7.0 suite (HP license).
#
# Ships only as a tarball: unpack into a scratch tree under $OUT_DIR, then
# cross-configure the autotools build. --build pins the in-container host gcc's
# triple so configure's host==build guard treats this as a cross-build. Plain
# `-static` (loadable static-PIE; never -static-pie/-fPIE). The driver runs
# ./netserver + ./netperf cwd-relative, so both binaries stage to $OUT_DIR root.

set -u
. "$SUITE_LIB"
suite_init netperf

scratch="$OUT_DIR/netperf-2.7.0"
rm -fr "$scratch"
tar xzf "$SUITE_SRC/netperf-2.7.0.tar.gz" -C "$OUT_DIR"
cd "$scratch" || die "unpack failed"

say "configuring + building netperf ($ARCH)"
./configure --build="$(gcc -dumpmachine)" --host="$HOST" CC="$MUSL_CC" \
    CFLAGS='-static -O2 -Wno-error' --disable-omni-tests --enable-cpuutil=none \
    || die "configure failed"
make -j || die "make failed"
need src/netperf src/netserver

stage src/netperf netperf
stage src/netserver netserver
stage_driver
rm -fr "$scratch"               # keep staged outputs lean
say "staged into $OUT_DIR"
