// vdso_clock_monotonic.c — exercise the vDSO clock_gettime fast path.
//
// Reads CLOCK_MONOTONIC in a tight loop and verifies (1) the sequence is
// non-decreasing (monotonic), (2) total elapsed wall time is in a sane
// range. With the vDSO mapped, `clock_gettime` should serve from the
// shared data page without trapping.

#include "common/assert.h"
#include <stdint.h>
#include <stdio.h>
#include <time.h>

#define ITERATIONS 100000

int main(void) {
    struct timespec start, prev, now;

    xtest_eq(clock_gettime(CLOCK_MONOTONIC, &start), 0);
    prev = start;

    for (int i = 0; i < ITERATIONS; i++) {
        xtest_eq(clock_gettime(CLOCK_MONOTONIC, &now), 0);
        // Monotonic: now must be >= prev.
        if (now.tv_sec < prev.tv_sec ||
            (now.tv_sec == prev.tv_sec && now.tv_nsec < prev.tv_nsec)) {
            fprintf(stderr,
                    "[xtest] iter %d: monotonic violated: prev=%ld.%09ld now=%ld.%09ld\n",
                    i, (long)prev.tv_sec, (long)prev.tv_nsec,
                    (long)now.tv_sec, (long)now.tv_nsec);
            return 1;
        }
        prev = now;
    }

    long long elapsed_ns =
        (long long)(now.tv_sec - start.tv_sec) * 1000000000LL +
        ((long long)now.tv_nsec - (long long)start.tv_nsec);

    // Sanity: any sane execution finishes in well under 60 seconds.
    xtest_assert(elapsed_ns >= 0);
    xtest_assert(elapsed_ns < 60LL * 1000000000LL);

    printf("[vdso_clock_monotonic] %d iters in %lld ns\n",
           ITERATIONS, elapsed_ns);
    return 0;
}
