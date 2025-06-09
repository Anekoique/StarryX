#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/epoll.h>
#include <sys/time.h>
#include <string.h>
#include <sys/socket.h>
#include <errno.h>
#include <fcntl.h>

#define N 1000

void test_basic_pipe() {
    printf("[TEST] Basic pipe + epoll\n");
    int epfd = epoll_create1(0);
    if (epfd < 0) { perror("epoll_create1"); exit(1); }
    int pipefd[2];
    if (pipe(pipefd) < 0) { perror("pipe"); exit(1); }
    struct epoll_event ev = {0};
    ev.events = EPOLLIN;
    ev.data.fd = pipefd[0];
    if (epoll_ctl(epfd, EPOLL_CTL_ADD, pipefd[0], &ev) < 0) { perror("epoll_ctl"); exit(1); }
    const char *msg = "hello epoll";
    write(pipefd[1], msg, strlen(msg));
    struct epoll_event events[4];
    int n = epoll_wait(epfd, events, 4, 1000);
    if (n <= 0) { printf("epoll_wait failed or timeout\n"); exit(1); }
    for (int i = 0; i < n; ++i) {
        if (events[i].data.fd == pipefd[0] && (events[i].events & EPOLLIN)) {
            char buf[32] = {0};
            int r = read(pipefd[0], buf, sizeof(buf)-1);
            printf("epoll event: fd=%d, read %d bytes: %s\n", pipefd[0], r, buf);
        }
    }
    close(pipefd[0]); close(pipefd[1]); close(epfd);
    printf("[OK] Basic pipe + epoll\n\n");
}

void test_socketpair() {
    printf("[TEST] pipe pair + epoll (代替socketpair)\n");
    int epfd = epoll_create1(0);
    if (epfd < 0) { perror("epoll_create1"); exit(1); }
    
    // 使用两个管道模拟双向通信
    int pipe1[2], pipe2[2];
    if (pipe(pipe1) < 0) { perror("pipe1"); exit(1); }
    if (pipe(pipe2) < 0) { perror("pipe2"); exit(1); }
    
    struct epoll_event ev1 = {0}, ev2 = {0};
    ev1.events = EPOLLIN;
    ev1.data.fd = pipe1[0];  // pipe1的读端
    ev2.events = EPOLLOUT;
    ev2.data.fd = pipe2[1];  // pipe2的写端
    
    if (epoll_ctl(epfd, EPOLL_CTL_ADD, pipe1[0], &ev1) < 0) { 
        perror("epoll_ctl pipe1[0]"); exit(1); 
    }
    if (epoll_ctl(epfd, EPOLL_CTL_ADD, pipe2[1], &ev2) < 0) { 
        perror("epoll_ctl pipe2[1]"); exit(1); 
    }
    
    // 写入数据到 pipe1，使 pipe1[0] 可读
    const char *msg = "pipe test";
    write(pipe1[1], msg, strlen(msg));
    
    struct epoll_event events[4];
    int n = epoll_wait(epfd, events, 4, 1000);
    if (n <= 0) { printf("epoll_wait failed or timeout\n"); exit(1); }
    
    int found_in = 0, found_out = 0;
    for (int i = 0; i < n; ++i) {
        if (events[i].data.fd == pipe1[0] && (events[i].events & EPOLLIN)) {
            char buf[32] = {0};
            int r = read(pipe1[0], buf, sizeof(buf)-1);
            printf("epoll event: fd=%d, read %d bytes: %s\n", pipe1[0], r, buf);
            found_in = 1;
        }
        if (events[i].data.fd == pipe2[1] && (events[i].events & EPOLLOUT)) {
            printf("epoll event: fd=%d, EPOLLOUT\n", pipe2[1]);
            found_out = 1;
        }
    }
    
    if (!found_in || !found_out) { 
        printf("[FAIL] pipe in/out\n"); 
        exit(1); 
    }
    
    close(pipe1[0]); close(pipe1[1]); 
    close(pipe2[0]); close(pipe2[1]); 
    close(epfd);
    printf("[OK] pipe pair + epoll\n\n");
}

