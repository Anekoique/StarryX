#include <stdio.h>
#include <stdlib.h>
#include <errno.h>
#include <unistd.h>
#include <signal.h>
#include <unistd.h>

void wake_me(int seconds, void (*func)(int))
{
	/* set up the signal handler */
	signal(SIGALRM, func);
	/* get the clock running */
	alarm(seconds);
}

unsigned long iter;

void report(int sig)
{
	fprintf(stdout, "COUNT|%lu|1|lps\n", iter);
	exit(0);
}

int main(int argc, char *argv[])
{
	char	buf[512];
	int		pvec[2], duration;

	if (argc != 2) {
		fprintf(stderr,"Usage: %s duration\n", argv[0]);
		exit(1);
		}

	duration = atoi(argv[1]);

	pipe(pvec);

	wake_me(duration, report);
	iter = 0;

	while (1) {
		if (write(pvec[1], buf, sizeof(buf)) != sizeof(buf)) {
			if ((errno != EINTR) && (errno != 0))
				fprintf(stderr,"write failed, error %d\n", errno);
			}
		if (read(pvec[0], buf, sizeof(buf)) != sizeof(buf)) {
			if ((errno != EINTR) && (errno != 0))
				fprintf(stderr,"read failed, error %d\n", errno);
			}
		iter++;
	}
}

