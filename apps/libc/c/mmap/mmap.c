#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>

#define FILE_SIZE (100 * 1024 * 1024) // 100MB
#define PAGE_SIZE (4096)              // 假设页大小4KB
#define TOUCH_PAGES 100               // 仅访问100个页面
#define TOUCH_SIZE (TOUCH_PAGES * PAGE_SIZE)

// 获取进程的RSS内存占用（KB）
unsigned long get_vmrss() {
    FILE *f = fopen("/proc/self/statm", "r");
    if (!f) return 0;
    
    unsigned long size, rss;
    fscanf(f, "%lu %lu", &size, &rss);
    fclose(f);
    return (rss * sysconf(_SC_PAGESIZE)) / 1024;
}

int main() {
    int fd;
    char temp[] = "/tmp/mmaptest-XXXXXX";
    char *map;
    
    // 创建稀疏临时文件
    if ((fd = mkstemp(temp)) < 0) {
        perror("mkstemp");
        return 1;
    }
    unlink(temp);  // 自动删除
    
    // 快速创建稀疏文件（仅设置文件大小）
    if (ftruncate(fd, FILE_SIZE) < 0) {
        perror("ftruncate");
        close(fd);
        return 1;
    }

    // 映射整个文件（100MB）
    map = mmap(NULL, FILE_SIZE, PROT_READ, MAP_PRIVATE, fd, 0);
    if (map == MAP_FAILED) {
        perror("mmap");
        close(fd);
        return 1;
    }
    close(fd);

    // 初始内存占用
    unsigned long rss1 = get_vmrss();
    printf("After mmap (lazy):\n");
    printf("  VmRSS: %lu KB\n", rss1);
    printf("  Expected: Small value (only metadata)\n\n");

    // 访问特定区域（触发缺页）
    printf("Accessing %d pages (%d KB)...\n", TOUCH_PAGES, TOUCH_SIZE/1024);
    for (int i = 0; i < TOUCH_SIZE; i += PAGE_SIZE) {
        // 从映射中间访问，避免边界效应
        char c = map[FILE_SIZE/2 + i];
        (void)c; // 抑制编译器警告
    }

    // 访问后内存占用
    unsigned long rss2 = get_vmrss();
    printf("After accessing pages:\n");
    printf("  VmRSS: %lu KB\n", rss2);
    printf("  RSS increase: %lu KB\n", rss2 - rss1);
    printf("  Expected increase: ~%d KB\n\n", TOUCH_SIZE/1024);

    // 验证内存释放
    munmap(map, FILE_SIZE);
    unsigned long rss3 = get_vmrss();
    printf("After munmap:\n");
    printf("  VmRSS: %lu KB (freed: %lu KB)\n", rss3, rss2 - rss3);

    return 0;
}
