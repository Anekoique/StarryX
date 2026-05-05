#!/bin/sh
# stage.sh — assemble xtest/build/<arch>/stage/root/tests/{c,scripts}.
#
# Layout under $STAGE_DIR/root/tests:
#   c/         ELFs from build-c.sh
#   scripts/   run-all.sh, run-c.sh

set -eu

ARCH=${ARCH:?ARCH must be set}
ROOT_DIR=${ROOT_DIR:-/code}

BUILD_DIR="$ROOT_DIR/xtest/build/$ARCH"
DEST="$BUILD_DIR/stage/root/tests"

mkdir -p "$DEST/c" "$DEST/scripts"

# Compiled C-test ELFs.
[ -d "$BUILD_DIR/c" ] && cp -a "$BUILD_DIR/c/." "$DEST/c/"

# In-guest runtime scripts.
cp -a "$ROOT_DIR"/xtest/scripts/run-*.sh "$DEST/scripts/"
chmod 0755 "$DEST/scripts/"*.sh

echo "[stage] staged tree under $DEST"
