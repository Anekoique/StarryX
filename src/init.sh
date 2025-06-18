echo @@@@@@@@@@ setup @@@@@@@@@@

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

# echo @@@@@@@@@@ files @@@@@@@@@@
# ls -lhAR /
# echo @@@@@@@@@@ env @@@@@@@@@@
# env
# echo

# echo @@@@@@@@@@ musl @@@@@@@@@@
cd /musl
./basic_testcode.sh
./lua_testcode.sh
./busybox_testcode.sh
./iozone_testcode.sh
./lmbench_testcode.sh
./libcbench_testcode.sh
# ./unixbench_testcode.sh
# ./cyclictest_testcode.sh

# echo @@@@@@@@@@ glibc @@@@@@@@@@
cd /glibc
./basic_testcode.sh
./lua_testcode.sh
./busybox_testcode.sh
./iozone_testcode.sh
./lmbench_testcode.sh
./libcbench_testcode.sh
# ./unixbench_testcode.sh
# ./cyclictest_testcode.sh
