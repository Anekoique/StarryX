host="127.0.0.1"
port="5001"
iperf="./iperf3"


./busybox echo "#### OS COMP TEST GROUP START iperf ####"


run_iperf() {
    name=$1
    args=$2
    echo "====== iperf $name begin ======"

    $iperf -4 -c $host -p $port -t 2 -i 0 $args
    if [ $? == 0 ]; then
	    ans="success"
    else
	    ans="fail"
    fi

    echo "====== iperf $name end: $ans ======"
    echo ""
}


#start server
# StarryX: background the server with `&` rather than iperf3's own `-D`
# daemonize. `-D` does a double-fork/detach the kernel doesn't keep listening
# (client gets "Connection refused"); a shell-backgrounded server + a short
# settle works — same model netperf's `netserver -D &` uses successfully here.
# `-4`: force IPv4 (iperf3 tries IPv6 first; "system does not seem to support
# IPv6"). `-B 127.0.0.1`: bind the loopback address explicitly. iperf3's default
# `-s` binds 0.0.0.0/:: which the kernel's loopback-only axnet does not accept
# (the server exits before "listening"); netperf works because it binds the
# loopback explicitly via `-L 127.0.0.1`. Mirror that here.
$iperf -4 -B $host -s -p $port &
iperf_server_pid=$!
./busybox sleep 1

#basic test
run_iperf "BASIC_UDP" "-u -b 1000G"
run_iperf "BASIC_TCP" ""

#parallel test
run_iperf "PARALLEL_UDP" "-u -P 5 -b 1000G"
run_iperf "PARALLEL_TCP" "-P 5"

#reverse test (server sends, client recieves)
run_iperf "REVERSE_UDP" "-u -R -b 1000G"
run_iperf "REVERSE_TCP" "-R"

./busybox kill -9 "$iperf_server_pid" 2>/dev/null

./busybox echo "#### OS COMP TEST GROUP END iperf ####"