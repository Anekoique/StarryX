#!/bin/sh
# run-all.sh — drive the whole test run inside the booted kernel.
#
# Layout (under /root/tests):
#   c/         first-party ELFs
#   iozone/    iozone benchmark (optional)
#   scripts/   this dir (run-c.sh, run-iozone.sh)
#
# Always exits 0 (failures never abort).

set -u

TESTS_ROOT=${TESTS_ROOT:-/root/tests}
SCRIPTS="$TESTS_ROOT/scripts"

echo "==== c ===="
sh "$SCRIPTS/run-c.sh" "$TESTS_ROOT/c"
echo "==== c done ===="

if [ -x "$TESTS_ROOT/iozone/iozone" ]; then
    echo "==== iozone ===="
    sh "$SCRIPTS/run-iozone.sh" "$TESTS_ROOT/iozone"
    echo "==== iozone done ===="
fi

echo "[done] xtest run complete"
exit 0
