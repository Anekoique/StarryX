#!/bin/sh
# build-suites.sh — cross-build every vendored OS-COMP suite in the contest image.
#
# Runs inside the contest Docker image (ARCH, MUSL_CC from xtest/Makefile). For
# each xtest/testsuites/<s>/ that ships a BUILD.sh, it exports a uniform env and
# runs the recipe. Outputs land under xtest/build/<arch>/testsuites/<s>/.
#
# Build-time policy (redesign-xtest C-8a): every suite is attempted so a
# contributor sees all failures at once, but ANY failure makes this script exit
# non-zero so the image is not baked from a partial build.

set -u

ARCH=${ARCH:?ARCH must be set}
MUSL_CC=${MUSL_CC:?MUSL_CC must be set}
ROOT_DIR=${ROOT_DIR:-/code}

SUITES_DIR="$ROOT_DIR/xtest/testsuites"
BUILD_ROOT="$ROOT_DIR/xtest/build/$ARCH/testsuites"
SUITE_LIB="$ROOT_DIR/xtest/scripts/build/lib/suite.sh"   # shared BUILD.sh helpers

# Cross-tool prefix, e.g. /opt/.../riscv64-linux-musl-gcc -> riscv64-linux-musl-
PREFIX=$(basename "$MUSL_CC" | sed 's/gcc$//')

if [ ! -d "$SUITES_DIR" ]; then
    echo "[build-suites] no testsuites/ dir — nothing to build"
    exit 0
fi

# lmbench builds last: its libtirpc/sysroot step is the only one that could leak
# into the shared toolchain, so we bound any contamination (C-15). All other
# suites build in directory order first.
suites=$(find "$SUITES_DIR" -mindepth 1 -maxdepth 1 -type d \
    -exec test -f '{}/BUILD.sh' ';' -print 2>/dev/null \
    | sort | sed '/\/lmbench$/d')
[ -d "$SUITES_DIR/lmbench" ] && [ -f "$SUITES_DIR/lmbench/BUILD.sh" ] \
    && suites="$suites $SUITES_DIR/lmbench"

[ -n "$suites" ] || { echo "[build-suites] no suites with a BUILD.sh"; exit 0; }

# Suites whose build failure is a WARNING, not fatal: they are quarantined at
# run time (run-all.sh SUITE_SKIP) so a missing binary never runs anyway, and
# their large cross-builds (lmbench's libtirpc, numactl) intermittently trip the
# x86-emulated cross-gcc on non-amd64 hosts. A hard build error here would block
# the whole image bake for a suite that would be skipped regardless. Override
# with SUITE_BUILD_OPTIONAL=.
SUITE_BUILD_OPTIONAL="${SUITE_BUILD_OPTIONAL-cyclictest unixbench lmbench}"

failures=""
soft_failures=""
count=0
for sdir in $suites; do
    suite=$(basename "$sdir")
    out="$BUILD_ROOT/$suite"
    mkdir -p "$out"
    count=$((count + 1))
    echo "[build-suites] === $suite ($ARCH) ==="
    if ARCH="$ARCH" MUSL_CC="$MUSL_CC" PREFIX="$PREFIX" \
        SUITE_SRC="$sdir" OUT_DIR="$out" ROOT_DIR="$ROOT_DIR" \
        SUITE_LIB="$SUITE_LIB" \
        sh "$sdir/BUILD.sh"; then
        echo "[build-suites] $suite OK"
    else
        opt=0
        for o in $SUITE_BUILD_OPTIONAL; do [ "$o" = "$suite" ] && opt=1; done
        if [ "$opt" -eq 1 ]; then
            echo "[build-suites] $suite FAILED (optional — quarantined, not blocking)" >&2
            soft_failures="$soft_failures $suite"
        else
            echo "[build-suites] $suite FAILED" >&2
            failures="$failures $suite"
        fi
    fi
done

echo "[build-suites] attempted $count suite(s) for $ARCH"
[ -z "$soft_failures" ] || echo "[build-suites] optional FAILURES (non-blocking):$soft_failures" >&2
[ -z "$failures" ] || { echo "[build-suites] FAILURES:$failures" >&2; exit 1; }
