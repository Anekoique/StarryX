#!/bin/sh
# bake-image.sh — build a fresh ext4 image containing the upstream Alpine
# rootfs's contents plus the staged test tree under /root/tests.
#
# Inputs (env):
#   ARCH         riscv64 | loongarch64
#   ROOTFS_IMG   upstream rootfs-$ARCH.img (read-only source)
#   STAGE_DIR    xtest/build/<arch>/stage/  (host-side; contains root/tests/)
#   OUT_IMG      path to write the test rootfs image
#
# We don't resize the upstream image in place — the contest e2fsprogs
# (1.46.5) rejects features the upstream image uses. Easier path: extract
# the upstream contents into a fresh image we control.

set -u

ARCH=${ARCH:?ARCH must be set}
ROOTFS_IMG=${ROOTFS_IMG:?ROOTFS_IMG must be set}
STAGE_DIR=${STAGE_DIR:?STAGE_DIR must be set}
OUT_IMG=${OUT_IMG:?OUT_IMG must be set}

[ -f "$ROOTFS_IMG"           ] || { echo "[bake-image] error: $ROOTFS_IMG missing" >&2; exit 1; }
[ -d "$STAGE_DIR/root/tests" ] || { echo "[bake-image] error: $STAGE_DIR/root/tests missing" >&2; exit 1; }

BUILD_DIR=$(dirname "$OUT_IMG")
MNT="$BUILD_DIR/mnt"
SRC_MNT="$BUILD_DIR/src-mnt"
mkdir -p "$BUILD_DIR" "$MNT" "$SRC_MNT"

LOOP_DEV=""
SRC_LOOP=""
cleanup() {
    rc=$?
    set +e
    mountpoint -q "$MNT"     2>/dev/null && umount "$MNT"
    mountpoint -q "$SRC_MNT" 2>/dev/null && umount "$SRC_MNT"
    [ -n "$LOOP_DEV" ] && losetup -d "$LOOP_DEV" 2>/dev/null
    [ -n "$SRC_LOOP" ] && losetup -d "$SRC_LOOP" 2>/dev/null
    rmdir "$MNT" "$SRC_MNT" 2>/dev/null
    [ $rc -ne 0 ] && rm -f "$OUT_IMG"
    return $rc
}
trap cleanup EXIT INT TERM

# Size = real bytes on disk for both the upstream rootfs and the staged
# tree, plus 64 MB headroom. Rounded up to the next 16 MB.
stage_kb=$(du -sk  "$STAGE_DIR/root/tests" | awk '{print $1}')
rootfs_kb=$(du -sk "$ROOTFS_IMG"           | awk '{print $1}')
target_mb=$(( (stage_kb + rootfs_kb) / 1024 + 64 ))
target_mb=$(( (target_mb / 16 + 1) * 16 ))
echo "[bake-image] creating $OUT_IMG (${target_mb} MB; stage=$((stage_kb/1024)) MB, rootfs=$((rootfs_kb/1024)) MB)"

truncate -s "${target_mb}M" "$OUT_IMG"
mkfs.ext4 -q -F -L tests-rootfs "$OUT_IMG"

LOOP_DEV=$(losetup --show -f  "$OUT_IMG")
SRC_LOOP=$(losetup --show -fr "$ROOTFS_IMG")
mount       "$LOOP_DEV" "$MNT"
mount -o ro "$SRC_LOOP" "$SRC_MNT"

# Tar-pipe upstream rootfs contents in, then overlay the staged tree.
( cd "$SRC_MNT" && tar -cf - . ) | ( cd "$MNT" && tar -xf - )
mkdir -p "$MNT/root/tests"
cp -a "$STAGE_DIR/root/tests/." "$MNT/root/tests/"

sync
echo "[bake-image] OK: $OUT_IMG"
