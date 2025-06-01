#include <sys/ipc.h>
#include <sys/sem.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>
#include <stdlib.h>
#include <errno.h>


union semun {
    int val;                    // SETVAL的值
    struct semid_ds *buf;       // IPC_STAT, IPC_SET的缓冲区
    unsigned short *array;      // GETALL, SETALL的数组
};


int sem_id;

int parent() {
    printf("父进程已启动\n");
    
    // 等待子进程启动
    printf("父进程等待子进程准备就绪...\n");
    sleep(1);
    
    // 父进程获取信号量（作为互斥锁使用）
    printf("父进程尝试获取信号量...\n");
    struct sembuf sem_op;
    sem_op.sem_num = 0;    // 操作信号量0
    sem_op.sem_op = -1;    // P操作（减1）
    sem_op.sem_flg = 0;    // 阻塞等待
    
    if (semop(sem_id, &sem_op, 1) == -1) {
        perror("父进程获取信号量失败");
        return 1;
    }
    
    printf("父进程已获取信号量，进入临界区...\n");
    
    // 模拟临界区操作
    printf("父进程在临界区工作3秒...\n");
    sleep(3);
    
    printf("父进程离开临界区，释放信号量...\n");
    
    // 释放信号量
    sem_op.sem_op = 1;     // V操作（加1）
    if (semop(sem_id, &sem_op, 1) == -1) {
        perror("父进程释放信号量失败");
        return 1;
    }
    
    printf("父进程已释放信号量\n");
    
    // 等待子进程结束
    wait(NULL);
    
    // 删除信号量集
    if (semctl(sem_id, 0, IPC_RMID) == -1) {
        perror("删除信号量集失败");
        return 1;
    }
    
    printf("父进程成功完成\n");
    return 0;
}

int child() {
    printf("子进程已启动\n");
    
    // 子进程也尝试获取信号量
    printf("子进程尝试获取信号量...\n");
    struct sembuf sem_op;
    sem_op.sem_num = 0;    // 操作信号量0
    sem_op.sem_op = -1;    // P操作（减1）
    sem_op.sem_flg = 0;    // 阻塞等待
    
    if (semop(sem_id, &sem_op, 1) == -1) {
        perror("子进程获取信号量失败");
        return 1;
    }
    
    printf("子进程已获取信号量，进入临界区...\n");
    
    // 模拟临界区操作
    printf("子进程在临界区工作2秒...\n");
    sleep(2);
    
    printf("子进程离开临界区，释放信号量...\n");
    
    // 释放信号量
    sem_op.sem_op = 1;     // V操作（加1）
    if (semop(sem_id, &sem_op, 1) == -1) {
        perror("子进程释放信号量失败");
        return 1;
    }
    
    printf("子进程已释放信号量\n");
    printf("子进程完成\n");
    return 0;
}

