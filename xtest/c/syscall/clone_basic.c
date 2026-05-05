#include "common/assert.h"
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    pid_t parent = getpid();
    pid_t pid = fork();
    xtest_assert(pid >= 0);

    if (pid == 0) {
        // child
        xtest_neq(getpid(), parent);
        xtest_eq(getppid(), parent);
        _exit(42);
    }

    int status = 0;
    pid_t reaped = waitpid(pid, &status, 0);
    xtest_eq(reaped, pid);
    xtest_assert(WIFEXITED(status));
    xtest_eq(WEXITSTATUS(status), 42);
    return 0;
}
