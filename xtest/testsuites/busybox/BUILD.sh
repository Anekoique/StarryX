#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-only
#
# NO-OP build for the busybox suite. busybox is run-only: the rootfs ships a
# static /bin/busybox and stage.sh drops a `./busybox` shim into the suite dir,
# so we only stage the run driver and the applet command list.

set -u
. "$SUITE_LIB"
suite_init busybox

stage_driver
stage "$SUITE_SRC/busybox_cmd.txt" busybox_cmd.txt
say "run-only (uses rootfs /bin/busybox via the staged ./busybox shim)"
