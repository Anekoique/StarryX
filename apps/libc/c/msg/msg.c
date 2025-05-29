#include <sys/ipc.h>
#include <sys/msg.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>
#include <stdlib.h>
#include <errno.h>

const int MOD = 998244353;
int msg_id;

// 消息结构体
struct msg_buffer {
    long msg_type;
    int data[10];
};

int parent() {
    printf("Parent process started\n");
    struct msg_buffer message;
    
    // 等待子进程启动
    printf("Parent waiting for child to be ready...\n");
    sleep(1);
    
    // 发送消息给子进程
    message.msg_type = 1;
    for (int i = 0; i < 10; i++) {
        message.data[i] = i;
    }
    
    printf("Parent sending initial data to child...\n");
    if (msgsnd(msg_id, &message, sizeof(message.data), 0) == -1) {
        perror("msgsnd failed in parent");
        return 1;
    }
    
    printf("Parent waiting for child to process and send back data...\n");
    
    // 接收子进程处理后的消息 - 增加重试机制
    struct msg_buffer received_msg;
    int retry_count = 0;
    int max_retries = 10;
    
    while (retry_count < max_retries) {
        if (msgrcv(msg_id, &received_msg, sizeof(received_msg.data), 2, IPC_NOWAIT) == -1) {
            if (errno == ENOMSG) {
                printf("No message yet, retrying... (%d/%d)\n", retry_count + 1, max_retries);
                sleep(1);
                retry_count++;
                continue;
            } else {
                perror("msgrcv failed in parent");
                return 1;
            }
        } else {
            break; // 成功接收到消息
        }
    }
    
    if (retry_count >= max_retries) {
        printf("Timeout waiting for child response\n");
        return 1;
    }
    
    printf("Parent received processed data from child\n");
    
    // 验证数据
    int success = 1;
    for (int i = 0; i < 10; i++) {
        if (received_msg.data[i] != i + MOD) {
            printf("Data mismatch at index %d: expected %d, got %d\n", 
                   i, i + MOD, received_msg.data[i]);
            success = 0;
        }
    }
    
    if (success) {
        printf("Check passed! All data correctly processed.\n");
    } else {
        printf("Check failed!\n");
        return -1;
    }
    
    // 等待子进程结束
    wait(NULL);
    
    // 删除消息队列
    if (msgctl(msg_id, IPC_RMID, NULL) == -1) {
        perror("msgctl failed");
        return 1;
    }
    
    return 0;
}

int child() {
    printf("Child process started\n");
    
    struct msg_buffer message;
    
    // 等待并接收来自父进程的消息
    printf("Child waiting for data from parent...\n");
    int retry_count = 0;
    int max_retries = 10;
    
    while (retry_count < max_retries) {
        if (msgrcv(msg_id, &message, sizeof(message.data), 1, IPC_NOWAIT) == -1) {
            if (errno == ENOMSG) {
                printf("Child: No message yet, retrying... (%d/%d)\n", retry_count + 1, max_retries);
                sleep(1);
                retry_count++;
                continue;
            } else {
                perror("msgrcv failed in child");
                return 1;
            }
        } else {
            break; // 成功接收到消息
        }
    }
    
    if (retry_count >= max_retries) {
        printf("Child: Timeout waiting for parent message\n");
        return 1;
    }
    
    printf("Child received data from parent\n");
    
    // 处理数据 - 加上MOD值
    printf("Child processing data...\n");
    for (int i = 0; i < 10; i++) {
        message.data[i] += MOD;
    }
    
    // 发送处理后的数据回父进程
    message.msg_type = 2;  // 使用不同的消息类型
    printf("Child sending processed data back to parent...\n");
    if (msgsnd(msg_id, &message, sizeof(message.data), 0) == -1) {
        perror("msgsnd failed in child");
        return 1;
    }
    
    printf("Child process finished\n");
    return 0;
}

// 额外的测试函数 - 测试消息队列的各种功能
void test_msgqueue_features() {
    printf("\n=== Testing Message Queue Features ===\n");
    
    int test_msg_id = msgget(IPC_PRIVATE, IPC_CREAT | 0666);
    if (test_msg_id == -1) {
        perror("msgget failed for feature test");
        return;
    }
    
    struct msg_buffer test_msg;
    
    // 测试1: 发送不同类型的消息
    printf("Test 1: Sending messages with different types\n");
    for (int type = 1; type <= 3; type++) {
        test_msg.msg_type = type;
        for (int i = 0; i < 10; i++) {
            test_msg.data[i] = type * 100 + i;
        }
        
        if (msgsnd(test_msg_id, &test_msg, sizeof(test_msg.data), IPC_NOWAIT) == -1) {
            perror("msgsnd failed in feature test");
        } else {
            printf("Sent message type %d\n", type);
        }
    }
    
    // 测试2: 接收特定类型的消息
    printf("Test 2: Receiving specific message types\n");
    
    // 接收类型为2的消息
    if (msgrcv(test_msg_id, &test_msg, sizeof(test_msg.data), 2, IPC_NOWAIT) != -1) {
        printf("Received message type %ld, first data: %d\n", 
               test_msg.msg_type, test_msg.data[0]);
    }
    
    // 接收任意类型的消息（队列中的第一个）
    if (msgrcv(test_msg_id, &test_msg, sizeof(test_msg.data), 0, IPC_NOWAIT) != -1) {
        printf("Received first message, type %ld, first data: %d\n", 
               test_msg.msg_type, test_msg.data[0]);
    }
    
    // 测试3: 获取消息队列状态
    printf("Test 3: Getting message queue status\n");
    struct msqid_ds queue_status;
    if (msgctl(test_msg_id, IPC_STAT, &queue_status) == 0) {
        printf("Queue info - Messages: %lu, Bytes: %lu\n", 
               queue_status.msg_qnum, queue_status.msg_cbytes);
    }
    
    // 清理
    msgctl(test_msg_id, IPC_RMID, NULL);
    printf("Feature tests completed\n");
}

int main() {
    // printf("Creating message queue...\n");
    
    // // 创建消息队列
    // msg_id = msgget(IPC_PRIVATE, IPC_CREAT | 0666);
    // if (msg_id == -1) {
    //     perror("msgget failed");
    //     return 1;
    // }
    
    // printf("Message queue created with ID: %d\n", msg_id);
    
    // // 创建子进程
    // pid_t pid = fork();
    // if (pid == -1) {
    //     perror("fork failed");
    //     return 1;
    // }
    
    // if (pid == 0) {
    //     // 子进程
    //     return child();
    // } else {
    //     // 父进程
    //     return parent();
    // }
    test_msgqueue_features();
    return 0;
}
