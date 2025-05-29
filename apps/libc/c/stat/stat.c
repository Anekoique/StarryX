#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <stdio.h>

int main() {
    int invalid_dirfd = -123; // Invalid file descriptor
    int fd = openat(invalid_dirfd, "/dev/random", O_RDONLY);
    if (fd == -1) {
        perror("openat failed");
        return 1;
    }
    printf("Successfully opened /dev/random: fd = %d\n", fd);
    close(fd);
    return 0;
}
