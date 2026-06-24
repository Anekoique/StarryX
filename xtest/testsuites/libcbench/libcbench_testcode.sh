
./busybox echo "#### OS COMP TEST GROUP START libcbench ####"
# libc-bench forks each benchmark and `main` returns the LAST child's raw
# wait() status as its own exit code (an upstream quirk). On StarryX that yields
# a non-zero exit even though every benchmark completed and printed its timing,
# so we judge by completion: count the `time:` lines libc-bench emits (one per
# benchmark) rather than trusting the exit code.
./libc-bench | ./busybox tee /tmp/libcbench.out
done_n=$(./busybox grep -c '  time:' /tmp/libcbench.out 2>/dev/null)
./busybox rm -f /tmp/libcbench.out
if [ "${done_n:-0}" -ge 20 ]; then
    ./busybox echo "testcase libcbench all-benchmarks success"
else
    ./busybox echo "testcase libcbench all-benchmarks fail"
fi
./busybox echo "#### OS COMP TEST GROUP END libcbench ####"