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
fi

ln -v -s /lib /lib64

export LD_LIBRARY_PATH=.

echo @@@@@@@@@@ files @@@@@@@@@@
ls -lhAR /
echo @@@@@@@@@@ env @@@@@@@@@@
env
echo

echo @@@@@@@@@@ musl @@@@@@@@@@
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

# echo @@@@@@@@@@ glibc @@@@@@@@@@
# cd /musl
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
#echo "#### OS COMP TEST GROUP START lmbench-glibc ####"
#./unixbench_testcode.sh
#echo "#### OS COMP TEST GROUP END lmbench-glibc ####"
#
# /musl/runtest.exe -w entry-static.exe argv
# /musl/runtest.exe -w entry-static.exe basename
# /musl/runtest.exe -w entry-static.exe clocale_mbfuncs
# /musl/runtest.exe -w entry-static.exe clock_gettime
# /musl/runtest.exe -w entry-static.exe dirname
# /musl/runtest.exe -w entry-static.exe env
# /musl/runtest.exe -w entry-static.exe fdopen
# /musl/runtest.exe -w entry-static.exe fnmatch
# /musl/runtest.exe -w entry-static.exe fscanf
# /musl/runtest.exe -w entry-static.exe fwscanf
# /musl/runtest.exe -w entry-static.exe iconv_open
# /musl/runtest.exe -w entry-static.exe inet_pton
# /musl/runtest.exe -w entry-static.exe mbc
# /musl/runtest.exe -w entry-static.exe memstream
# /musl/runtest.exe -w entry-static.exe pthread_cancel_points
# /musl/runtest.exe -w entry-static.exe pthread_cancel
# /musl/runtest.exe -w entry-static.exe pthread_cond
# /musl/runtest.exe -w entry-static.exe pthread_tsd
# /musl/runtest.exe -w entry-static.exe qsort
# /musl/runtest.exe -w entry-static.exe random
# /musl/runtest.exe -w entry-static.exe search_hsearch
# /musl/runtest.exe -w entry-static.exe search_insque
# /musl/runtest.exe -w entry-static.exe search_lsearch
# /musl/runtest.exe -w entry-static.exe search_tsearch
# /musl/runtest.exe -w entry-static.exe setjmp
# /musl/runtest.exe -w entry-static.exe snprintf
# /musl/runtest.exe -w entry-static.exe socket
# /musl/runtest.exe -w entry-static.exe sscanf
# /musl/runtest.exe -w entry-static.exe sscanf_long
# /musl/runtest.exe -w entry-static.exe stat
# /musl/runtest.exe -w entry-static.exe strftime
# /musl/runtest.exe -w entry-static.exe string
# /musl/runtest.exe -w entry-static.exe string_memcpy
# /musl/runtest.exe -w entry-static.exe string_memmem
# /musl/runtest.exe -w entry-static.exe string_memset
# /musl/runtest.exe -w entry-static.exe string_strchr
# /musl/runtest.exe -w entry-static.exe string_strcspn
# /musl/runtest.exe -w entry-static.exe string_strstr
# /musl/runtest.exe -w entry-static.exe strptime
# /musl/runtest.exe -w entry-static.exe strtod
# /musl/runtest.exe -w entry-static.exe strtod_simple
# /musl/runtest.exe -w entry-static.exe strtof
# /musl/runtest.exe -w entry-static.exe strtol
# /musl/runtest.exe -w entry-static.exe strtold
# /musl/runtest.exe -w entry-static.exe swprintf
# /musl/runtest.exe -w entry-static.exe tgmath
# /musl/runtest.exe -w entry-static.exe time
# /musl/runtest.exe -w entry-static.exe tls_align
# /musl/runtest.exe -w entry-static.exe udiv
# /musl/runtest.exe -w entry-static.exe ungetc
# /musl/runtest.exe -w entry-static.exe utime
# /musl/runtest.exe -w entry-static.exe wcsstr
# /musl/runtest.exe -w entry-static.exe wcstol
# /musl/runtest.exe -w entry-static.exe daemon_failure
# /musl/runtest.exe -w entry-static.exe dn_expand_empty
# /musl/runtest.exe -w entry-static.exe dn_expand_ptr_0
# /musl/runtest.exe -w entry-static.exe fflush_exit
# /musl/runtest.exe -w entry-static.exe fgets_eof
# /musl/runtest.exe -w entry-static.exe fgetwc_buffering
# /musl/runtest.exe -w entry-static.exe fpclassify_invalid_ld80
# /musl/runtest.exe -w entry-static.exe ftello_unflushed_append
# /musl/runtest.exe -w entry-static.exe getpwnam_r_crash
# /musl/runtest.exe -w entry-static.exe getpwnam_r_errno
# /musl/runtest.exe -w entry-static.exe iconv_roundtrips
# /musl/runtest.exe -w entry-static.exe inet_ntop_v4mapped
# /musl/runtest.exe -w entry-static.exe inet_pton_empty_last_field
# /musl/runtest.exe -w entry-static.exe iswspace_null
# /musl/runtest.exe -w entry-static.exe lrand48_signextend
# /musl/runtest.exe -w entry-static.exe lseek_large
# /musl/runtest.exe -w entry-static.exe malloc_0
# /musl/runtest.exe -w entry-static.exe mbsrtowcs_overflow
# /musl/runtest.exe -w entry-static.exe memmem_oob_read
# /musl/runtest.exe -w entry-static.exe memmem_oob
# /musl/runtest.exe -w entry-static.exe mkdtemp_failure
# /musl/runtest.exe -w entry-static.exe mkstemp_failure
# /musl/runtest.exe -w entry-static.exe printf_1e9_oob
# /musl/runtest.exe -w entry-static.exe printf_fmt_g_round
# /musl/runtest.exe -w entry-static.exe printf_fmt_g_zeros
# /musl/runtest.exe -w entry-static.exe printf_fmt_n
# /musl/runtest.exe -w entry-static.exe pthread_robust_detach
# /musl/runtest.exe -w entry-static.exe pthread_cancel_sem_wait
# /musl/runtest.exe -w entry-static.exe pthread_cond_smasher
# /musl/runtest.exe -w entry-static.exe pthread_condattr_setclock
# /musl/runtest.exe -w entry-static.exe pthread_exit_cancel
# /musl/runtest.exe -w entry-static.exe pthread_once_deadlock
# /musl/runtest.exe -w entry-static.exe pthread_rwlock_ebusy
# /musl/runtest.exe -w entry-static.exe putenv_doublefree
# /musl/runtest.exe -w entry-static.exe regex_backref_0
# /musl/runtest.exe -w entry-static.exe regex_bracket_icase
# /musl/runtest.exe -w entry-static.exe regex_ere_backref
# /musl/runtest.exe -w entry-static.exe regex_escaped_high_byte
# /musl/runtest.exe -w entry-static.exe regex_negated_range
# /musl/runtest.exe -w entry-static.exe regexec_nosub
# /musl/runtest.exe -w entry-static.exe rewind_clear_error
# /musl/runtest.exe -w entry-static.exe rlimit_open_files
# /musl/runtest.exe -w entry-static.exe scanf_bytes_consumed
# /musl/runtest.exe -w entry-static.exe scanf_match_literal_eof
# /musl/runtest.exe -w entry-static.exe scanf_nullbyte_char
# /musl/runtest.exe -w entry-static.exe setvbuf_unget
# /musl/runtest.exe -w entry-static.exe sigprocmask_internal
# /musl/runtest.exe -w entry-static.exe sscanf_eof
# /musl/runtest.exe -w entry-static.exe statvfs
# /musl/runtest.exe -w entry-static.exe strverscmp
# /musl/runtest.exe -w entry-static.exe syscall_sign_extend
# /musl/runtest.exe -w entry-static.exe uselocale_0
# /musl/runtest.exe -w entry-static.exe wcsncpy_read_overflow
# /musl/runtest.exe -w entry-static.exe wcsstr_false_negative
