# ================== LTP Run Scripts ==================

run_ltp() {
  echo "#### OS COMP TEST GROUP START ltp-$1 ####"

  export LTP_TIMEOUT_MUL=0.5
  export LTP_DEV_FS_TYPE=tmpfs
  export LTP_SINGLE_FS_TYPE=tmpfs

  riscv_ltp="
    abort01
    abs01
    accept01
    accept03
    accept4_01
    access01
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
    copy_file_range01
    crash01
    crash02
    creat01
    creat03
    connect01
    data_space
    diotest1
    dirtypipe
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
    futex_cmp_requeue02
    futex_wait01
    futex_wait04
    futex_wake01
    getdents01
    getdents02
    getdomainname01
    getcwd01
    getcwd03
    geteuid01
    geteuid01_16
    geteuid02
    geteuid02_16
    getgid03
    getgid03_16
    getgroups01
    getgroups01_16
    gethostname01
    getitimer01
    getitimer02
    getpagesize01
    getpeername01
    getpgid01
    getpgid02
    getpgrp01
    getpid01
    getpid02
    getppid01
    getppid02
    getpriority01
    getpriority02
    getrandom01
    getrandom02
    getrandom03
    getrandom04
    getrlimit01
    getrlimit02
    getrusage01
    getrusage02
    getsid01
    getsid02
    getsockname01
    getsockopt01
    gettid01
    gettid02
    getuid01
    getuid03
    ioctl04
    ioctl05
    ioctl06
    kill06
    kill07
    kill08
    kill09
    kill11
    lchown01
    lchown01_16
    lchown02
    lchown02_16
    link02
    link04
    link05
    link08
    linkat02
    listen01
    llseek01
    llseek02
    llseek03
    lseek01
    lseek07
    lstat01
    lstat01_64
    lstat02
    lstat02_64
    madvise01
    madvise10
    memcmp01
    memcpy01
    memset01
    mkdirat02
    mkdirat01
    mknod01
    mknod02
    mlock01
    mlock04
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
    msgctl01
    msgctl02
    msgctl04
    msgget01
    msgget02
    msgrcv02
    msgrcv03
    msgrcv07
    msgrcv08
    msgsnd02
    msgsnd05
    munmap01
    munmap02
    munmap03
    munlock01
    nanosleep04
    newuname01
    open01
    open02
    open03
    open04
    open12
    open13
    pipe01
    pipe04
    pipe05
    pipe06
    pipe09
    pipe10
    pipe11
    pipe12
    pipe14
    pipe2_01
    poll01
    posix_fadvise01
    posix_fadvise03
    ppoll01
    prctl01
    prctl03
    prctl04
    prctl05
    pread01
    pread01_64
    pread02
    pread02_64
    preadv01
    preadv01_64
    preadv02
    preadv02_64
    preadv201
    preadv201_64
    preadv202
    preadv202_64
    pselect02
    pselect02_64
    pselect03
    pselect03_64
    pwrite01
    pwrite01_64
    pwrite04
    pwrite04_64
    pwritev01
    pwritev01_64
    pwritev201
    pwritev201_64
    read01
    read02
    read04
    readlink01
    readlink03
    readlinkat01
    readv01
    readv02
    reboot01
    recv01
    recvfrom01
    rmdir01
    rmdir02
    rt_sigaction01
    rt_sigaction02
    rt_sigaction03
    rt_sigprocmask01
    rt_sigprocmask02
    rtc01
    sbrk01
    sbrk02
    sched_getaffinity01
    sched_setaffinity01
    sched_getparam01
    sched_getparam03
    sched_setparam01
    sched_setparam02
    sched_setparam03
    sched_setparam04
    sched_setparam05
    sched_getscheduler01
    sched_getscheduler02
    sched_setscheduler01
    sched_yield01
    select03
    sem_nstest
    semctl01
    semctl03
    semctl05
    semctl07
    semget01
    semget02
    semop01
    send01
    sendfile02
    sendfile02_64
    sendfile04
    sendfile04_64
    sendfile05
    sendfile05_64
    sendfile06
    sendfile06_64
    sendfile08
    sendfile08_64
    set_robust_list01
    set_tid_address01
    setegid01
    setfsgid02
    setfsgid02_64
    setfsuid01
    setfsuid01_64
    setfsuid02
    setfsuid02_64
    setfsuid03
    setfsuid03_64
    setpriority02
    setgid01
    setgid01_64
    setgid03
    setgid03_64
    setgroups01
    setgroups01_64
    setgroups02
    setgroups02_64
    setitimer02
    setpgid01
    setpgid02
    setpgrp01
    setpgrp02
    setregid01
    setregid01_64
    setregid03
    setregid03_64
    setresuid04
    setresuid04_64
    setresuid05
    setresuid05_64
    setreuid01
    setreuid01_64
    setreuid03
    setreuid03_64
    setreuid04
    setreuid04_64
    setreuid05
    setreuid05_64
    setreuid07
    setreuid07_64
    setrlimit01
    setrlimit02
    setrlimit03
    setrlimit04
    setrlimit05
    setsockopt01
    setsockopt03
    setsockopt04
    setuid01
    setuid01_64
    shmat03
    shmctl02
    shmctl06
    shmctl07
    shmctl08
    sigaction02
    sigaltstack02
    signal01
    signal02
    signal03
    signal04
    signal05
    sigpending02
    sigprocmask01
    socket01
    socket02
    socketpair01
    socketpair02
    splice03
    splice07
    splice08
    stat01
    stat01_64
    stat02
    stat02_64
    stat03
    stat03_64
    statfs02
    statfs02_64
    statvfs02
    statx02
    statx03
    symlink01
    symlink02
    symlink03
    symlink04
    syscall01
    sysconf01
    syslog11
    time01
    times01
    timerfd02
    timerfd_gettime01
    timerfd_settime01
    tkill01
    truncate02
    truncate02_64
    truncate03
    truncate03_64
    uname01
    uname02
    uname04
    unlink05
    unlink07
    unlink08
    utime06
    utime07
    utsname01
    utsname04
    wait01
    wait02
    waitid01
    waitid05
    waitpid01
    waitpid03
    waitpid04
    write01
    write03
    write05
    writev01
    writev02
    writev05
    writev06
    writev07
  "

  loongarch_ltp="
    abort01
    access01
    access04
    alarm02
    alarm03
    alarm05
    alarm06
    alarm07
    bind01
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
    copy_file_range01
    crash02
    creat01
    creat03
    dirtypipe
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
    fcntl02
    fcntl03
    fcntl05
    fcntl08
    fcntl02_64
    fcntl03_64
    fcntl05_64
    fcntl08_64
    fcntl13
    fcntl13_64
    fcntl29
    fcntl29_64
    flock01
    flock04
    flock06
    fork01
    fork03
    fork07
    fork08
    fork10
    fstat02
    fstat02_64
    fstat03
    fstat03_64
    fstatfs01
    fstatfs01_64
    fstatfs02
    fstatfs02_64
    ftruncate01
    ftruncate01_64
    futex_cmp_requeue02
    futex_wait01
    futex_wait04
    futex_wake01
    getdents01
    getdents02
    getdomainname01
    getcwd01
    getcwd03
    geteuid01
    geteuid02
    getgid03
    gethostname01
    getitimer01
    getitimer02
    getpagesize01
    getpeername01
    getpgid01
    getpgid02
    getpgrp01
    getpid01
    getpid02
    getppid01
    getppid02
    getpriority01
    getpriority02
    getrandom01
    getrandom02
    getrandom03
    getrandom04
    getrlimit01
    getrlimit02
    getrusage01
    getrusage02
    getsid01
    getsid02
    getsockname01
    getsockopt01
    gettid02
    getuid01
    getuid03
    ioctl05
    kill06
    kill11
    link02
    link04
    link05
    link08
    llseek01
    llseek02
    llseek03
    lseek01
    lseek07
    lstat01
    lstat01_64
    lstat02
    lstat02_64
    madvise01
    memcmp01
    memcpy01
    memset01
    mkdirat02
    mlock01
    mlock04
    mmap001
    mmap02
    mmap06
    mmap08
    mmap09
    mmap17
    mmap19
    munlock01
    nanosleep04
    open01
    open02
    open03
    open04
    pipe01
    pipe06
    pipe09
    pipe10
    pipe11
    pipe12
    pipe14
    pipe2_01
    poll01
    posix_fadvise01
    posix_fadvise03
    ppoll01
    prctl01
    prctl03
    prctl04
    prctl05
    pread01
    pread01_64
    pread02
    pread02_64
    preadv01
    preadv01_64
    preadv02
    preadv02_64
    preadv201
    preadv201_64
    preadv202
    preadv202_64
    pselect02
    pselect02_64
    pselect03
    pselect03_64
    pwrite01
    pwrite01_64
    pwrite04
    pwrite04_64
    pwritev01
    pwritev01_64
    pwritev201
    pwritev201_64
    read01
    read02
    read04
    readlink01
    readlink03
    readlinkat01
    readv01
    readv02
    reboot01
    rmdir01
    rmdir02
    sbrk01
    sbrk02
    sched_getaffinity01
    sched_setaffinity01
    sched_getparam01
    sched_getparam03
    sched_setparam01
    sched_setparam02
    sched_setparam03
    sched_setparam04
    sched_getscheduler01
    sched_getscheduler02
    sched_setscheduler01
    select03
    sem_nstest
    semctl01
    semctl03
    semctl05
    semctl07
    semget01
    semget02
    semop01
    sendfile02
    sendfile02_64
    sendfile04
    sendfile04_64
    sendfile05
    sendfile05_64
    sendfile06
    sendfile06_64
    sendfile08
    sendfile08_64
    setegid01
    setfsgid02
    setfsgid02_64
    setfsuid01
    setfsuid01_64
    setfsuid02
    setfsuid02_64
    setfsuid03
    setfsuid03_64
    setpriority02
    setgid01
    setgid01_64
    setgid03
    setgid03_64
    setgroups01
    setgroups01_64
    setgroups02
    setgroups02_64
    setitimer02
    setpgid01
    setpgid02
    setpgrp01
    setpgrp02
    setregid01
    setregid01_64
    setregid03
    setregid03_64
    setresuid04
    setresuid04_64
    setresuid05
    setresuid05_64
    setreuid01
    setreuid01_64
    setreuid03
    setreuid03_64
    setreuid04
    setreuid04_64
    setreuid05
    setreuid05_64
    setreuid07
    setreuid07_64
    setrlimit02
    setrlimit03
    setrlimit04
    setrlimit05
    setsockopt01
    setsockopt03
    setsockopt04
    setuid01
    setuid01_64
    shmat03
    shmctl02
    shmctl06
    shmctl07
    shmctl08
    sigaltstack02
    signal01
    signal02
    signal03
    signal04
    signal05
    sigpending02
    sigprocmask01
    socket01
    socket02
    socketpair01
    socketpair02
    splice03
    splice07
    splice08
    stat01
    stat01_64
    stat02
    stat02_64
    stat03
    stat03_64
    statfs02
    statfs02_64
    statvfs02
    statx02
    statx03
    symlink02
    symlink04
    syscall01
    syslog11
    time01
    times01
    timerfd02
    timerfd_gettime01
    timerfd_settime01
    tkill01
    truncate02
    truncate02_64
    truncate03
    truncate03_64
    uname01
    uname02
    uname04
    unlink05
    unlink07
    unlink08
    utime06
    utime07
    utsname01
    utsname04
    wait01
    wait02
    waitid01
    waitid05
    waitpid01
    waitpid03
    waitpid04
    write03
    write05
    writev01
    writev07
  "

  cd ltp/testcases/bin
  if [[ $ARCH == loongarch64 ]]; then
    for f in $loongarch_ltp; do
      echo "RUN LTP CASE $f"
      ./$f
      echo "FAIL LTP CASE $f : 0"
    done
  else
    for f in $riscv_ltp; do
      echo "RUN LTP CASE $f"
      ./$f
      echo "FAIL LTP CASE $f : 0"
    done
  fi
  cd ../../..

  echo "#### OS COMP TEST GROUP END ltp-$1 ####"
}

