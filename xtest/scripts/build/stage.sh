#!/bin/sh
# stage.sh — assemble xtest/build/<arch>/stage/root/tests/{c,iozone,scripts}.
#
# Layout under $STAGE_DIR/root/tests:
#   c/         ELFs from build-c.sh
#   iozone/    iozone ELF from build-iozone.sh (if built)
#   scripts/   run-all.sh, run-c.sh, run-iozone.sh

set -eu

ARCH=${ARCH:?ARCH must be set}
ROOT_DIR=${ROOT_DIR:-/code}

BUILD_DIR="$ROOT_DIR/xtest/build/$ARCH"
DEST="$BUILD_DIR/stage/root/tests"

mkdir -p "$DEST/c" "$DEST/scripts"

# Compiled C-test ELFs.
[ -d "$BUILD_DIR/c" ] && cp -a "$BUILD_DIR/c/." "$DEST/c/"

# iozone benchmark (optional — only staged when build-iozone.sh ran).
if [ -x "$BUILD_DIR/iozone/iozone" ]; then
    mkdir -p "$DEST/iozone"
    cp -a "$BUILD_DIR/iozone/iozone" "$DEST/iozone/iozone"
fi

# In-guest runtime scripts.
cp -a "$ROOT_DIR"/xtest/scripts/run-*.sh "$DEST/scripts/"
chmod 0755 "$DEST/scripts/"*.sh

echo "[stage] staged tree under $DEST"
