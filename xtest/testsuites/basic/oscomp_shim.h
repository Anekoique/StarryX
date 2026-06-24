/* oscomp_shim.h — reconcile the rCore-lineage OS-COMP `basic` syscall tests
 * with a musl toolchain.
 *
 * Force-included (-include) ahead of every translation unit. The vendored
 * sources `#include "unistd.h"` / `"stdio.h"` / `"stdlib.h"` / `"string.h"` /
 * `"stddef.h"`; with no first-party headers vendored alongside them, those
 * quoted includes resolve to the musl system headers. This shim supplies only
 * the rCore-isms that musl does not, and routes the calls whose rCore
 * signatures clash with musl's prototypes through raw syscalls so no
 * conflicting declaration is ever pulled in.
 *
 * Surgical: every entry below is triggered by an idiom that actually appears in
 * the vendored src sources. No speculative shims.
 */
#ifndef OSCOMP_SHIM_H
#define OSCOMP_SHIM_H

/* Standard musl headers the sources lean on but never include directly. */
#include <fcntl.h>
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <time.h>
#include <signal.h>          /* SIGCHLD (clone.c) */
#include <sys/syscall.h>     /* SYS_* numbers for the raw-syscall wrappers */
#include <sys/wait.h>        /* wait/waitpid/WEXITSTATUS (exit/fork/wait/...) */
#include <sched.h>           /* sched_yield (yield.c, waitpid.c) */
#include <sys/mman.h>        /* mmap/munmap, PROT_*, MAP_* (mmap.c, munmap.c) */
#include <sys/mount.h>       /* mount/umount (mount.c, umount.c) */

/* ---- test harness banners (first-party stdio.h) ----------------------------
 * Upstream TEST_START/TEST_END used a newline-free puts(); musl's puts()
 * appends '\n', so reproduce the exact banner text with printf() instead. */
#undef TEST_START
#undef TEST_END
#define TEST_START(x) do { printf("========== START "); printf("%s", (x)); printf(" ==========\n"); } while (0)
#define TEST_END(x)   do { printf("========== END "); printf("%s", (x)); printf(" ==========\n"); } while (0)

/* ---- assert -> panic (first-party stdlib.h) -------------------------------- */
__attribute__((noreturn)) static void panic(const char *msg)
{
    fputs(msg, stderr);
    _exit(1);
}
#undef assert
#define assert(f) do { if (!(f)) panic("\n --- Assert Fatal ! ---\n"); } while (0)

/* ---- *_FILENO aliases (first-party stdio.h) -------------------------------- */
#ifndef STDIN
#define STDIN  STDIN_FILENO
#endif
#ifndef STDOUT
#define STDOUT STDOUT_FILENO
#endif
#ifndef STDERR
#define STDERR STDERR_FILENO
#endif

/* ---- open flag spelling (first-party stddef.h) ----------------------------- */
#ifndef O_CREATE
#define O_CREATE O_CREAT
#endif

/* ---- struct kstat + fstat (first-party stddef.h / unistd.h) ----------------
 * Mirrors the Linux `struct stat` layout the kernel fills, exposing the
 * st_size / st_*time_sec fields the tests print. We define our own `fstat` so
 * musl's `int fstat(int, struct stat *)` is never declared and cannot clash.
 *
 * Arch portability: rv64 has SYS_fstat (fills this layout directly); loongarch64
 * has neither SYS_fstat nor SYS_newfstatat — only SYS_statx. So on la64 we call
 * statx and copy the fields the tests read into struct kstat. */
