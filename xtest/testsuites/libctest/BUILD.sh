#!/bin/sh
# BUILD.sh — cross-build the musl libc-test suite.
#
# StarryX-specific deltas vs. upstream:
#   * LDFLAGS gains `-Wl,-z,notext`: the static link mixes PIC objects against
#     the contest musl libc.a, so the linker needs text-reloc permission.
#   * OBJCOPY=${PREFIX}objcopy: the Makefile derives OBJCOPY from $(PREFIX),
#     which we pass empty (so CC= wins) — without this it picks the host objcopy,
#     which cannot read the target .obj the `--redefine-sym` rule feeds it.
#   * KEEP the upstream `-rdynamic` on entry-dynamic.exe: it is a genuinely
#     dynamic binary (uses the rootfs loader), and the `dlopen` test introspects
#     its own dynamic symbol table — which only exists with --export-dynamic.
#
# Final binaries stay plain `-static` (upstream already passes it) — never
# -static-pie/-fPIE.

set -u
. "$SUITE_LIB"
suite_init libctest

enter libc-test
make clean >/dev/null 2>&1 || true

LDFLAGS="-Os -s -lpthread -lm -lrt -Wl,-z,notext"
mk() { make "$@" PREFIX= CC="$MUSL_CC" OBJCOPY="${PREFIX}objcopy" LDFLAGS="$LDFLAGS" -j1; }

# `make disk` builds entry-*.exe / runtest.exe / run-*.sh; `make so` builds the
# dlopen DSOs (disk does not). Both are retried: the emulated cross-gcc
# intermittently SIGSEGVs on a random source; make resumes from built objects.
say "building entry-static/entry-dynamic/runtest + DSOs ($ARCH)"
retry 5 -- mk disk
retry 5 -- mk so

# Stage with subdir layout preserved (the dynamic tests dlopen DSOs by a path
# relative to entry-dynamic.exe). cp_to <src> <dst-under-OUT_DIR>.
cp_to() { mkdir -p "$OUT_DIR/$(dirname "$2")"; cp -a "$1" "$OUT_DIR/$2" || die "stage $1"; }
need disk/entry-static.exe disk/entry-dynamic.exe disk/runtest.exe \
     disk/run-static.sh disk/run-dynamic.sh
cp_to disk/entry-static.exe  entry-static.exe
cp_to disk/entry-dynamic.exe entry-dynamic.exe
cp_to disk/runtest.exe       runtest.exe
cp_to disk/run-static.sh     run-static.sh
cp_to disk/run-dynamic.sh    run-dynamic.sh

# DSOs: keep the src/{functional,regression}/ layout AND a flat copy at the suite
# root — the dlopen/tls tests resolve `./<name>.so` next to entry-dynamic.exe.
for so in src/functional/*.so src/regression/*.so; do
    [ -f "$so" ] || continue
    cp_to "$so" "$so"
    cp_to "$so" "$(basename "$so")"
done

stage_driver
chmod 0755 "$OUT_DIR"/*.sh "$OUT_DIR"/*.exe 2>/dev/null || true
say "staged into $OUT_DIR"
