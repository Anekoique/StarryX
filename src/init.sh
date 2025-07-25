# ./busybox mkdir -v /bin
# ./busybox ln -v -s /musl/busybox /bin/busybox
# cd /bin
# export PATH=/bin
# busybox ln -v -s busybox ln
# busybox ln -v -s busybox cp
# busybox ln -v -s busybox stat
# busybox ln -v -s busybox mkdir
# mkdir -v /lib
# cp -v /glibc/lib/ld-linux-riscv64-lp64d.so.1 /lib
# 
# stat /lib/ld-linux-riscv64-lp64d.so.1
# stat /glibc/lib/ld-linux-riscv64-lp64d.so.1


./busybox mkdir -v /bin
./busybox ln -v -s /musl/busybox /bin/busybox
cd /bin
export PATH=/bin
busybox ln -v -s busybox ln
ln -v -s busybox cp
ln -v -s busybox mv
ln -v -s busybox rm
ln -v -s busybox cat
ln -v -s busybox touch
ln -v -s busybox sh
ln -v -s busybox ls
ln -v -s busybox env
ln -v -s busybox mkdir
ln -v -s busybox sleep
ln -v -s busybox basename
ln -v -s busybox stat

mkdir -v /lib
mkdir -v /usr
cp -v /glibc/lib/* /lib
if [[ $ARCH == loongarch64 ]]; then
    ln -v -s /musl/lib/libc.so /lib/ld-musl-loongarch-lp64d.so.1
else
    ln -v -s /musl/lib/libc.so /lib/ld-musl-$ARCH.so.1
    ln -v -s /musl/lib/libc.so /lib/ld-musl-$ARCH-sf.so.1
fi

ln -v -s /lib /lib64
ln -v -s /lib /usr/lib
ln -v -s /lib /usr/lib64

export LD_LIBRARY_PATH=".:./lib:/musl/lib:/lib"

mkdir -v -p /var/tmp

mkdir -v /etc/
echo "root:x:0:0:root:/root:/bin/bash" >/etc/passwd
echo "nobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin" >>/etc/passwd
cat > /etc/protocols <<EOF
ip      0       IP      # internet protocol, pseudo protocol number
icmp    1       ICMP    # internet control message protocol
tcp     6       TCP     # transmission control protocol
udp     17      UDP     # user datagram protocol
EOF

run_ltp() {
    echo "#### OS COMP TEST GROUP START ltp-$1 ####"

    export LTP_TIMEOUT_MUL=0.5
    export LTP_DEV_FS_TYPE=tmpfs
    export LTP_SINGLE_FS_TYPE=tmpfs

    all_testcases="
    "
    passed_testcase="
    abort01
    abs01
    accept01
    accept03
    accept4_01
    access01
    access02
    access04
    alarm02
    alarm03
    alarm05
    alarm06
    alarm07
    bind01
    bind05
    brk01
    brk02
    capget01
    capset01
    capset02
    "

    cd ltp/testcases/bin
    for f in $all_testcases; do
        echo "RUN LTP CASE $f"
        ./$f
        echo "FAIL LTP CASE $f : 0"
    done
    cd ../../..

    echo "#### OS COMP TEST GROUP END ltp-$1 ####"
}

cd /musl
# run_ltp musl
# ./iozone -t 4 -i 0 -i 1 -r 1k -s 1m
# /musl/runtest.exe -w entry-static.exe syscall_sign_extend
# ./libctest_testcode.sh
# ./basic_testcode.sh
# ./lua_testcode.sh
# ./busybox_testcode.sh
# ./iozone_testcode.sh
# ./lmbench_testcode.sh
# ./libcbench_testcode.sh
# ./iperf_testcode.sh
# ./netperf_testcode.sh

cd /glibc
# ./basic_testcode.sh
# ./lua_testcode.sh
# ./busybox_testcode.sh
# ./iozone_testcode.sh
# ./lmbench_testcode.sh
# ./libcbench_testcode.sh
# ./iperf_testcode.sh
./netperf_testcode.sh
# ./cyclictest_testcode.sh
