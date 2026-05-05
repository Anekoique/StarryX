#include "common/assert.h"
#include <fcntl.h>
#include <string.h>
#include <unistd.h>

int main(void) {
    const char *path = "/tmp/xtest_open_close.tmp";
    unlink(path);

    int fd = open(path, O_CREAT | O_RDWR | O_TRUNC, 0600);
    xtest_assert(fd >= 0);

    const char *msg = "xtest";
    ssize_t n = write(fd, msg, 5);
    xtest_eq(n, 5);

    xtest_eq(lseek(fd, 0, SEEK_SET), 0);
    char buf[8] = {0};
    n = read(fd, buf, sizeof(buf));
    xtest_eq(n, 5);
    xtest_streq(buf, msg);

    xtest_eq(close(fd), 0);
    xtest_eq(unlink(path), 0);
    return 0;
}