// 测试信号量的各种功能
void test_semaphore_features() {
    printf("\n=== 测试信号量功能 ===\n");
    
    // 创建信号量集（3个信号量）
    int test_sem_id = semget(IPC_PRIVATE, 3, IPC_CREAT | 0666);
    if (test_sem_id == -1) {
        perror("创建信号量集失败");
        return;
    }
    printf("创建了信号量集，ID为: %d\n", test_sem_id);
    
    union semun arg;
    
    // 测试1: SETVAL和GETVAL
    printf("\n测试1: SETVAL 和 GETVAL\n");
    arg.val = 5;
    if (semctl(test_sem_id, 0, SETVAL, arg) == -1) {
        perror("SETVAL 操作失败");
    } else {
        printf("将信号量0的值设置为5\n");
    }
    
    int val = semctl(test_sem_id, 0, GETVAL);
    if (val == -1) {
        perror("GETVAL 操作失败");
    } else {
        printf("获取信号量0的值: %d\n", val);
    }
    
    // 测试2: SETALL和GETALL
    printf("\n测试2: SETALL 和 GETALL\n");
    unsigned short values[3] = {10, 20, 30};
    arg.array = values;
    if (semctl(test_sem_id, 0, SETALL, arg) == -1) {
        perror("SETALL 操作失败");
    } else {
        printf("将所有信号量设置为 [10, 20, 30]\n");
    }
    
    unsigned short result[3];
    arg.array = result;
    if (semctl(test_sem_id, 0, GETALL, arg) == -1) {
        perror("GETALL 操作失败");
    } else {
        printf("获取所有信号量的值: [%d, %d, %d]\n", 
               result[0], result[1], result[2]);
    }
    
    // 测试3: 基本的semop操作
    printf("\n测试3: 基本的semop操作\n");
    struct sembuf ops[2];
    
    // 对信号量0进行P操作（减3）
    ops[0].sem_num = 0;
    ops[0].sem_op = -3;
    ops[0].sem_flg = IPC_NOWAIT;  // 非阻塞
    
    if (semop(test_sem_id, ops, 1) == -1) {
        perror("P操作失败");
    } else {
        printf("P操作成功，信号量0减少了3\n");
        val = semctl(test_sem_id, 0, GETVAL);
        printf("信号量0的当前值: %d\n", val);
    }
    
    // 对信号量1进行V操作（加5）
    ops[0].sem_num = 1;
    ops[0].sem_op = 5;
    ops[0].sem_flg = 0;
    
    if (semop(test_sem_id, ops, 1) == -1) {
        perror("V操作失败");
    } else {
        printf("V操作成功，信号量1增加了5\n");
        val = semctl(test_sem_id, 1, GETVAL);
        printf("信号量1的当前值: %d\n", val);
    }
    
    // 测试4: 多个操作的原子性
    printf("\n测试4: 多个操作的原子性\n");
    ops[0].sem_num = 0;
    ops[0].sem_op = -2;
    ops[0].sem_flg = IPC_NOWAIT;
    ops[1].sem_num = 2;
    ops[1].sem_op = -5;
    ops[1].sem_flg = IPC_NOWAIT;
    
    if (semop(test_sem_id, ops, 2) == -1) {
        printf("预期失败的原子操作（资源不足）: %s\n", 
               strerror(errno));
    } else {
        printf("原子操作成功\n");
    }
    
    // 测试5: IPC_STAT
    printf("\n测试5: IPC_STAT\n");
    struct semid_ds sem_info;
    arg.buf = &sem_info;
    if (semctl(test_sem_id, 0, IPC_STAT, arg) == -1) {
        perror("IPC_STAT 操作失败");
    } else {
        printf("信号量集信息 - 信号量数量: %lu\n", 
               sem_info.sem_nsems);
    }
    
    // 测试6: 测试错误情况
    printf("\n测试6: 错误情况测试\n");
    
    // 尝试访问不存在的信号量
    val = semctl(test_sem_id, 5, GETVAL);  // 信号量5不存在
    if (val == -1) {
        printf("预期错误，访问不存在的信号量: %s\n", 
               strerror(errno));
    }
    
    // 尝试设置超出范围的值
    arg.val = -1;  // 负值
    if (semctl(test_sem_id, 0, SETVAL, arg) == -1) {
        printf("预期错误，设置负值: %s\n", 
               strerror(errno));
    }
    
    // 清理
    if (semctl(test_sem_id, 0, IPC_RMID) == -1) {
        perror("清理失败");
    } else {
        printf("功能测试完成并清理\n");
    }
}

int main() {
    printf("=== System V 信号量测试程序 ===\n");
    
    // 首先运行功能测试
    test_semaphore_features();
    
    printf("\n=== 进程同步测试 ===\n");
    
    // 创建信号量集（1个信号量用作互斥锁）
    sem_id = semget(IPC_PRIVATE, 1, IPC_CREAT | 0666);
    if (sem_id == -1) {
        perror("创建信号量集失败");
        return 1;
    }
    
    printf("创建了信号量，ID为: %d\n", sem_id);
    
    // 初始化信号量为1（互斥锁）
    union semun arg;
    arg.val = 1;
    if (semctl(sem_id, 0, SETVAL, arg) == -1) {
        perror("初始化信号量失败");
        return 1;
    }
    printf("信号量初始化为1\n");
    
    // 创建子进程
    pid_t pid = fork();
    if (pid == -1) {
        perror("创建子进程失败");
        return 1;
    }
    
    if (pid == 0) {
        // 子进程
        return child();
    } else {
        // 父进程
        return parent();
    }
}