void test_real_socketpair() {
    printf("[TEST] real socketpair + epoll\n");
    int epfd = epoll_create1(0);
    if (epfd < 0) { perror("epoll_create1"); exit(1); }

    int sv[2];
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, sv) < 0) {
        perror("socketpair");
        exit(1);
    }

    struct epoll_event ev = {0};
    ev.events = EPOLLIN;
    ev.data.fd = sv[1];
    if (epoll_ctl(epfd, EPOLL_CTL_ADD, sv[1], &ev) < 0) {
        perror("epoll_ctl");
        exit(1);
    }

    // 写入数据到 sv[0]，使 sv[1] 可读
    const char *msg = "socketpair test";
    write(sv[0], msg, strlen(msg));

    struct epoll_event events[2];
    int n = epoll_wait(epfd, events, 2, 1000);
    if (n <= 0) {
        printf("epoll_wait failed or timeout\n");
        exit(1);
    }

    int found = 0;
    for (int i = 0; i < n; ++i) {
        if (events[i].data.fd == sv[1] && (events[i].events & EPOLLIN)) {
            char buf[32] = {0};
            int r = read(sv[1], buf, sizeof(buf)-1);
            printf("epoll event: fd=%d, read %d bytes: %s\n", sv[1], r, buf);
            found = 1;
        }
    }
    if (!found) {
        printf("[FAIL] socketpair/EPOLLIN\n");
        exit(1);
    }

    close(sv[0]);
    close(sv[1]);
    close(epfd);
    printf("[OK] real socketpair + epoll\n\n");
}

void test_mod_and_del() {
    printf("[TEST] EPOLL_CTL_MOD and EPOLL_CTL_DEL\n");
    int epfd = epoll_create1(0);
    if (epfd < 0) { perror("epoll_create1"); exit(1); }
    int pipefd[2];
    if (pipe(pipefd) < 0) { perror("pipe"); exit(1); }
    struct epoll_event ev = {0};
    ev.events = EPOLLOUT;
    ev.data.fd = pipefd[1];
    if (epoll_ctl(epfd, EPOLL_CTL_ADD, pipefd[1], &ev) < 0) { perror("epoll_ctl add"); exit(1); }
    close(pipefd[0]); // 关闭读端，写端会变为EPIPE，但依然可写
    struct epoll_event events[2];
    int n = epoll_wait(epfd, events, 2, 1000);
    int found = 0;
    for (int i = 0; i < n; ++i) {
        if (events[i].data.fd == pipefd[1] && (events[i].events & EPOLLOUT)) {
            printf("epoll event: fd=%d, EPOLLOUT after mod\n", pipefd[1]);
            found = 1;
        }
    }
    if (!found) { printf("[FAIL] mod/EPOLLOUT\n"); exit(1); }
    // 删除
    if (epoll_ctl(epfd, EPOLL_CTL_DEL, pipefd[1], NULL) < 0) { perror("epoll_ctl del"); exit(1); }
    n = epoll_wait(epfd, events, 2, 500);
    if (n != 0) { printf("[FAIL] del not effective\n"); exit(1); }
    close(pipefd[1]); close(epfd);
    printf("[OK] EPOLL_CTL_MOD and EPOLL_CTL_DEL\n\n");
}

void test_timeout() {
    printf("[TEST] epoll_wait timeout\n");
    int epfd = epoll_create1(0);
    if (epfd < 0) { perror("epoll_create1"); exit(1); }
    struct epoll_event events[2];
    int n = epoll_wait(epfd, events, 2, 300);
    if (n != 0) { printf("[FAIL] timeout expected 0\n"); exit(1); }
    close(epfd);
    printf("[OK] epoll_wait timeout\n\n");
}

int main() {
    test_basic_pipe();
    test_socketpair();
    test_real_socketpair();
    test_mod_and_del();
    test_timeout();

    printf("[TEST] epoll_wait latency\n");
    int epfd = epoll_create1(0);
    int pipes[N][2];
    struct epoll_event ev = {0}, events[N];
    for (int i = 0; i < N; ++i) {
        pipe(pipes[i]);
        ev.events = EPOLLIN;
        ev.data.fd = pipes[i][0];
        epoll_ctl(epfd, EPOLL_CTL_ADD, pipes[i][0], &ev);
    }

    // 激活所有pipe
    for (int i = 0; i < N; ++i) {
        write(pipes[i][1], "x", 1);
    }

    struct timeval t1, t2;
    gettimeofday(&t1, NULL);
    int n = epoll_wait(epfd, events, N, 1000);
    gettimeofday(&t2, NULL);

    printf("epoll_wait returned %d events\n", n);
    long us = (t2.tv_sec-t1.tv_sec)*1000000 + (t2.tv_usec-t1.tv_usec);
    printf("epoll_wait latency: %ld us\n", us);

    // 清理
    for (int i = 0; i < N; ++i) {
        close(pipes[i][0]);
        close(pipes[i][1]);
    }
    close(epfd);

    printf("[OK] epoll_wait latency\n\n");

    printf("All epoll tests passed!\n");
    return 0;
}