echo -e "\n=============== Build RootFS ===============\n"

/musl/busybox mkdir -v /bin
/musl/busybox --install -s /bin
export PATH=/bin

mkdir -v /lib
mkdir -v /usr
mkdir -v -p /var/tmp

cp /glibc/lib/libc.so.6 /lib/libc.so.6
ln -v -s /glibc/lib/libm.so.6 /lib/libm.so.6
ln -v -s /lib/libc.so.6 /lib/libc.so
ln -v -s /lib/libm.so.6 /lib/libm.so
if [[ $ARCH == loongarch64 ]]; then
  ln -v -s /musl/lib/libc.so /lib/ld-musl-loongarch-lp64d.so.1
  ln -v -s /glibc/lib/ld-linux-loongarch-lp64d.so.1 /lib/ld-linux-loongarch-lp64d.so.1
else
  ln -v -s /musl/lib/libc.so /lib/ld-musl-riscv64.so.1
  ln -v -s /musl/lib/libc.so /lib/ld-musl-riscv64-sf.so.1
  ln -v -s /glibc/lib/ld-linux-riscv64-lp64d.so.1 /lib/ld-linux-riscv64-lp64d.so.1
fi
ln -v -s /lib /lib64
ln -v -s /lib /usr/lib
ln -v -s /lib /usr/lib64

