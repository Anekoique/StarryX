#!/bin/sh
# suite-adapters.sh — per-suite result extractors.
#
# Each upstream <suite>_testcode.sh brackets its output with
#   #### OS COMP TEST GROUP START <name> #### … #### … END <name> ####
# markers that span the WHOLE suite, not individual cases. They are NOT a
# verdict source — per redesign-xtest NG-8 we strip them. Verdicts come from
# each suite's NATIVE per-case token, parsed by an adapter below.
#
# Contract:  adapt_<suite> <suite-name>     (driver stdout on stdin)
#   - reads the captured driver output on stdin
#   - emits ONLY `[PASS] <suite>/<case>` / `[FAIL] <suite>/<case> exit=<n>` lines
#   - does NOT print a summary — run-suite.sh counts the emitted lines and
#     prints the single `[summary]`. (Keeping summary out of the adapter avoids
#     the POSIX pipe-subshell counter-scoping trap.)
#
# The driver exit code is available as $DRC for invocation-granularity suites.

# Drop the GROUP markers; pass everything else through.
strip_markers() {
    grep -v '#### OS COMP TEST GROUP'
}

# adapt_invocation <suite> — one verdict from the driver exit code in $DRC.
# Benchmark suites (cyclictest, lmbench, unixbench, libcbench) print numbers
# with no per-case pass/fail token, so the unit is "the driver exited 0".
adapt_invocation() {
    cat >/dev/null   # the benchmark log was already streamed to the console
    if [ "${DRC:-0}" -eq 0 ]; then
        echo "[PASS] $1/run"
    else
        echo "[FAIL] $1/run exit=${DRC:-1}"
    fi
}

# basic / lua / netperf: the upstream driver may print a per-case `>>> <case>
# rc=<n>` line (our vendored drivers are patched to emit these per sub-test). If
# the driver emitted none, fall back to one invocation-level verdict from $DRC.
adapt_rc_lines() {
    suite=$1
    seen=$(mktemp 2>/dev/null || echo /tmp/.adapt_seen.$$)
    : >"$seen"
    strip_markers | while IFS= read -r line; do
        case "$line" in
            '>>> '*' rc='*)
                name=${line#>>> }; name=${name% rc=*}
                rc=${line##* rc=}
                echo x >>"$seen"
                if [ "$rc" -eq 0 ] 2>/dev/null; then
                    echo "[PASS] $suite/$name"
                else
                    echo "[FAIL] $suite/$name exit=$rc"
                fi
                ;;
        esac
    done
    if [ ! -s "$seen" ]; then
        if [ "${DRC:-0}" -eq 0 ]; then echo "[PASS] $suite/run"
        else echo "[FAIL] $suite/run exit=${DRC:-1}"; fi
    fi
    rm -f "$seen"
}
adapt_basic()   { adapt_rc_lines "$@"; }

# iperf / netperf: `====== <tool> <NAME> end: success|fail ======` per test.
adapt_beginend() {
    suite=$1; tool=$2
    strip_markers | while IFS= read -r line; do
        case "$line" in
            *"$tool "*'end: success'*)
                name=$(echo "$line" | sed -E "s/.*$tool ([A-Za-z0-9_]+) end.*/\1/")
                echo "[PASS] $suite/$name" ;;
            *"$tool "*'end: fail'*)
                name=$(echo "$line" | sed -E "s/.*$tool ([A-Za-z0-9_]+) end.*/\1/")
                echo "[FAIL] $suite/$name exit=1" ;;
        esac
    done
}
adapt_netperf() { adapt_beginend "$1" netperf; }

# `testcase <suite> <case> success|fail` — the shape lua's test.sh and our
# iozone driver both emit.
adapt_testcase() {
    suite=$1; tag=$2
    strip_markers | while IFS= read -r line; do
        case "$line" in
            "testcase $tag "*' success')
                s=${line#testcase $tag }; s=${s% success}
                echo "[PASS] $suite/$s" ;;
            "testcase $tag "*' fail')
                s=${line#testcase $tag }; s=${s% fail}
                echo "[FAIL] $suite/$s exit=1" ;;
        esac
    done
}
adapt_lua()    { adapt_testcase "$1" lua; }
adapt_iozone() { adapt_testcase "$1" iozone; }

# busybox: lines `testcase busybox <cmd> success|fail`.
adapt_busybox() {
    suite=$1
    strip_markers | while IFS= read -r line; do
        case "$line" in
            'testcase busybox '*' success')
                cmd=${line#testcase busybox }; cmd=${cmd% success}
                echo "[PASS] $suite/$cmd" ;;
            'testcase busybox '*' fail')
                cmd=${line#testcase busybox }; cmd=${cmd% fail}
                echo "[FAIL] $suite/$cmd exit=1" ;;
        esac
    done
}

# libctest: runtest.exe brackets each case with
#   ========== START <runner>.exe <test> ==========
#   Pass!                       (success)  | FAIL <test> ...   (failure)
#   ========== END   <runner>.exe <test> ==========
# We track the test name from the START line and pair it with the Pass!/FAIL
# token before the matching END.
adapt_libctest() {
    suite=$1
    cur=""
    res=""
    strip_markers | while IFS= read -r line; do
        case "$line" in
            '========== START '*' =========='*)
                cur=$(echo "$line" | sed -E 's/.*START [^ ]+ ([^ ]+) =+.*/\1/')
                res="" ;;
            'Pass!'*)        [ -z "$res" ] && res=pass ;;
            'FAIL '*|*' FAIL'*|*'failed'*|*'Segmentation fault'*) res=fail ;;
            '========== END '*' =========='*)
                [ -n "$cur" ] || continue
                if [ "$res" = fail ]; then echo "[FAIL] $suite/$cur exit=1"
                else echo "[PASS] $suite/$cur"; fi
                cur=""; res="" ;;
        esac
    done
}

# iperf: `====== iperf <name> begin/end: success|fail ======`.
adapt_iperf() { adapt_beginend "$1" iperf; }

# cyclictest: `====== cyclictest <NAME> end: success|fail ======` per stage.
adapt_cyclictest() { adapt_beginend "$1" cyclictest; }

# libcbench: driver emits `testcase libcbench <case> success|fail` after
# confirming every benchmark printed its timing (the binary's own exit code is
# an unreliable wait()-status quirk — see the driver).
adapt_libcbench()  { adapt_testcase "$1" libcbench; }

# Benchmark suites — invocation granularity (one verdict per run).
adapt_unixbench()  { adapt_invocation "$@"; }
adapt_lmbench()    { adapt_invocation "$@"; }
