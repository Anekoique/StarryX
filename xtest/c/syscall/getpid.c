#include "common/assert.h"
#include <unistd.h>

int main(void) {
    pid_t a = getpid();
    pid_t b = getpid();
    xtest_assert(a > 0);
    xtest_eq(a, b);
    return 0;
}
