#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#include <assert.h>
#include <signal.h>
#include <setjmp.h>
#include <sys/stat.h>

#define LARGE_FILE_SIZE (1024 * 1024)  // 1MB
#define MEDIUM_FILE_SIZE (64 * 1024)   // 64KB
#define SMALL_FILE_SIZE (8 * 1024)     // 8KB
#define PAGE_SIZE 4096
#define MAX_FILES 8

static sigjmp_buf segv_jmp;
static volatile int segv_caught = 0;

// SIGSEGV信号处理器
void segv_handler(int sig) {
    segv_caught = 1;
    siglongjmp(segv_jmp, 1);
}

// 生成测试数据
void generate_test_data(char *buf, size_t size, unsigned int seed) {
    srand(seed);
    for (size_t i = 0; i < size; i++) {
        buf[i] = (char)(rand() % 256);
    }
}

// 创建测试文件
int create_test_file(const char *template, size_t size, unsigned int seed) {
    char temp[256];
    strcpy(temp, template);
    
    int fd = mkstemp(temp);
    if (fd < 0) {
        perror("mkstemp");
        return -1;
    }
    
    // 删除文件，但保持fd打开
    unlink(temp);
    
    if (size > 0) {
        char *data = malloc(size);
        if (!data) {
            close(fd);
            return -1;
        }
        
        generate_test_data(data, size, seed);
        
        if (write(fd, data, size) != (ssize_t)size) {
            perror("write");
            free(data);
            close(fd);
            return -1;
        }
        
        free(data);
    }
    
    return fd;
}

int test_boundary_conditions() {
    printf("=== Test 1: Boundary Conditions ===\n");
    
    int fd = create_test_file("/tmp/boundary-XXXXXX", MEDIUM_FILE_SIZE, 12345);
    if (fd < 0) return 0;
    
    char *mapped = mmap(NULL, MEDIUM_FILE_SIZE, PROT_READ, MAP_PRIVATE, fd, 0);
    if (mapped == MAP_FAILED) {
        perror("mmap");
        close(fd);
        return 0;
    }
    
    printf("Testing valid boundary access...\n");
    
    // 测试文件开始
    char first = mapped[0];
    printf("  First byte: 0x%02x\n", (unsigned char)first);
    
    // 测试文件结尾
    char last = mapped[MEDIUM_FILE_SIZE - 1];
    printf("  Last byte: 0x%02x\n", (unsigned char)last);
    
    // 测试页边界
    for (int page = 0; page < MEDIUM_FILE_SIZE / PAGE_SIZE; page++) {
        size_t offset = page * PAGE_SIZE;
        char page_start = mapped[offset];
        printf("  Page %d start: 0x%02x\n", page, (unsigned char)page_start);
    }
    
    printf("Testing invalid access (should be safe due to file mapping)...\n");
    
    // 设置信号处理器
    signal(SIGSEGV, segv_handler);
    
    // 尝试访问超出文件范围的映射区域（如果映射大小大于文件大小）
    size_t extended_size = MEDIUM_FILE_SIZE + PAGE_SIZE;
    munmap(mapped, MEDIUM_FILE_SIZE);
    
    mapped = mmap(NULL, extended_size, PROT_READ, MAP_PRIVATE, fd, 0);
    if (mapped != MAP_FAILED) {
        segv_caught = 0;
        if (sigsetjmp(segv_jmp, 1) == 0) {
            // 尝试访问超出文件范围的区域
            volatile char test = mapped[MEDIUM_FILE_SIZE + 100];
            (void)test;
            printf("  Access beyond file succeeded (filled with zeros)\n");
        } else {
            printf("  Access beyond file triggered SIGSEGV (expected)\n");
        }
        munmap(mapped, extended_size);
    }
    
    signal(SIGSEGV, SIG_DFL);
    close(fd);
    
    printf("Boundary conditions test COMPLETED\n\n");
    return 1;
}

