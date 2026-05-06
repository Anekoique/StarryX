/* Linux-compatible vDSO symbol versioning. musl/glibc probe `__vdso_*`
 * names under the `LINUX_2.6` tag. */
LINUX_2.6 {
    global:
        __vdso_clock_gettime;
        __vdso_gettimeofday;
        __vdso_clock_getres;
        __vdso_time;
        __vdso_getcpu;
        __vdso_rt_sigreturn;
    local: *;
};
