#!/bin/sh
# timeout.sh — run a command with a wall-clock bound, POSIX-sh only.
#
#   timeout.sh <seconds> <cmd> [args...]
#
# The contest Alpine rootfs busybox `timeout` exists, but we cannot rely on its
# flags being uniform, and several suite drivers self-background helpers
# (hackbench, netserver) that must be reaped on timeout. So we run <cmd> in its
# own process group and, on timeout, signal the WHOLE group (TERM then KILL) so
# no orphaned stressor/server survives.
#
# Exit code: the command's own exit code, or 124 if it timed out.

set -u

if [ "$#" -lt 2 ]; then
    echo "usage: timeout.sh <seconds> <cmd> [args...]" >&2
    exit 2
fi

secs=$1
shift

# Run the command in its own process group when possible so we can kill its
# whole tree (reaping self-backgrounded helpers). `setsid` is in busybox on the
# Alpine rootfs; if absent (some hosts), fall back to a plain child and signal
# just the direct child. GRP is the kill target: "-PID" for a group, "PID" else.
if command -v setsid >/dev/null 2>&1; then
    setsid "$@" &
    cmd_pid=$!
    grp="-$cmd_pid"
else
    "$@" &
    cmd_pid=$!
    grp="$cmd_pid"
fi

# Watchdog: sleep, then kill the command (group) if still alive.
(
    i=0
    while [ "$i" -lt "$secs" ]; do
        kill -0 "$cmd_pid" 2>/dev/null || exit 0
        sleep 1
        i=$((i + 1))
    done
    # Still running after <secs>: TERM, give it a moment, then KILL.
    kill -TERM "$grp" 2>/dev/null
    sleep 1
    kill -KILL "$grp" 2>/dev/null
) &
watch_pid=$!

# Wait for the command; capture its status.
wait "$cmd_pid"
rc=$?

# Did the watchdog fire? If the command's group is gone and rc indicates a
# signal kill at/after our deadline, report timeout. We detect by checking
# whether the watchdog already exited normally (command finished) vs was still
# counting (we killed it). Simplest robust signal: if rc > 128 the command was
# signalled; treat a TERM/KILL from us as a timeout.
case "$rc" in
    143 | 137) rc=124 ;; # 128+15 (TERM) / 128+9 (KILL) → our timeout kill
esac

# Tear down the watchdog if the command finished on its own.
kill "$watch_pid" 2>/dev/null
wait "$watch_pid" 2>/dev/null

exit "$rc"