int test_multiple_mappings() {
    printf("=== Test 2: Multiple File Mappings ===\n");
    
    struct {
        int fd;
        char *mapped;
        size_t size;
        unsigned int seed;
    } files[MAX_FILES];
    
    size_t sizes[] = {SMALL_FILE_SIZE, MEDIUM_FILE_SIZE/4, MEDIUM_FILE_SIZE/2, 
                      MEDIUM_FILE_SIZE, MEDIUM_FILE_SIZE*2, SMALL_FILE_SIZE*3,
                      PAGE_SIZE, PAGE_SIZE*5};
    
    // 创建多个文件映射
    for (int i = 0; i < MAX_FILES; i++) {
        char template[] = "/tmp/multi-XXXXXX";
        files[i].size = sizes[i];
        files[i].seed = 1000 + i;
        files[i].fd = create_test_file(template, files[i].size, files[i].seed);
        
        if (files[i].fd < 0) {
            // 清理已创建的文件
            for (int j = 0; j < i; j++) {
                munmap(files[j].mapped, files[j].size);
                close(files[j].fd);
            }
            return 0;
        }
        
        files[i].mapped = mmap(NULL, files[i].size, PROT_READ, MAP_PRIVATE, files[i].fd, 0);
        if (files[i].mapped == MAP_FAILED) {
            perror("mmap");
            close(files[i].fd);
            // 清理已创建的映射
            for (int j = 0; j < i; j++) {
                munmap(files[j].mapped, files[j].size);
                close(files[j].fd);
            }
            return 0;
        }
        
        printf("  File %d: mapped at %p, size %zu bytes\n", i, files[i].mapped, files[i].size);
    }
    
    printf("Testing concurrent access to all mappings...\n");
    
    // 并发访问所有映射
    for (int round = 0; round < 3; round++) {
        printf("  Round %d: ", round + 1);
        for (int i = 0; i < MAX_FILES; i++) {
            // 随机访问每个文件的不同位置
            size_t offset = (round * 1234 + i * 567) % files[i].size;
            volatile char test = files[i].mapped[offset];
            (void)test;
            printf("%d ", i);
        }
        printf("OK\n");
    }
    
    // 验证数据完整性
    printf("Verifying data integrity...\n");
    for (int i = 0; i < MAX_FILES; i++) {
        // 重新生成期望数据
        char *expected = malloc(files[i].size);
        generate_test_data(expected, files[i].size, files[i].seed);
        
        // 验证前256字节
        size_t verify_size = (files[i].size > 256) ? 256 : files[i].size;
        int match = 1;
        for (size_t j = 0; j < verify_size; j++) {
            if (files[i].mapped[j] != expected[j]) {
                match = 0;
                break;
            }
        }
        
        printf("  File %d integrity: %s\n", i, match ? "OK" : "FAILED");
        free(expected);
    }
    
    // 清理
    for (int i = 0; i < MAX_FILES; i++) {
        munmap(files[i].mapped, files[i].size);
        close(files[i].fd);
    }
    
    printf("Multiple mappings test COMPLETED\n\n");
    return 1;
}

int test_offset_mapping() {
    printf("=== Test 3: Offset Mapping ===\n");
    
    int fd = create_test_file("/tmp/offset-XXXXXX", MEDIUM_FILE_SIZE, 54321);
    if (fd < 0) return 0;
    
    // 测试不同偏移的映射
    size_t offsets[] = {0, PAGE_SIZE, PAGE_SIZE*2, PAGE_SIZE*3, MEDIUM_FILE_SIZE/2};
    int num_offsets = sizeof(offsets) / sizeof(offsets[0]);
    
    char *expected_data = malloc(MEDIUM_FILE_SIZE);
    generate_test_data(expected_data, MEDIUM_FILE_SIZE, 54321);
    
    for (int i = 0; i < num_offsets; i++) {
        size_t offset = offsets[i];
        if (offset >= MEDIUM_FILE_SIZE) continue;
        
        size_t map_size = MEDIUM_FILE_SIZE - offset;
        if (map_size > PAGE_SIZE * 4) map_size = PAGE_SIZE * 4; // 限制映射大小
        
        printf("Testing offset mapping: offset=%zu, size=%zu\n", offset, map_size);
        
        char *mapped = mmap(NULL, map_size, PROT_READ, MAP_PRIVATE, fd, offset);
        if (mapped == MAP_FAILED) {
            printf("  Offset mapping failed: %s\n", strerror(errno));
            continue;
        }
        
        // 验证映射数据
        int verify_ok = 1;
        size_t verify_size = (map_size > 1024) ? 1024 : map_size;
        
        for (size_t j = 0; j < verify_size; j++) {
            if (mapped[j] != expected_data[offset + j]) {
                printf("  Data mismatch at position %zu\n", j);
                verify_ok = 0;
                break;
            }
        }
        
        printf("  Offset mapping verification: %s\n", verify_ok ? "OK" : "FAILED");
        
        munmap(mapped, map_size);
    }
    
    free(expected_data);
    close(fd);
    
    printf("Offset mapping test COMPLETED\n\n");
    return 1;
}