struct kstat {
    unsigned long long st_dev;
    unsigned long long st_ino;
    unsigned int       st_mode;
    unsigned int       st_nlink;
    unsigned int       st_uid;
    unsigned int       st_gid;
    unsigned long long st_rdev;
    unsigned long long __pad;
    long long          st_size;
    int                st_blksize;
    int                __pad2;
    long long          st_blocks;
    long               st_atime_sec;
    long               st_atime_nsec;
    long               st_mtime_sec;
    long               st_mtime_nsec;
    long               st_ctime_sec;
    long               st_ctime_nsec;
    unsigned           __pad3[2];
};
#ifdef SYS_fstat
static int fstat(int fd, struct kstat *st)
{
    return (int)syscall(SYS_fstat, fd, st);
}
#else
/* loongarch64: synthesize from statx. */
struct shim_statx_ts { long long tv_sec; unsigned int tv_nsec; unsigned int __r; };
struct shim_statx {
    unsigned int stx_mask, stx_blksize, stx_attributes_lo32, stx_attributes_hi32;
    unsigned int stx_nlink, stx_uid, stx_gid, stx_mode; unsigned short __sp[1];
    unsigned long long stx_ino, stx_size, stx_blocks, stx_attributes_mask;
    struct shim_statx_ts stx_atime, stx_btime, stx_ctime, stx_mtime;
    unsigned int stx_rdev_major, stx_rdev_minor, stx_dev_major, stx_dev_minor;
    unsigned long long __spare[14];
};
static int fstat(int fd, struct kstat *st)
{
    struct shim_statx sx;
    /* AT_EMPTY_PATH=0x1000, STATX_BASIC_STATS=0x7ff */
    long r = syscall(SYS_statx, fd, "", 0x1000, 0x7ff, &sx);
    if (r != 0) return (int)r;
    st->st_size      = (long long)sx.stx_size;
    st->st_blksize   = (int)sx.stx_blksize;
    st->st_blocks    = (long long)sx.stx_blocks;
    st->st_mode      = sx.stx_mode;
    st->st_nlink     = sx.stx_nlink;
    st->st_atime_sec = (long)sx.stx_atime.tv_sec;
    st->st_mtime_sec = (long)sx.stx_mtime.tv_sec;
    st->st_ctime_sec = (long)sx.stx_ctime.tv_sec;
    return 0;
}
#endif

/* ---- get_time() (first-party unistd.h) ------------------------------------
 * gettimeofday.c / sleep.c read a monotonic tick count. */
static long get_time(void)
{
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0)
        return -1;
    return (long)(ts.tv_sec * 1000 + ts.tv_nsec / 1000000);
}

/* ---- clone(fn, arg, stack, stack_size, flags) (first-party unistd.h) -------
 * rCore signature; musl's clone() prototype differs, so wrap the raw syscall.
 * Stack grows down, so hand the kernel the top of the buffer. */
static int clone(int (*fn)(void *), void *arg, void *stack,
                 unsigned long stack_size, unsigned long flags)
{
    char *sp = (char *)stack + stack_size;
    int pid = (int)syscall(SYS_clone, flags, sp, 0, 0, 0);
    if (pid == 0) {
        _exit(fn(arg));
    }
    return pid;
}

/* ---- getdents(fd, dirp64, len) + struct linux_dirent64 (first-party) -------
 * musl exposes only getdents64 via <dirent.h>; the test uses the bare-metal
 * struct + getdents() name, so define both against SYS_getdents64. */
struct linux_dirent64 {
    unsigned long long d_ino;
    long long          d_off;
    unsigned short     d_reclen;
    unsigned char      d_type;
    char               d_name[];
};
static int getdents(int fd, struct linux_dirent64 *dirp64, unsigned long len)
{
    return (int)syscall(SYS_getdents64, fd, dirp64, len);
}

/* ---- brk(addr) returning the resulting break (first-party unistd.h) --------
 * brk.c expects brk(0) to report the current break and brk(p) to report the
 * new one; musl's brk() returns 0/-1, so call SYS_brk directly. */
static long brk_(void *addr)
{
    return (long)syscall(SYS_brk, addr);
}
#define brk(addr) brk_((void *)(addr))

/* ---- uname(buf) (first-party unistd.h) ------------------------------------
 * uname.c declares its own struct utsname and calls uname(&un); avoid pulling
 * <sys/utsname.h> (which would redeclare both) by routing through SYS_uname. */
static int uname(void *buf)
{
    return (int)syscall(SYS_uname, buf);
}

/* ---- times(buf) (first-party unistd.h) ------------------------------------
 * times.c declares its own struct tms and calls times(&mytimes); avoid
 * <sys/times.h> by routing through SYS_times. */
static long times(void *buf)
{
    return (long)syscall(SYS_times, buf);
}

#endif /* OSCOMP_SHIM_H */
