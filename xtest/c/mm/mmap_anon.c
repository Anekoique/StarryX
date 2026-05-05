#include "common/assert.h"
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

int main(void) {
    long page = sysconf(_SC_PAGESIZE);
    xtest_assert(page > 0);

    size_t len = (size_t)page * 4;
    void *p = mmap(NULL, len, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    xtest_neq((long long)(intptr_t)p, (long long)(intptr_t)MAP_FAILED);

    memset(p, 0xa5, len);
    unsigned char *bytes = (unsigned char *)p;
    xtest_eq(bytes[0], 0xa5);
    xtest_eq(bytes[len - 1], 0xa5);

    xtest_eq(munmap(p, len), 0);
    return 0;
}
