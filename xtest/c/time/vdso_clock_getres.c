// vdso_clock_getres.c — verify the vDSO clock_getres entry returns 1 ns
// for the supported clocks (matches the kernel-side syscall behavior).

#include "common/assert.h"
#include <stdio.h>
#include <time.h>

int main(void) {
    struct timespec res;

    xtest_eq(clock_getres(CLOCK_REALTIME, &res), 0);
    xtest_eq(res.tv_sec, 0);
    xtest_eq(res.tv_nsec, 1);

    xtest_eq(clock_getres(CLOCK_MONOTONIC, &res), 0);
    xtest_eq(res.tv_sec, 0);
    xtest_eq(res.tv_nsec, 1);

    printf("[vdso_clock_getres] OK\n");
    return 0;
}
