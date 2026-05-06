// vdso_rt_sigreturn.c — verify the vDSO `__vdso_rt_sigreturn` path works.
//
// Installs a SIGUSR1 handler, raises SIGUSR1, returns from the handler
// (which goes through the vDSO trampoline), and asserts the flag the
// handler set is visible in the caller. If the trampoline is broken,
// the process either crashes or never reaches the post-raise() check.

#include "common/assert.h"
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

static volatile sig_atomic_t handler_ran = 0;

static void handler(int sig) {
    (void)sig;
    handler_ran = 1;
}

int main(void) {
    struct sigaction sa = {0};
    sa.sa_handler = handler;
    sa.sa_flags = SA_RESTART;

    xtest_eq(sigaction(SIGUSR1, &sa, NULL), 0);
    xtest_eq(raise(SIGUSR1), 0);

    // If we reach here, the handler returned cleanly through the vDSO
    // rt_sigreturn trampoline.
    xtest_eq(handler_ran, 1);

    printf("[vdso_rt_sigreturn] OK\n");
    return 0;
}
