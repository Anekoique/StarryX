#!/bin/sh
# BUILD.sh — cross-build the vendored musl libc-bench suite.
#
# Trivial flat Makefile: compiles {main,malloc,pthread,regex,stdio,string,utf8}.c
# at -Os and links one static `libc-bench` (its LDFLAGS already pass -static — we
# never add -static-pie/-fPIE; the contest musl `-static` is a loadable
# static-PIE). The Makefile drives compile+link via $(CC), so `make CC=` is all
# the cross-targeting we need. Driver runs `./libc-bench` cwd-relative.

set -u
. "$SUITE_LIB"
suite_init libcbench

enter libc-bench
make clean >/dev/null 2>&1 || true
say "building libc-bench ($ARCH)"
make CC="$MUSL_CC" -j || die "make failed"
need libc-bench

stage libc-bench libc-bench
stage_driver
say "staged into $OUT_DIR"
