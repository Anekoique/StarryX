#!/bin/sh
# run-c.sh — execute every ELF under /root/tests/c and report PASS/FAIL.
# A failing test never aborts the run; the script always exits 0.

set -u

C_DIR="${1:-/root/tests/c}"

TESTS=$([ -d "$C_DIR" ] && find "$C_DIR" -maxdepth 1 -type f -perm -u+x | sort)
if [ -z "$TESTS" ]; then
    echo "no first-party tests at $C_DIR"
    exit 0
fi

pass=0
fail=0
for t in $TESTS; do
    name=$(basename "$t")
    "$t"
    rc=$?
    if [ $rc -eq 0 ]; then
        echo "[PASS] $name"
        pass=$((pass + 1))
    else
        if [ $rc -gt 128 ]; then
            sig=$((rc - 128))
            case $sig in
                6)  signame=ABRT ;;  8) signame=FPE ;;
                9)  signame=KILL ;; 11) signame=SEGV ;;
                13) signame=PIPE ;; 14) signame=ALRM ;;
                15) signame=TERM ;;  *) signame=$sig ;;
            esac
            echo "[FAIL] $name signal=$signame"
        else
            echo "[FAIL] $name exit=$rc"
        fi
        fail=$((fail + 1))
    fi
done

echo "[summary] c: $pass passed, $fail failed"
exit 0
