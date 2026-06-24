#!/bin/sh
# stage.sh — assemble xtest/build/<arch>/stage/root/tests/{c,testsuites,scripts}.
#
# Layout under $STAGE_DIR/root/tests:
#   c/             ELFs from build-c.sh
#   testsuites/<s> built suite outputs from build-suites.sh (incl. iozone), each
#                  with a `busybox` staged in so the vendored ./busybox resolves
#   scripts/       run-all.sh, run-c.sh, run-suite.sh, lib/

set -eu

ARCH=${ARCH:?ARCH must be set}
ROOT_DIR=${ROOT_DIR:-/code}

BUILD_DIR="$ROOT_DIR/xtest/build/$ARCH"
DEST="$BUILD_DIR/stage/root/tests"

mkdir -p "$DEST/c" "$DEST/scripts/lib"

# Arch marker for the in-guest runner: the StarryX kernel's uname(2) does not
# populate utsname.machine with the real arch, so `uname -m` is unreliable in the
# guest. We stamp the build arch here and run-all.sh reads it for arch-specific
# behaviour (e.g. skipping iperf on riscv64).
printf '%s\n' "$ARCH" > "$DEST/.arch"

# Compiled C-test ELFs.
[ -d "$BUILD_DIR/c" ] && cp -a "$BUILD_DIR/c/." "$DEST/c/"

# Vendored OS-COMP suites (optional — only when build-suites.sh ran). Each
# suite's built outputs are copied verbatim; a `busybox` is dropped into each
# suite dir because every upstream <suite>_testcode.sh calls `./busybox`
# cwd-relative (the build is no-op for busybox itself; we reuse the rootfs one
# at run time, but the driver still needs `./busybox` present).
if [ -d "$BUILD_DIR/testsuites" ]; then
    mkdir -p "$DEST/testsuites"
    cp -a "$BUILD_DIR/testsuites/." "$DEST/testsuites/"
    # A placeholder busybox marker per suite; the real /bin/busybox is symlinked
    # in-guest by run-all.sh (host stage cannot embed the guest binary). We drop
    # a tiny shim that execs the system busybox so `./busybox <applet>` works.
    for sdir in "$DEST"/testsuites/*/; do
        [ -d "$sdir" ] || continue
        cat >"$sdir/busybox" <<'SHIM'
#!/bin/sh
exec /bin/busybox "$@"
SHIM
        chmod 0755 "$sdir/busybox"
    done
fi

# In-guest runtime scripts + lib helpers.
cp -a "$ROOT_DIR"/xtest/scripts/run-*.sh "$DEST/scripts/"
cp -a "$ROOT_DIR"/xtest/scripts/lib/. "$DEST/scripts/lib/"
chmod 0755 "$DEST/scripts/"*.sh "$DEST/scripts/lib/"*.sh

echo "[stage] staged tree under $DEST"