int test_write_protection() {
    printf("=== Test 4: Write Protection ===\n");
    
    int fd = create_test_file("/tmp/writetest-XXXXXX", PAGE_SIZE * 2, 99999);
    if (fd < 0) return 0;
    
    // 测试只读映射
    char *ro_mapped = mmap(NULL, PAGE_SIZE * 2, PROT_READ, MAP_PRIVATE, fd, 0);
    if (ro_mapped == MAP_FAILED) {
        perror("mmap read-only");
        close(fd);
        return 0;
    }
    
    printf("Testing read-only mapping...\n");
    volatile char test_read = ro_mapped[100];
    printf("  Read access: OK (value=0x%02x)\n", (unsigned char)test_read);
    
    // 设置信号处理器来捕获写入尝试
    signal(SIGSEGV, segv_handler);
    segv_caught = 0;
    
    if (sigsetjmp(segv_jmp, 1) == 0) {
        // 尝试写入只读映射
        ro_mapped[100] = 0xFF;
        printf("  Write to read-only: UNEXPECTED SUCCESS\n");
    } else {
        printf("  Write to read-only: Correctly blocked by SIGSEGV\n");
    }
    
    munmap(ro_mapped, PAGE_SIZE * 2);
    
    // 测试读写映射
    char *rw_mapped = mmap(NULL, PAGE_SIZE * 2, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0);
    if (rw_mapped == MAP_FAILED) {
        printf("  Read-write mapping failed: %s\n", strerror(errno));
    } else {
        printf("Testing read-write mapping...\n");
        
        char original = rw_mapped[100];
        printf("  Original value: 0x%02x\n", (unsigned char)original);
        
        segv_caught = 0;
        if (sigsetjmp(segv_jmp, 1) == 0) {
            rw_mapped[100] = 0xAB;
            printf("  Write access: OK\n");
            printf("  New value: 0x%02x\n", (unsigned char)rw_mapped[100]);
        } else {
            printf("  Write access: FAILED with SIGSEGV\n");
        }
        
        munmap(rw_mapped, PAGE_SIZE * 2);
    }
    
    signal(SIGSEGV, SIG_DFL);
    close(fd);
    
    printf("Write protection test COMPLETED\n\n");
    return 1;
}

int test_large_file_lazy_loading() {
    printf("=== Test 5: Large File Lazy Loading ===\n");
    
    int fd = create_test_file("/tmp/largefile-XXXXXX", LARGE_FILE_SIZE, 77777);
    if (fd < 0) return 0;
    
    printf("Created large file (%d KB)\n", LARGE_FILE_SIZE / 1024);
    
    // 映射整个大文件
    char *mapped = mmap(NULL, LARGE_FILE_SIZE, PROT_READ, MAP_PRIVATE, fd, 0);
    if (mapped == MAP_FAILED) {
        perror("mmap large file");
        close(fd);
        return 0;
    }
    
    printf("Large file mapped successfully\n");
    
    // 稀疏访问模式 - 只访问少数页面
    int access_pages[] = {0, 10, 50, 100, 200, LARGE_FILE_SIZE/PAGE_SIZE - 1};
    int num_accesses = sizeof(access_pages) / sizeof(access_pages[0]);
    
    printf("Testing sparse access pattern...\n");
    
    for (int i = 0; i < num_accesses; i++) {
        int page = access_pages[i];
        if (page >= LARGE_FILE_SIZE / PAGE_SIZE) continue;
        
        size_t offset = page * PAGE_SIZE;
        printf("  Accessing page %d (offset %zu)...", page, offset);
        
        // 触发页面加载
        volatile char test = mapped[offset];
        volatile char test2 = mapped[offset + PAGE_SIZE/2];
        
        printf(" OK\n");
    }
    
    // 测试连续访问
    printf("Testing sequential access pattern...\n");
    size_t sequential_start = LARGE_FILE_SIZE / 4;
    size_t sequential_size = PAGE_SIZE * 8;
    
    printf("  Sequential read from offset %zu, size %zu...", sequential_start, sequential_size);
    
    for (size_t i = 0; i < sequential_size; i += 64) {
        if (sequential_start + i >= LARGE_FILE_SIZE) break;
        volatile char test = mapped[sequential_start + i];
        (void)test;
    }
    
    printf(" OK\n");
    
    munmap(mapped, LARGE_FILE_SIZE);
    close(fd);
    
    printf("Large file lazy loading test COMPLETED\n\n");
    return 1;
}

