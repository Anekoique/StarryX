
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <sched.h>
#include <unistd.h>
#include <errno.h>
#include <string.h>

// 打印CPU亲和性掩码的辅助函数
void print_cpu_mask(cpu_set_t *mask) {
    printf("CPU affinity mask: ");
    for (int i = 0; i < CPU_SETSIZE; i++) {
        if (CPU_ISSET(i, mask)) {
            printf("%d ", i);
        }
    }
    printf("\n");
}

int main() {
    cpu_set_t original_mask, new_mask;
    pid_t pid = getpid();
    printf("%d\n", pid);
    // 1. 获取当前CPU亲和性
    if (sched_getaffinity(pid, sizeof(original_mask), &original_mask) == -1) {
        perror("sched_getaffinity");
        exit(EXIT_FAILURE);
    }
    printf("Original ");
    print_cpu_mask(&original_mask);

    // 2. 设置新的CPU亲和性（绑定到CPU 0）
    CPU_ZERO(&new_mask);
    CPU_SET(0, &new_mask);
    if (sched_setaffinity(pid, sizeof(new_mask), &new_mask) == -1) {
        perror("sched_setaffinity");
        exit(EXIT_FAILURE);
    }

    // 3. 验证新设置
    cpu_set_t verify_mask;
    if (sched_getaffinity(pid, sizeof(verify_mask), &verify_mask) == -1) {
        perror("sched_getaffinity");
        exit(EXIT_FAILURE);
    }
    printf("New ");
    print_cpu_mask(&verify_mask);

    // 4. 恢复原始设置
    if (sched_setaffinity(pid, sizeof(original_mask), &original_mask) == -1) {
        perror("sched_setaffinity (restore)");
        exit(EXIT_FAILURE);
    }
    printf("Original affinity restored\n");

    return 0;
}
