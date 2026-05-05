#ifndef XTEST_ASSERT_H
#define XTEST_ASSERT_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Tiny assert helpers for first-party xtest C tests.
//
// Tests are single-file programs that return 0 on success and non-zero on
// failure. xtest_assert / xtest_eq / xtest_neq print a descriptive line and
// exit(1) on failure so run-c.sh reports the file/line that broke.

#define XTEST_FAIL(fmt, ...) do {                                           \
    fprintf(stderr, "[xtest] %s:%d: " fmt "\n",                             \
            __FILE__, __LINE__, ##__VA_ARGS__);                             \
    exit(1);                                                                \
} while (0)

#define xtest_assert(cond) do {                                             \
    if (!(cond)) {                                                          \
        XTEST_FAIL("assertion failed: %s", #cond);                          \
    }                                                                       \
} while (0)

#define xtest_eq(a, b) do {                                                 \
    long long _a = (long long)(a);                                          \
    long long _b = (long long)(b);                                          \
    if (_a != _b) {                                                         \
        XTEST_FAIL("expected %s == %s, got %lld vs %lld",                   \
                   #a, #b, _a, _b);                                         \
    }                                                                       \
} while (0)

#define xtest_neq(a, b) do {                                                \
    long long _a = (long long)(a);                                          \
    long long _b = (long long)(b);                                          \
    if (_a == _b) {                                                         \
        XTEST_FAIL("expected %s != %s, got %lld",                           \
                   #a, #b, _a);                                             \
    }                                                                       \
} while (0)

#define xtest_streq(a, b) do {                                              \
    const char *_a = (a);                                                   \
    const char *_b = (b);                                                   \
    if (strcmp(_a, _b) != 0) {                                              \
        XTEST_FAIL("expected %s == %s, got \"%s\" vs \"%s\"",               \
                   #a, #b, _a, _b);                                         \
    }                                                                       \
} while (0)

#endif
