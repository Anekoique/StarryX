// vdso_gettimeofday.c — verify gettimeofday agrees with CLOCK_REALTIME.
//
// Both routes (vDSO gettimeofday + vDSO clock_gettime(CLOCK_REALTIME))
// read from the same kernel data page; they must agree to within the
// time taken to issue both calls.

#include "common/assert.h"
#include <stdio.h>
#include <sys/time.h>
#include <time.h>

int main(void) {
    struct timeval tv;
    struct timespec ts;

    xtest_eq(gettimeofday(&tv, NULL), 0);
    xtest_eq(clock_gettime(CLOCK_REALTIME, &ts), 0);

    long long tv_us = (long long)tv.tv_sec * 1000000 + tv.tv_usec;
    long long ts_us = (long long)ts.tv_sec * 1000000 + ts.tv_nsec / 1000;

    long long delta = ts_us - tv_us;
    if (delta < 0) {
        delta = -delta;
    }
    // gettimeofday and clock_gettime called back-to-back should agree to
    // within 100 ms even on a heavily-loaded system.
    xtest_assert(delta < 100000);

    printf("[vdso_gettimeofday] tv=%lld.%06ld ts=%lld.%09ld delta=%lld us\n",
           (long long)tv.tv_sec, (long)tv.tv_usec,
           (long long)ts.tv_sec, (long)ts.tv_nsec, delta);
    return 0;
}
