## 进程管理

`xprocess`是实现StarryX进程管理的核心组件，它提供了完整的进程生命周期管理、进程间关系维护以及线程组织功能。它实现了数据管理与生命周期的管理的分离，将POSIX标准下的进程间关系由组件进行维护，而进程和线程的相关内部数据提供接口交由具体的OS实现，具有良好的灵活性和可扩展性。

![xprocess](./images/xprocess.png)

其内部具体实现了基本的进程管理组织：

```rust
/// 线程
pub struct Thread {
    tid: ...        // 线程id
    process: ...    // 所属进程
    data: ...       // 线程数据
}

/// 线程组
pub struct ThreadGroup {
    threads：...     // 线程集合
    exit_code: ...   // 退出码
    group_exited: ...// 是否退出
}

/// 进程
pub struct Process {
    pid: ...        // 进程id
    is_zombie: ...  // 僵尸进程
    tg: ...         // 线程组
    data: ...       // 进程数据接口
    children: ...   // 子进程
    parent: ...     // 父进程
    group: ...      // 进程组
}

/// 进程组
pub struct ProcessGroup {
    pgid: ...       // 进程组id
    session: ...    // 会话 
    processes: ...  // 进程集合
}

/// 会话
pub struct Session {
    sid: ...            // 会话id
    process_groups: ... // 所属进程组
}
```

这些数据结构实现了基本的进程层次管理与基本功能，在这基础上我们提供了进程创建退出等api让OS灵活地进行生命周期的维护：

```rust
/// 进程构建器
pub struct ProcessBuilder {
    data<T>(data: T) -> Self        // 设置进程数据
    build() -> Arc<Process>         // 构建进程实例
}

/// 进程创建
impl Process {
    new_init(pid: Pid) -> ProcessBuilder    // 创建init进程
    fork(pid: Pid) -> ProcessBuilder        // 创建子进程
}

/// 进程状态控制
impl Process {
    exit(self: &Arc<Self>)                  // 进程退出，转为僵尸状态
    group_exit(&self)                       // 标记整个线程组退出
    free(&self)                             // 释放僵尸进程资源
}

/// 线程构建器等...
```

经过对进程层次管理和生命周期管理的实现，`xprocess`为宏内核提供了稳定可靠的进程抽象和管理能力，同时保持了良好的可扩展性和可维护性。