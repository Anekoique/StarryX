#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# BUILD.sh — cross-build the cyclictest suite (rt-tests 2.7).
#
# Produces `cyclictest` + `hackbench` (driver runs both cwd-relative and reaps
# hackbench with `kill -2` — that reaping must stay intact). rt-tests bundles
# numactl, which cyclictest links statically.
#
# Flag routing into the rt-tests Makefile (our triple matches none of its arch
# branches, so we inject explicitly; plain `-static` only — loadable static-PIE,
# never -static-pie/-fPIE):
#   LDFLAGS=-Wl,-z,notext   text-reloc permission (static link mixes PIC libnuma)
#   LDFLAGS1=-latomic -static   appended to the cyclictest link (RTTESTNUMA)
#   CFLAGS1=-DLOONGARCH_MUSL  skips a sigev_notify_thread_id redefine on musl
#                             (the macro name is misleading — needed on rv64 too)

set -u
. "$SUITE_LIB"
suite_init cyclictest

enter rt-tests-2.7
make clean >/dev/null 2>&1 || true
rm -fr numactl-2.0.14

# Build the bundled numactl on tmpfs, then symlink it where the Makefile expects
# (./numactl-2.0.14/.libs/libnuma.a). We do NOT use the Makefile's own
# extract_numactl: under Docker on macOS, /code is a bind mount that rejects tar
# restoring the tarball's read-only build-aux/* files ("Permission denied").
nbuild="${TMPDIR:-/tmp}/cyclictest-numactl-$$"
cleanup() { rm -f "$SUITE_SRC/rt-tests-2.7/numactl-2.0.14" 2>/dev/null; rm -rf "$nbuild" 2>/dev/null; }
trap cleanup EXIT INT TERM
rm -rf "$nbuild"; mkdir -p "$nbuild"
tar zxf numactl-2.0.14.tar.gz -C "$nbuild"
( cd "$nbuild/numactl-2.0.14" && ./configure --host="$HOST" && CC="${PREFIX}gcc" make -j ) \
    || die "numactl build failed"
ln -s "$nbuild/numactl-2.0.14" numactl-2.0.14

say "building cyclictest + hackbench ($ARCH)"
make CROSS_COMPILE="$PREFIX" CFLAGS1="-DLOONGARCH_MUSL" \
     LDFLAGS="-Wl,-z,notext" LDFLAGS1="-latomic -static" \
     cyclictest hackbench || die "make failed"
need cyclictest hackbench

stage cyclictest cyclictest
stage hackbench hackbench
stage_driver
say "staged into $OUT_DIR"