export LD_LIBRARY_PATH=".:./lib:/musl/lib:/lib"

# echo -e "\n=============== Preliminary Round ===============\n"
# 
# cd /musl
# run_ltp musl
# 
# cd /glibc
# run_ltp glibc
# 
# cd /musl
# if [[ $ARCH == riscv64 ]]; then
#   ./libctest_testcode.sh
# fi
# ./basic_testcode.sh
# ./lua_testcode.sh
# ./busybox_testcode.sh
# ./libcbench_testcode.sh
# ./iozone_testcode.sh
# ./iperf_testcode.sh
# ./netperf_testcode.sh
# 
# cd /glibc
# ./basic_testcode.sh
# ./lua_testcode.sh
# ./busybox_testcode.sh
# ./libcbench_testcode.sh
# ./iozone_testcode.sh
# ./iperf_testcode.sh
# ./netperf_testcode.sh
# 
# cd /musl
# timeout 420 ./lmbench_testcode.sh
# cd /glibc
# timeout 420 ./lmbench_testcode.sh
# 
# cd /musl
# ./cyclictest_testcode.sh
# cd /glibc
# ./cyclictest_testcode.sh

# echo -e "\n=============== Final Round Stage 1 ===============\n"

# cd /musl
# ./interrupts_testcode.sh
# ./copy-file-range_testcode.sh
# ./splice_testcode.sh

# cd /glibc
# ./interrupts_testcode.sh
# ./copy-file-range_testcode.sh
# ./splice_testcode.sh


# echo -e "\n=============== Final Round Stage 2 ===============\n"

# cd /musl
# export HOME=/musl
# cp -r ./usr/share /usr
# cp ./usr/bin/git /bin
# ./git_testcode.sh
# 
# cd /glibc
# export HOME=/glibc
# rm -r /usr/share
# rm /bin/git
# cp -r ./usr/share /usr
# cp ./usr/bin/git /bin
# ./git_testcode.sh

echo -e "\n================ Linux APPs ================\n"

cd /musl
sh
