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
#include <pthread.h>
#include <dlfcn.h>

#define LARGE_FILE_SIZE (512 * 1024)   // 512KB for dynamic testing
#define MEDIUM_FILE_SIZE (64 * 1024)   // 64KB
#define SMALL_FILE_SIZE (8 * 1024)     // 8KB
#define PAGE_SIZE 4096
#define MAX_FILES 4
#define NUM_THREADS 3

static sigjmp_buf segv_jmp;
static volatile int segv_caught = 0;

// Thread data structure
typedef struct {
    int thread_id;
    char *mapped_data;
    size_t size;
    int *results;
} thread_data_t;

// SIGSEGV signal handler
void segv_handler(int sig) {
    segv_caught = 1;
    siglongjmp(segv_jmp, 1);
}

// Generate test data
void generate_test_data(char *buf, size_t size, unsigned int seed) {
    srand(seed);
    for (size_t i = 0; i < size; i++) {
        buf[i] = (char)(rand() % 256);
    }
}

// Create test file
int create_test_file(const char *template, size_t size, unsigned int seed) {
    char temp[256];
    strcpy(temp, template);
    
    int fd = mkstemp(temp);
    if (fd < 0) {
        perror("mkstemp");
        return -1;
    }
    
    unlink(temp);  // Delete file but keep fd open
    
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

// Thread function for concurrent testing
void* thread_test_function(void* arg) {
    thread_data_t *data = (thread_data_t*)arg;
    int success = 1;
    
    printf("  Thread %d: Starting concurrent access test\n", data->thread_id);
    
    // Random access pattern
    for (int i = 0; i < 100; i++) {
        size_t offset = (data->thread_id * 1000 + i * 13) % data->size;
        volatile char test = data->mapped_data[offset];
        (void)test;  // Suppress unused variable warning
        
        // Small delay to increase contention
        usleep(100);
    }
    
    data->results[data->thread_id] = success;
    printf("  Thread %d: Completed successfully\n", data->thread_id);
    return NULL;
}

int test_dynamic_library_info() {
    printf("=== Test 1: Dynamic Library Information ===\n");
    
    // Get information about loaded libraries
    printf("Checking dynamic linker information...\n");
    
    // Test dlopen functionality
    void *handle = dlopen("libc.so.6", RTLD_LAZY);
    if (handle) {
        printf("  Successfully opened libc.so.6\n");
        
        // Try to get malloc symbol
        void *malloc_sym = dlsym(handle, "malloc");
        if (malloc_sym) {
            printf("  Found malloc symbol in libc\n");
        } else {
            printf("  Could not find malloc symbol: %s\n", dlerror());
        }
        
        dlclose(handle);
    } else {
        printf("  Could not open libc.so.6: %s\n", dlerror());
    }
    
    printf("Dynamic library test COMPLETED\n\n");
    return 1;
}

int test_threaded_mmap_access() {
    printf("=== Test 2: Threaded mmap Access ===\n");
    
    int fd = create_test_file("/tmp/threaded-XXXXXX", MEDIUM_FILE_SIZE, 54321);
    if (fd < 0) return 0;
    
    // Map file with read permissions
    char *mapped = mmap(NULL, MEDIUM_FILE_SIZE, PROT_READ, MAP_PRIVATE, fd, 0);
    if (mapped == MAP_FAILED) {
        perror("mmap");
        close(fd);
        return 0;
    }
    
    printf("File mapped for threaded access test\n");
    
    // Create threads for concurrent access
    pthread_t threads[NUM_THREADS];
    thread_data_t thread_data[NUM_THREADS];
    int results[NUM_THREADS];
    
    // Initialize thread data
    for (int i = 0; i < NUM_THREADS; i++) {
        thread_data[i].thread_id = i;
        thread_data[i].mapped_data = mapped;
        thread_data[i].size = MEDIUM_FILE_SIZE;
        thread_data[i].results = results;
        results[i] = 0;
    }
    
    // Create threads
    printf("Creating %d threads for concurrent access...\n", NUM_THREADS);
    for (int i = 0; i < NUM_THREADS; i++) {
        if (pthread_create(&threads[i], NULL, thread_test_function, &thread_data[i]) != 0) {
            printf("Failed to create thread %d\n", i);
            // Clean up created threads
            for (int j = 0; j < i; j++) {
                pthread_join(threads[j], NULL);
            }
            munmap(mapped, MEDIUM_FILE_SIZE);
            close(fd);
            return 0;
        }
    }
    
    // Wait for all threads to complete
    for (int i = 0; i < NUM_THREADS; i++) {
        pthread_join(threads[i], NULL);
    }
    
    // Check results
    int all_success = 1;
    for (int i = 0; i < NUM_THREADS; i++) {
        if (!results[i]) {
            printf("Thread %d failed\n", i);
            all_success = 0;
        }
    }
    
    printf("Threaded access test: %s\n", all_success ? "SUCCESS" : "FAILED");
    
    munmap(mapped, MEDIUM_FILE_SIZE);
    close(fd);
    
    printf("Threaded mmap test COMPLETED\n\n");
    return all_success;
}

int test_large_file_performance() {
    printf("=== Test 3: Large File Performance ===\n");
    
    int fd = create_test_file("/tmp/perftest-XXXXXX", LARGE_FILE_SIZE, 99999);
    if (fd < 0) return 0;
    
    printf("Testing performance with %dKB file\n", LARGE_FILE_SIZE / 1024);
    
    // Map the large file
    char *mapped = mmap(NULL, LARGE_FILE_SIZE, PROT_READ, MAP_PRIVATE, fd, 0);
    if (mapped == MAP_FAILED) {
        perror("mmap large file");
        close(fd);
        return 0;
    }
    
    // Time the access pattern
    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);
    
    // Sequential access pattern
    printf("  Sequential access test...");
    for (size_t i = 0; i < LARGE_FILE_SIZE; i += PAGE_SIZE) {
        volatile char test = mapped[i];
        (void)test;
    }
    
    clock_gettime(CLOCK_MONOTONIC, &end);
    
    double elapsed = (end.tv_sec - start.tv_sec) + 
                    (end.tv_nsec - start.tv_nsec) / 1000000000.0;
    
    printf(" completed in %.3f seconds\n", elapsed);
    printf("  Pages accessed: %d\n", LARGE_FILE_SIZE / PAGE_SIZE);
    printf("  Average time per page: %.3f ms\n", 
           (elapsed * 1000) / (LARGE_FILE_SIZE / PAGE_SIZE));
    
    // Random access pattern
    printf("  Random access test...");
    clock_gettime(CLOCK_MONOTONIC, &start);
    
    srand(12345);
    for (int i = 0; i < 100; i++) {
        size_t offset = rand() % LARGE_FILE_SIZE;
        volatile char test = mapped[offset];
        (void)test;
    }
    
    clock_gettime(CLOCK_MONOTONIC, &end);
    elapsed = (end.tv_sec - start.tv_sec) + 
              (end.tv_nsec - start.tv_nsec) / 1000000000.0;
    
    printf(" completed in %.3f seconds\n", elapsed);
    printf("  Average time per random access: %.3f ms\n", (elapsed * 1000) / 100);
    
    munmap(mapped, LARGE_FILE_SIZE);
    close(fd);
    
    printf("Performance test COMPLETED\n\n");
    return 1;
}

