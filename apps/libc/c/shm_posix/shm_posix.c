#include <stdio.h>    // For printf, perror
#include <stdlib.h>   // For exit, atoi
#include <fcntl.h>    // For O_CREAT, O_RDWR
#include <sys/mman.h> // For shm_open, mmap, munmap, shm_unlink
#include <sys/stat.h> // For mode constants (S_IRUSR, S_IWUSR)
#include <unistd.h>   // For ftruncate, fork, sleep
#include <string.h>   // For strlen, strcpy, strcmp
#include <sys/wait.h>

#define SHM_NAME "/my_posix_shm" // 共享内存对象的名称
#define SHM_SIZE 4096            // 共享内存区域的大小（字节）
#define MESSAGE "Hello, POSIX Shared Memory!" // 要写入共享内存的消息

int main() {
    int shm_fd;         // 共享内存文件描述符
    void *shm_ptr;      // 指向共享内存区域的指针
    pid_t pid;          // 进程ID

    printf("--- POSIX Shared Memory Test ---\n");

    // 1. 创建或打开共享内存对象
    // shm_open(name, oflag, mode)
    // O_CREAT: 如果不存在则创建
    // O_RDWR: 读写模式
    // S_IRUSR | S_IWUSR: 用户读写权限 (0600)
    shm_fd = shm_open(SHM_NAME, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR);
    if (shm_fd == -1) {
        perror("shm_open failed");
        exit(EXIT_FAILURE);
    }
    printf("Shared memory object '%s' opened/created with fd: %d\n", SHM_NAME, shm_fd);

    // 2. 设置共享内存对象的大小
    // ftruncate(fd, length): 将文件描述符fd对应的文件大小截断为length字节
    if (ftruncate(shm_fd, SHM_SIZE) == -1) {
        perror("ftruncate failed");
        // 如果失败，尝试清理已创建的共享内存对象
        shm_unlink(SHM_NAME);
        exit(EXIT_FAILURE);
    }
    printf("Shared memory object size set to %d bytes.\n", SHM_SIZE);

    // 3. 将共享内存对象映射到进程的地址空间
    // mmap(addr, length, prot, flags, fd, offset)
    // NULL: 让系统选择映射的起始地址
    // SHM_SIZE: 映射的长度
    // PROT_READ | PROT_WRITE: 内存区域可读可写
    // MAP_SHARED: 多个进程可以共享此映射
    // shm_fd: 共享内存文件描述符
    // 0: 从文件开头映射
    shm_ptr = mmap(NULL, SHM_SIZE, PROT_READ | PROT_WRITE, MAP_SHARED, shm_fd, 0);
    if (shm_ptr == MAP_FAILED) {
        perror("mmap failed");
        shm_unlink(SHM_NAME);
        exit(EXIT_FAILURE);
    }
    printf("Shared memory mapped to address: %p\n", shm_ptr);

    // 4. 关闭共享内存文件描述符（映射建立后即可关闭，不影响映射本身）
    close(shm_fd); 

    // 5. 创建子进程
    pid = fork();

    if (pid == -1) {
        perror("fork failed");
        munmap(shm_ptr, SHM_SIZE);
        shm_unlink(SHM_NAME);
        exit(EXIT_FAILURE);
    } else if (pid == 0) {
        // 子进程逻辑
        printf("\n[Child Process] Starting...\n");
        sleep(1); // 等待父进程写入数据

        printf("[Child Process] Attempting to read from shared memory...\n");
        char buffer[SHM_SIZE];
        strcpy(buffer, (char *)shm_ptr); // 从共享内存读取数据

        printf("[Child Process] Read: \"%s\"\n", buffer);

        // 验证数据
        if (strcmp(buffer, MESSAGE) == 0) {
            printf("[Child Process] Data verification: SUCCESS! Read data matches expected message.\n");
        } else {
            printf("[Child Process] Data verification: FAILED! Read data does not match expected message.\n");
        }

        printf("[Child Process] Exiting.\n");
        exit(EXIT_SUCCESS);
    } else {
        // 父进程逻辑
        printf("\n[Parent Process] Starting...\n");
        printf("[Parent Process] Writing \"%s\" to shared memory...\n", MESSAGE);
        strcpy((char *)shm_ptr, MESSAGE); // 向共享内存写入数据
        printf("[Parent Process] Data written. Waiting for child...\n");

        // 等待子进程结束
        int status;
        waitpid(pid, &status, 0);

        if (WIFEXITED(status) && WEXITSTATUS(status) == EXIT_SUCCESS) {
            printf("[Parent Process] Child process exited successfully.\n");
        } else {
            printf("[Parent Process] Child process exited with an error.\n");
        }

        // 6. 解除内存映射
        if (munmap(shm_ptr, SHM_SIZE) == -1) {
            perror("munmap failed");
        }
        printf("[Parent Process] Shared memory unmapped.\n");

        // 7. 删除共享内存对象 (仅在不再需要时，通常由最后一个使用它的进程删除)
        // shm_unlink(name): 删除共享内存对象的名称。当所有对此对象的映射和文件描述符都关闭后，对象本身才会被销毁。
        if (shm_unlink(SHM_NAME) == -1) {
            perror("shm_unlink failed");
        }
        printf("[Parent Process] Shared memory object '%s' unlinked (deleted).\n", SHM_NAME);

        printf("--- POSIX Shared Memory Test Complete ---\n");
        exit(EXIT_SUCCESS);
    }

    return 0; // 不会到达这里
}