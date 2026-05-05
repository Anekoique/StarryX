#!/bin/sh
# run-all.sh — drive the whole test run inside the booted kernel.
#
# Layout (under /root/tests):
#   c/         first-party ELFs
#   scripts/   this dir (run-c.sh)
#
# Always exits 0 (failures never abort).

set -u

TESTS_ROOT=${TESTS_ROOT:-/root/tests}
SCRIPTS="$TESTS_ROOT/scripts"

echo "==== c ===="
sh "$SCRIPTS/run-c.sh" "$TESTS_ROOT/c"
echo "==== c done ===="

echo "[done] xtest run complete"
exit 0
