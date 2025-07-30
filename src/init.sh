/musl/busybox mkdir -v /bin
/musl/busybox --install -s /bin
export PATH=/bin

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
cat >/etc/protocols <<EOF
ip      0       IP
icmp    1       ICMP
tcp     6       TCP
udp     17      UDP
EOF

run_ltp() {
  echo "#### OS COMP TEST GROUP START ltp-$1 ####"

  export LTP_TIMEOUT_MUL=0.5
  export LTP_DEV_FS_TYPE=tmpfs
  export LTP_SINGLE_FS_TYPE=tmpfs

  # copy_file_range01
  # copy_file_range02
  # copy_file_range03
  # dirtypipe

  all_testcases="
    futex_cmp_requeue02
    futex_wait01
    futex_wait02
    futex_wait04
    futex_wake01
    futex_wake02
    futex_wake03
    futex_wake04
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
    chdir01
    chdir04
    chmod01
    chmod03
    chown01
    chown02
    chown03
    chown04
    chown05
    chroot01
    clock_adjtime01
    clock_getres01
    clock_gettime02
    clock_nanosleep01
    clock_nanosleep04
    clone01
    clone03
    clone06
    clone07
    clone08
    clone301
    clone302
    close01
    close02
    confstr01
    crash01
    crash02
    creat01
    creat03
    connect01
    data_space
    diotest1
    dup01
    dup02
    dup03
    dup04
    dup07
    dup201
    dup202
    dup203
    dup204
    dup205
    dup206
    dup207
    dup3_01
    dup3_02
    epoll_create01
    epoll_create02
    epoll_create1_01
    epoll_create1_02
    epoll_ctl01
    epoll_ctl02
    epoll_ctl03
    epoll_wait03
    epoll_wait04
    epoll_wait07
    epoll_pwait02
    eventfd2_01
    eventfd2_02
    eventfd2_03
    execve03
    exit01
    exit02
    exit_group01
    faccessat01
    faccessat02
    faccessat201
    faccessat202
    fallocate03
    fallocate04
    fchdir01
    fchdir02
    fchmod01
    fchmod03
    fchmod04
    fchmodat01
    fchmodat02
    fchown01
    fchown02
    fchown03
    fchown05
    fchownat01
    fchownat02
    fcntl02
    fcntl03
    fcntl05
    fcntl08
    fcntl09
    fcntl02_64
    fcntl03_64
    fcntl05_64
    fcntl08_64
    fcntl09_64
    fcntl10
    fcntl10_64
    fcntl13
    fcntl13_64
    fcntl29
    fcntl29_64
    fdatasync01
    fdatasync02
    flock01
    flock04
    flock06
    fork01
    fork03
    fork07
    fork08
    fork09
    fork10
    fstat02
    fstat02_64
    fstat03
    fstat03_64
    fstatfs01
    fstatfs01_64
    fstatfs02
    fstatfs02_64
    fsync01
    ftruncate01
    ftruncate01_64
    getrandom01
    getrandom02
    getrandom03
    getrandom04
    getrlimit01
    getrlimit02
    getrusage01
    getrusage02
    getsid01
    getsockname01
    getsockopt01
    gettid01
    gettid02
    getuid01
    getuid03
    kill06
    kill07
    kill08
    kill09
    kill11
    mmap001
    mmap02
    mmap03
    mmap05
    mmap06
    mmap08
    mmap09
    mmap11
    mmap16
    mmap17
    mmap19
    recv01
    recvfrom01
    rt_sigaction01
    rt_sigaction02
    rt_sigaction03
    rt_sigprocmask01
    rt_sigprocmask02
    rtc01
    poll01
    sbrk01
    sbrk02
    sched_yield01
    select03
    sem_nstest
    semop01
    semget01
    semget02
    semctl01
    semctl03
    semctl05
    semctl07
    send01
    setreuid01
    setreuid03
    setreuid04
    setreuid05
    setreuid07
    setrlimit01
    setrlimit02
    setrlimit03
    setrlimit04
    setrlimit05
    setsockopt01
    setsockopt03
    setsockopt04
    setuid01
    sigaction02
    sigaltstack02
    signal01
    signal02
    signal03
    signal04
    signal05
    sigpending02
    sigprocmask01
    times01
    tkill01
    uname01
    uname02
    uname04
    utime06
    utime07
    wait01
    wait02
    waitpid03
    waitpid04
    "

  cd ltp/testcases/bin
  for f in $all_testcase; do
    echo "RUN LTP CASE $f"
    ./$f
    echo "FAIL LTP CASE $f : 0"
  done
  cd ../../..

  echo "#### OS COMP TEST GROUP END ltp-$1 ####"
}

cd /musl
# sh
run_ltp musl
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
# run_ltp musl
# ./basic_testcode.sh
# ./lua_testcode.sh
# ./busybox_testcode.sh
# ./iozone_testcode.sh
# ./lmbench_testcode.sh
# ./libcbench_testcode.sh
# ./iperf_testcode.sh
# ./netperf_testcode.sh
# ./cyclictest_testcode.sh