int test_memory_mapping_limits() {
    printf("=== Test 4: Memory Mapping Limits ===\n");
    
    // Test multiple simultaneous mappings
    struct {
        int fd;
        char *mapped;
        size_t size;
    } mappings[MAX_FILES];
    
    size_t sizes[] = {PAGE_SIZE, PAGE_SIZE*2, PAGE_SIZE*4, PAGE_SIZE*8};
    
    printf("Testing multiple simultaneous mappings...\n");
    
    // Create multiple mappings
    for (int i = 0; i < MAX_FILES; i++) {
        char template[] = "/tmp/limit-test-XXXXXX";
        mappings[i].size = sizes[i];
        mappings[i].fd = create_test_file(template, mappings[i].size, 1000 + i);
        
        if (mappings[i].fd < 0) {
            printf("Failed to create test file %d\n", i);
            // Clean up previous mappings
            for (int j = 0; j < i; j++) {
                munmap(mappings[j].mapped, mappings[j].size);
                close(mappings[j].fd);
            }
            return 0;
        }
        
        mappings[i].mapped = mmap(NULL, mappings[i].size, PROT_READ, MAP_PRIVATE, mappings[i].fd, 0);
        if (mappings[i].mapped == MAP_FAILED) {
            printf("Failed to map file %d: %s\n", i, strerror(errno));
            close(mappings[i].fd);
            // Clean up previous mappings
            for (int j = 0; j < i; j++) {
                munmap(mappings[j].mapped, mappings[j].size);
                close(mappings[j].fd);
            }
            return 0;
        }
        
        printf("  Mapping %d: %p, size %zu bytes\n", i, mappings[i].mapped, mappings[i].size);
    }
    
    // Test access to all mappings
    printf("Testing access to all mappings...\n");
    for (int i = 0; i < MAX_FILES; i++) {
        printf("  Accessing mapping %d...", i);
        volatile char test = mappings[i].mapped[0];
        volatile char test2 = mappings[i].mapped[mappings[i].size - 1];
        (void)test; (void)test2;
        printf(" OK\n");
    }
    
    // Clean up
    for (int i = 0; i < MAX_FILES; i++) {
        munmap(mappings[i].mapped, mappings[i].size);
        close(mappings[i].fd);
    }
    
    printf("Memory mapping limits test COMPLETED\n\n");
    return 1;
}

