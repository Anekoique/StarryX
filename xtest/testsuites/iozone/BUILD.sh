#!/bin/sh
# BUILD.sh — cross-compile vendored iozone into a static musl ELF.
#
# Mirrors upstream's `linux-AMD64` target (threads + largefiles + async I/O +
# SysV shared memory). Plain `-static` (the contest musl `-static` is a loadable
# static-PIE; `-static-pie`/`-fPIE` faults the loader near address 0 before
# main). `-w`: iozone's pre-C99 source warns heavily.

set -u
. "$SUITE_LIB"
suite_init iozone

DEFS="-Dunix -Dlinux -DHAVE_ANSIC_C -DASYNC_IO -D_LARGEFILE64_SOURCE"
CFLAGS="-O3 -w $DEFS"
src="$SUITE_SRC/src"

say "compiling for $ARCH"
"$MUSL_CC" -c $CFLAGS -DSHARED_MEM -DNAME='"linux"' -DHAVE_PREAD \
    "$src/iozone.c"   -o "$OUT_DIR/iozone.o"   || die "compile iozone.c failed"
"$MUSL_CC" -c $CFLAGS -DSHARED_MEM \
    "$src/libbif.c"   -o "$OUT_DIR/libbif.o"   || die "compile libbif.c failed"
"$MUSL_CC" -c $CFLAGS \
    "$src/libasync.c" -o "$OUT_DIR/libasync.o" || die "compile libasync.c failed"
"$MUSL_CC" -static -O3 \
    "$OUT_DIR/iozone.o" "$OUT_DIR/libasync.o" "$OUT_DIR/libbif.o" \
    -lpthread -o "$OUT_DIR/iozone" || die "link failed"
rm -f "$OUT_DIR"/*.o
chmod 0755 "$OUT_DIR/iozone"

stage_driver
say "built $OUT_DIR/iozone"
