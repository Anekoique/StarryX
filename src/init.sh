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

mkdir -v /lib
mkdir -v /etc
cp -v /glibc/lib/* /lib
if [[ $ARCH == loongarch64 ]]; then
    ln -v -s /musl/lib/libc.so /lib/ld-musl-loongarch-lp64d.so.1
else
    ln -v -s /musl/lib/libc.so /lib/ld-musl-$ARCH.so.1
    ln -v -s /musl/lib/libc.so /lib/ld-musl-$ARCH-sf.so.1
fi

ln -v -s /lib /lib64

export LD_LIBRARY_PATH=.

# echo @@@@@@@@@@ files @@@@@@@@@@
# ls -lhAR /
# echo @@@@@@@@@@ env @@@@@@@@@@
# env
# echo

# echo @@@@@@@@@@ musl @@@@@@@@@@
cd /musl
echo "#### OS COMP TEST GROUP START basic-glibc ####"
./basic_testcode.sh
echo "#### OS COMP TEST GROUP END basic-glibc ####"
echo "#### OS COMP TEST GROUP START lua-glibc ####"
./lua_testcode.sh
echo "#### OS COMP TEST GROUP END lua-glibc ####"
echo "#### OS COMP TEST GROUP START libctest-glibc ####"
./libctest_testcode.sh
echo "#### OS COMP TEST GROUP END libctest-glibc ####"
echo "#### OS COMP TEST GROUP START busybox-glibc ####"
./busybox_testcode.sh
echo "#### OS COMP TEST GROUP END busybox-glibc ####"
echo "#### OS COMP TEST GROUP START iozone-glibc ####"
./iozone_testcode.sh
echo "#### OS COMP TEST GROUP END iozone-glibc ####"
echo "#### OS COMP TEST GROUP START lmbench-musl ####"
./lmbench_testcode.sh
echo "#### OS COMP TEST GROUP END lmbench-musl ####"
# echo "#### OS COMP TEST GROUP START lmbench-glibc ####"
# ./unixbench_testcode.sh
# echo "#### OS COMP TEST GROUP END lmbench-glibc ####"
# echo "#### OS COMP TEST GROUP START libcbench-glibc ####"
# ./libc-bench
# echo "#### OS COMP TEST GROUP END libcbench-glibc ####"
# echo "#### OS COMP TEST GROUP START cyclictest-glibc ####"
# ./cyclictest_testcode.sh
# echo "#### OS COMP TEST GROUP END cyclictest-glibc ####"

# echo @@@@@@@@@@ glibc @@@@@@@@@@
# cd /glibc
# echo "#### OS COMP TEST GROUP START basic-glibc ####"
# ./basic_testcode.sh
# echo "#### OS COMP TEST GROUP END basic-glibc ####"
# echo "#### OS COMP TEST GROUP START lua-glibc ####"
# ./lua_testcode.sh
# echo "#### OS COMP TEST GROUP END lua-glibc ####"
# echo "#### OS COMP TEST GROUP START libctest-glibc ####"
# ./libctest_testcode.sh
# echo "#### OS COMP TEST GROUP END libctest-glibc ####"
# echo "#### OS COMP TEST GROUP START busybox-glibc ####"
# ./busybox_testcode.sh
# echo "#### OS COMP TEST GROUP END busybox-glibc ####"
# echo "#### OS COMP TEST GROUP START iozone-glibc ####"
# ./iozone_testcode.sh
# echo "#### OS COMP TEST GROUP END iozone-glibc ####"
# echo "#### OS COMP TEST GROUP START lmbench-glibc ####"
# ./lmbench_testcode.sh
# echo "#### OS COMP TEST GROUP END lmbench-glibc ####"
# echo "#### OS COMP TEST GROUP START lmbench-glibc ####"
# ./unixbench_testcode.sh
# echo "#### OS COMP TEST GROUP END lmbench-glibc ####"
