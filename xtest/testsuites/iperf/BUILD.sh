#!/bin/sh
# BUILD.sh — cross-build the iperf3 suite (BSD-3-Clause).
#
# Autotools tree: cross-configure with HOST, build the static binary
# (--enable-static-bin → loadable static-PIE under the contest musl toolchain;
# never -static-pie/-fPIE). Keep examples/ and iperf3.spec.in vendored — both
# are referenced by configure.ac's AC_CONFIG_FILES.

set -u
. "$SUITE_LIB"
suite_init iperf

enter iperf
make clean 2>/dev/null || true
say "configuring + building iperf3 ($ARCH)"
./configure --host="$HOST" CC="$MUSL_CC" --enable-static-bin --disable-shared || die "configure failed"
make -j || die "make failed"

# Static binary lands in src/iperf3 (older libtool layouts use src/.libs/iperf3).
if   [ -f src/iperf3 ];       then stage src/iperf3 iperf3
elif [ -f src/.libs/iperf3 ]; then stage src/.libs/iperf3 iperf3
else die "no iperf3 binary produced"
fi

stage_driver
say "staged into $OUT_DIR"
