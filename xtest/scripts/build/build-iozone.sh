#!/bin/sh
# build-iozone.sh — cross-compile vendored iozone into a static musl ELF.
#
# Runs inside the contest Docker image (or directly on a host that exposes the
# musl cross toolchain). ARCH and MUSL_CC come from xtest/Makefile.
# Layout: xtest/iozone/src/*.c -> build/<arch>/iozone/iozone.
#
# Recipe mirrors upstream's `linux-AMD64` target (threads + largefiles + async
# I/O + SysV shared memory), retargeted at the musl cross compiler and linked
# statically so it runs on the Alpine rootfs with no loader shims.

set -u

ARCH=${ARCH:?ARCH must be set}
MUSL_CC=${MUSL_CC:?MUSL_CC must be set}

ROOT_DIR=${ROOT_DIR:-/code}
SRC_DIR="$ROOT_DIR/xtest/iozone/src"
OUT_DIR="$ROOT_DIR/xtest/build/$ARCH/iozone"

mkdir -p "$OUT_DIR"

# Shared feature defines (see upstream makefile, iozone_linux-AMD64.o rule).
# Link plain `-static` exactly like the first-party C tests (build-c.sh): the
# contest musl toolchain turns that into a static-PIE the StarryX user-ELF
# loader runs correctly. Do NOT force `-static-pie`/`-fPIE` — that produces a
# differently-relocated binary whose musl self-relocation faults under the
# loader (segfault near address 0 before main). -w: iozone's pre-C99 source
# emits a wall of warnings we do not own.
DEFS="-Dunix -Dlinux -DHAVE_ANSIC_C -DASYNC_IO -D_LARGEFILE64_SOURCE"
CFLAGS="-O3 -w $DEFS"

echo "[build-iozone] compiling iozone for $ARCH"

"$MUSL_CC" -c $CFLAGS -DSHARED_MEM -DNAME='"linux"' -DHAVE_PREAD \
    "$SRC_DIR/iozone.c"   -o "$OUT_DIR/iozone.o"   || exit 1
"$MUSL_CC" -c $CFLAGS -DSHARED_MEM \
    "$SRC_DIR/libbif.c"   -o "$OUT_DIR/libbif.o"   || exit 1
"$MUSL_CC" -c $CFLAGS \
    "$SRC_DIR/libasync.c" -o "$OUT_DIR/libasync.o" || exit 1

"$MUSL_CC" -static -O3 \
    "$OUT_DIR/iozone.o" "$OUT_DIR/libasync.o" "$OUT_DIR/libbif.o" \
    -lpthread -o "$OUT_DIR/iozone" || exit 1

rm -f "$OUT_DIR"/*.o
echo "[build-iozone] built $OUT_DIR/iozone"