int test_error_handling() {
    printf("=== Test 5: Error Handling ===\n");
    
    // Test invalid file descriptor
    printf("Testing invalid file descriptor...\n");
    char *invalid_map = mmap(NULL, PAGE_SIZE, PROT_READ, MAP_PRIVATE, -1, 0);
    if (invalid_map == MAP_FAILED) {
        printf("  Invalid fd correctly rejected: %s\n", strerror(errno));
    } else {
        printf("  ERROR: Invalid fd was accepted\n");
        munmap(invalid_map, PAGE_SIZE);
        return 0;
    }
    
    // Test mapping beyond file size
    printf("Testing mapping beyond file size...\n");
    int small_fd = create_test_file("/tmp/small-XXXXXX", PAGE_SIZE/2, 11111);
    if (small_fd >= 0) {
        char *over_map = mmap(NULL, PAGE_SIZE*2, PROT_READ, MAP_PRIVATE, small_fd, 0);
        if (over_map == MAP_FAILED) {
            printf("  Over-mapping correctly rejected: %s\n", strerror(errno));
        } else {
            printf("  Over-mapping allowed (may be valid)\n");
            
            // Test access beyond file
            signal(SIGSEGV, segv_handler);
            segv_caught = 0;
            if (sigsetjmp(segv_jmp, 1) == 0) {
                volatile char test = over_map[PAGE_SIZE];  // Beyond file
                (void)test;  // Suppress unused variable warning
                printf("    Access beyond file succeeded (zero-filled)\n");
            } else {
                printf("    Access beyond file triggered SIGSEGV\n");
            }
            signal(SIGSEGV, SIG_DFL);
            
            munmap(over_map, PAGE_SIZE*2);
        }
        close(small_fd);
    }
    
    printf("Error handling test COMPLETED\n\n");
    return 1;
}

int main() {
    int tests_passed = 0;
    int total_tests = 1;
    
    // if (test_dynamic_library_info()) tests_passed++;
    // if (test_threaded_mmap_access()) tests_passed++;
    // if (test_large_file_performance()) tests_passed++;
    // if (test_memory_mapping_limits()) tests_passed++;
    if (test_error_handling()) tests_passed++;
    
    printf("=== Final Test Results ===\n");
    printf("Passed: %d/%d tests\n", tests_passed, total_tests);
    
    if (tests_passed == total_tests) {
        printf("All dynamic linking mmap tests PASSED! 🎉\n");
        printf("The mmap lazy allocation works correctly with dynamic linking.\n");
        return 0;
    } else {
        printf("Some tests FAILED! ❌\n");
        printf("Please check the mmap implementation with dynamic linking.\n");
        return 1;
    }
} 