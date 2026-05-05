#include "common/assert.h"
#include <signal.h>
#include <stdio.h>

static volatile sig_atomic_t fired = 0;

static void handler(int sig) {
    (void)sig;
    fired = 1;
}

int main(void) {
    struct sigaction sa = {0};
    sa.sa_handler = handler;
    sigemptyset(&sa.sa_mask);
    xtest_eq(sigaction(SIGUSR1, &sa, NULL), 0);

    xtest_eq(raise(SIGUSR1), 0);
    xtest_eq(fired, 1);
    return 0;
}
