#!/bin/sh
# build-c.sh — cross-compile xtest/c/**/*.c into static musl ELFs.
#
# Runs inside the contest Docker image (ARCH and MUSL_CC come from
# xtest/Makefile). Layout: xtest/c/<group>/<name>.c -> build/<arch>/c/<name>.
# Names must be unique across groups (we flatten); duplicates are rejected.

set -u

ARCH=${ARCH:?ARCH must be set}
MUSL_CC=${MUSL_CC:?MUSL_CC must be set}

ROOT_DIR=${ROOT_DIR:-/code}
SRC_DIR="$ROOT_DIR/xtest/c"
OUT_DIR="$ROOT_DIR/xtest/build/$ARCH/c"

mkdir -p "$OUT_DIR"

SOURCES=$(find "$SRC_DIR" -type f -name '*.c' -not -path "$SRC_DIR/common/*" | sort)
[ -n "$SOURCES" ] || { echo "[build-c] no .c files under $SRC_DIR"; exit 0; }

DUPES=$(echo "$SOURCES" | xargs -n1 basename | sort | uniq -d)
if [ -n "$DUPES" ]; then
    echo "[build-c] error: duplicate test basenames:" >&2
    echo "$DUPES" >&2
    exit 1
fi

CFLAGS="-static -O2 -Wall -Wextra -I$SRC_DIR"
failures=""
count=0
for src in $SOURCES; do
    name=$(basename "$src" .c)
    count=$((count + 1))
    printf '[build-c] %s -> %s\n' "$src" "$OUT_DIR/$name"
    "$MUSL_CC" $CFLAGS -o "$OUT_DIR/$name" "$src" || failures="$failures $name"
done

echo "[build-c] compiled $count test(s) for $ARCH"
[ -z "$failures" ] || { echo "[build-c] FAILURES:$failures" >&2; exit 1; }