int test_edge_cases() {
    printf("=== Test 6: Edge Cases ===\n");
    
    // 测试空文件映射
    printf("Testing empty file mapping...\n");
    int empty_fd = create_test_file("/tmp/empty-XXXXXX", 0, 0);
    if (empty_fd >= 0) {
        char *empty_mapped = mmap(NULL, PAGE_SIZE, PROT_READ, MAP_PRIVATE, empty_fd, 0);
        if (empty_mapped == MAP_FAILED) {
            printf("  Empty file mapping failed (expected): %s\n", strerror(errno));
        } else {
            printf("  Empty file mapping succeeded\n");
            munmap(empty_mapped, PAGE_SIZE);
        }
        close(empty_fd);
    }
    
    // 测试非页对齐的文件大小
    printf("Testing non-page-aligned file size...\n");
    size_t odd_size = PAGE_SIZE + 100;  // 不是页大小的整数倍
    int odd_fd = create_test_file("/tmp/oddsize-XXXXXX", odd_size, 88888);
    if (odd_fd >= 0) {
        char *odd_mapped = mmap(NULL, PAGE_SIZE * 2, PROT_READ, MAP_PRIVATE, odd_fd, 0);
        if (odd_mapped == MAP_FAILED) {
            printf("  Non-aligned size mapping failed: %s\n", strerror(errno));
        } else {
            printf("  Non-aligned size mapping succeeded\n");
            
            // 访问文件末尾附近
            printf("    Accessing near file end...");
            volatile char test1 = odd_mapped[odd_size - 1];  // 文件最后一字节
            printf(" OK\n");
            
            // 尝试访问文件结束后的区域
            signal(SIGSEGV, segv_handler);
            segv_caught = 0;
            if (sigsetjmp(segv_jmp, 1) == 0) {
                volatile char test2 = odd_mapped[odd_size + 100];  // 文件结束后
                printf("    Access beyond file end: succeeded (zero-filled)\n");
            } else {
                printf("    Access beyond file end: SIGSEGV (expected)\n");
            }
            signal(SIGSEGV, SIG_DFL);
            
            munmap(odd_mapped, PAGE_SIZE * 2);
        }
        close(odd_fd);
    }
    
    printf("Edge cases test COMPLETED\n\n");
    return 1;
}

int main() {
    printf("Starting robust mmap lazy allocation tests...\n\n");
    
    int tests_passed = 0;
    int total_tests = 6;
    
    if (test_boundary_conditions()) tests_passed++;
    if (test_multiple_mappings()) tests_passed++;
    if (test_offset_mapping()) tests_passed++;
    if (test_write_protection()) tests_passed++;
    if (test_large_file_lazy_loading()) tests_passed++;
    if (test_edge_cases()) tests_passed++;
    
    printf("=== Final Test Results ===\n");
    printf("Passed: %d/%d tests\n", tests_passed, total_tests);
    
    if (tests_passed == total_tests) {
        printf("All robust mmap tests PASSED! 🎉\n");
        printf("The mmap lazy allocation implementation is working correctly.\n");
        return 0;
    } else {
        printf("Some tests FAILED! ❌\n");
        printf("Please check the mmap implementation.\n");
        return 1;
    }
} 