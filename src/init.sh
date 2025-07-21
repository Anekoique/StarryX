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
mkdir -v /etc
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
echo "nobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin" >/etc/passwd


cd /musl
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
<<<<<<< Updated upstream


cd /musl/ltp/testcases/bin

ltp_testlist="
accept01
accept4_01
access01
access02
access03
alarm02
alarm03
alarm05
alarm06
alarm07
bind01
bpf_prog01
brk01
brk02
chdir04
chmod01
chroot02
clock_getres01
clock_gettime01
clock_gettime02
clock_gettime04
clock_nanosleep01
clock_nanosleep04
clone01
clone03
clone06
clone07
clone302
close01
close02
confstr01
creat01
creat05
creat08
dup01
dup02
dup03
dup04
dup06
dup201
dup203
dup204
dup205
dup206
dup207
dup3_01
epoll_create01
epoll_create1_01
epoll_create1_02
epoll_ctl01
epoll_ctl02
epoll_ctl03
epoll_pwait03
epoll_wait02
epoll_wait03
epoll_wait04
epoll_wait07
execve03
exit02
faccessat01
faccessat02
fallocate03
fchmod01
fchmod03
fchmod04
fchmodat01
fchmodat02
fchown01
fchown05
fcntl02
fcntl02_64
fcntl03
fcntl03_64
fcntl04
fcntl04_64
fcntl05
fcntl05_64
fcntl08
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
fpathconf01
fstat02
fstat02_64
fstat03
fstat03_64
fsync02
ftruncate01
ftruncate01_64
futex_cmp_requeue02
futex_wait01
futex_wait04
futex_wake01
getcwd01
getdents02
getdomainname01
getegid02
getegid02_16
geteuid01
geteuid02
getgid03
gethostname01
getitimer01
getpagesize01
getpeername01
getpgid01
getpgid02
getpgrp01
getpid01
getpid02
getppid01
getppid02
getpriority02
getrandom01
getrandom02
getrandom03
getrandom04
getrandom05
getrlimit01
getrlimit02
getrlimit03
getrusage01
getsockopt01
gettid01
gettid02
gettimeofday01
getuid01
getuid03
in6_01
in6_02
link02
link05
llseek02
llseek03
lseek01
lseek07
lstat01
lstat01_64
lstat02_64
madvise10
memcmp01
memcpy01
memset01
mkdir05
mknod01
mknod02
mlock01
mmap02
mmap05
mmap06
mmap09
mmap17
mmap19
mq_open01
mq_timedreceive01
mq_unlink01
msgctl01
msgctl02
msgctl03
msgctl06
msgctl12
msgrcv02
name_to_handle_at02
nanosleep04
open01
open02
open03
open04
open07
open08
open10
open11
open_by_handle_at02
openat01
pathconf01
personality01
personality02
pidns05
pipe01
pipe10
pipe11
pipe14
pipe2_01
pivot_root01
poll01
poll02
posix_fadvise03
posix_fadvise03_64
ppoll01
prctl01
prctl05
prctl08
pread01
pread01_64
pread02
pread02_64
preadv01
preadv01_64
preadv02
preadv02_64
pselect01
pselect01_64
pselect03
pselect03_64
pwrite01
pwrite01_64
pwrite02_64
pwrite04
pwrite04_64
pwritev01
pwritev01_64
pwritev02
pwritev02_64
read01
readdir01
readlink01
readlink03
readlinkat01
readlinkat02
readv01
realpath01
rmdir01
sbrk01
sbrk02
sched_getaffinity01
sched_getscheduler01
sched_rr_get_interval03
sched_setaffinity01
sched_setparam01
select02
select03
semctl03
semctl07
sendfile02
sendfile02_64
sendfile03
sendfile03_64
sendfile04
sendfile04_64
sendfile08
sendfile08_64
setdomainname02
setegid01
setfsgid01
setfsgid02
setgid01
setgid03
sethostname02
setpgid02
setpgid03
setpgrp02
setpriority02
setregid03
setregid04
setresuid04
setresuid05
setreuid01
setreuid03
setreuid04
setreuid05
setreuid07
setrlimit02
setrlimit03
setrlimit05
setsockopt03
setuid01
setxattr02
shmat02
shmat03
shmctl02
shmdt02
sigaltstack02
signal01
signal02
signal03
signal04
signal05
sigpending02
socket01
socket02
socketpair02
splice07
stat01
stat01_64
stat02
stat02_64
stat03
stat03_64
statvfs02
statx01
statx02
statx03
symlink02
symlink04
syscall01
syslog11
tgkill03
thp01
time01
timerfd02
times01
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
unshare01
utime07
utsname01
utsname03
utsname04
wait01
wait02
wait401
wait402
waitid05
waitid06
waitpid01
waitpid03
waitpid04
write01
write03
write05
write06
"
echo "#### OS COMP TEST GROUP START ltp-musl ####"

# 定义目标目录
target_dir="ltp/"

# 遍历目录下的所有文件
for file in $ltp_testlist; do
  # 跳过目录，仅处理文件
  if [ -f "$file" ]; then
    # 输出文件名
    echo "RUN LTP CASE $file"

    "./$file"
    ret=$?

    # 输出文件名和返回值
    echo "FAIL LTP CASE $file : $ret"
  fi
done


echo "#### OS COMP TEST GROUP END ltp-musl ####"
=======
./netperf_testcode.sh
>>>>>>> Stashed changes

cd /glibc
# ./basic_testcode.sh
# ./lua_testcode.sh
# ./busybox_testcode.sh
# ./iozone_testcode.sh
# ./lmbench_testcode.sh
# ./libcbench_testcode.sh
# ./cyclictest_testcode.sh
# ./iperf_testcode.sh
