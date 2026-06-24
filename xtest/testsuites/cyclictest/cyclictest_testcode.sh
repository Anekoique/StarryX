
./busybox echo "#### OS COMP TEST GROUP START cyclictest ####"

# StarryX: each ./cyclictest call is wrapped in `busybox timeout` so a single
# wedged invocation cannot hang the whole run (cyclictest's SCHED_FIFO/-a CPU
# affinity can block on the single-CPU QEMU guest). hackbench's loop count is
# bounded (upstream `-l 100000000` is effectively unbounded for the guest and
# never completes; -2/SIGINT reaps it at the end either way).
run_cyclictest() {
    echo "====== cyclictest $1 begin ======"
    ./busybox timeout 15 ./cyclictest $2
    if [ $? == 0 ]; then
	    ans="success"
    else
	    ans="fail"
    fi
  echo "====== cyclictest $1 end: $ans ======"
}

run_cyclictest NO_STRESS_P1 "-i 1000 -t1  -p99 -D 1s -q"
run_cyclictest NO_STRESS_P8 "-i 1000 -t8  -p99 -D 1s -q"

echo "====== start hackbench ======"
./hackbench -l 1000 &
hackbench_pid=$!

sleep 1

run_cyclictest STRESS_P1 "-i 1000 -t1  -p99 -D 1s -q"
run_cyclictest STRESS_P8 "-i 1000 -t8  -p99 -D 1s -q"

# Kill children in the parent process's interrupt processing, 
# so SIGINT is used instead of SIGKILL
kill -2 $hackbench_pid
if [ $? == 0 ]; then
    ans="success"
else
    ans="fail, ignore STRESS result"
fi
sleep 1
echo "====== kill hackbench: $ans ======"


./busybox echo "#### OS COMP TEST GROUP END cyclictest ####"