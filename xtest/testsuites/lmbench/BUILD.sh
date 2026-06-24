#!/bin/sh
# BUILD.sh — cross-build the lmbench suite (GPL-2.0 + results clause).
#
# Produces `lmbench_all` (the dispatcher the driver runs cwd-relative) + a
# trivial `hello` (the driver cp's it to /tmp for lat_proc shell). Plain
# `-static` only (loadable static-PIE; never -static-pie/-fPIE).
#
# RPC / sysroot containment (C-15): upstream's root `build:` target copies glibc
# sys/ headers into the SHARED toolchain sysroot and links a system libtirpc. We
# do NEITHER. Instead we cross-build the bundled libtirpc-1.3.6 and point lmbench
# at its headers, plus a vendored compat-include/sys/queue.h (musl omits it but
# libtirpc's clnt_bcast.c needs it). The shared sysroot is never touched.
#
# Build mechanics: libtirpc unpacks on tmpfs then symlinks in (the bind-mount FS
# rejects tar restoring read-only files). lmbench builds via its own
# `../scripts/build all` (a direct `$O/lmbench_all` target has no rule on a clean
# tree; the top-level `lmbench` target pulls an unvendored BitKeeper bk.ver dep).

set -u
. "$SUITE_LIB"
suite_init lmbench

root="$SUITE_SRC/lmbench_src"
tirpc="$root/libtirpc-1.3.6"
compat="$root/compat-include"
[ -f "$compat/sys/queue.h" ] || die "vendored $compat/sys/queue.h missing"

# 1. Bundled libtirpc, static. Unpack on tmpfs + symlink (bind-mount rejects the
#    tarball's read-only files); build last so any leak can't reach siblings.
cd "$root"; rm -fr libtirpc-1.3.6
tbuild="${TMPDIR:-/tmp}/lmbench-libtirpc-$$"
cleanup() { rm -f "$tirpc" 2>/dev/null; rm -rf "$tbuild" 2>/dev/null; }
trap cleanup EXIT INT TERM
rm -rf "$tbuild"; mkdir -p "$tbuild"
tar xzf libtirpc-1.3.6.tar.gz -C "$tbuild"
ln -s "$tbuild/libtirpc-1.3.6" libtirpc-1.3.6
say "building bundled libtirpc ($ARCH)"
( cd "$tbuild/libtirpc-1.3.6" \
    && ./configure --host="$HOST" CC="$MUSL_CC" CFLAGS="-O2 -I$compat" \
                   --disable-shared --disable-gssapi \
    && make -j ) || die "libtirpc build failed"
need "$tirpc/src/.libs/libtirpc.a"

# 2. lmbench. -I points at the bundled tirpc headers + the compat shim; the
#    Makefile's dangling -I/usr/include/tirpc is a harmless no-op on musl.
say "building lmbench_all ($ARCH)"
cd "$root/src"
env CC="$MUSL_CC" OS="$HOST" MAKE=make MAKEFLAGS="" \
    CFLAGS="-O -D_GNU_SOURCE -I$compat -I$tirpc/tirpc" \
    ../scripts/build all || die "lmbench build failed"
need "$root/bin/$HOST/lmbench_all"

# 3. A standalone `hello` (lmbench's own src/hello.c is harness-bound).
printf 'int main(void){ return 0; }\n' > "$OUT_DIR/.hello.c"
"$MUSL_CC" -static -O2 -o "$OUT_DIR/hello" "$OUT_DIR/.hello.c" || die "hello build failed"
rm -f "$OUT_DIR/.hello.c"
chmod 0755 "$OUT_DIR/hello"

stage "$root/bin/$HOST/lmbench_all" lmbench_all
stage_driver
say "staged into $OUT_DIR"
