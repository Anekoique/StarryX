# Todo

## 6.1

Progress：

- Impl System V Ipc
- run lmbench succressfully

Todo：

- add docs
- impl Posix Ipc
- Page Cache （msync）and copy-on-write
- Modify axfs_ng's management of cache and mount points
- Iperf and netpert
- Io mpx：epoll
- glibc libctest

## 5.28

Progress:

- replace axfs with axfs-ng
- impl system V shm
- pass musl Basic/libc/busybox/lua/iozone

Todo:

- Impl System V sem and msg
- add docs
- Modify the implemented syscall
- pass glibc Basic/libc/busybox/lua/iozone
- start lmbench

## 5.20

- add docs
- new filesystem (reffer to axfs-ng/axfs-ng-vfs/axfs)
- testcode (shmem need)
  - libctest (pthread_cancel_points pthread_robust_detach)
  - iozone

## 5.18

- add docs

- testcode (fix libctest -> busybox test)
  - busybox (df;free ;hwclock ;kill pid10)
  - libctest (socket pthread_cancel_points pthread_robust_detach)
  - Fix the error of tls_get_new_dtv in multi-core scenario (libctest) (futex? or membarriar?)
- share memory/cow related system features (future)

## 5.12

- refactor,replace current project with a more clear and hierarchical structure(src—core—module—api)

- more syscall
- share memory/cow related system features
- fix the file stat wrong implementation

# StarryX-Record

启动流程 _start(axhal) -> rust_entry(axhal) -> rust_main(axruntime) -> main(starry) -> run_user_app


run_user_app major process
1. 进入执行程序目录
2. 加载用户elf文件
3. 初始化用户上下文
4. 创建用户任务和线程数据
5. 复制全局命名空间数据到本地线程空间
6. 创建进程和线程
7. 阻塞主任务并调度

## Structure

```mermaid
graph LR
    subgraph "API structure"
        Utils[utils]
        Backend[backend]
        Syscall[syscall]
        
        Utils -->|"提供工具"| Syscall
        Backend -->|"提供抽象"| Syscall
        Utils -.->|"支持"| Backend
    end
    class Utils,Backend,Syscall core;
```



api部分重构为utils，backend，syscall三个部分，utils为syscall提供的“工具”，所有syscall均可复用比如ptr；backend为syscall的后端实现，提供可被复用的抽象trait,datastrucre and functions,可被同一模块的多个syscall复用

## 进程管理

### 数据结构

```mermaid
graph TD
    subgraph "Task 层 (基础调度单元)"
        Task["Task/TaskInner
        - id: TaskId
        - name: String
        - state: TaskState
        - ctx: TaskContext
        - kstack: TaskStack
        - task_ext: AxTaskExt"]
    end
    
    subgraph "Task扩展层 (连接Task和Thread)"
        TaskExt["TaskExt
        - time: TimeStat
        - thread: Arc<Thread>"]
    end
    
    subgraph "Thread 层 (线程)"
        Thread["Thread
        - tid: Pid
        - process: Arc<Process>
        - data: Box<dyn Any>"]
        
        ThreadData["ThreadData
        - clear_child_tid: AtomicUsize"]
    end
    
    subgraph "Process 层 (进程)"
        Process["Process
        - pid: Pid
        - is_zombie: AtomicBool
        - tg: ThreadGroup
        - data: Box<dyn Any>
        - children: StrongMap<Pid, Arc<Process>>
        - parent: Weak<Process>
        - group: Arc<ProcessGroup>"]
        
        ProcessData["ProcessData
        - exe_path: String
        - aspace: Arc<Mutex<AddrSpace>>
        - ns: AxNamespace
        - heap_bottom/top: AtomicUsize"]
        
        ThreadGroup["ThreadGroup
        - threads: WeakMap<Pid, Weak<Thread>>
        - exit_code: i32
        - group_exited: bool"]
        
        ProcessGroup["ProcessGroup
        - pgid: Pid
        - session: Arc<Session>
        - processes: WeakMap<Pid, Weak<Process>>"]
        
        Session["Session
        - sid: Pid
        - process_groups: WeakMap<Pid, Weak<ProcessGroup>>"]
    end
    
    %% 连接关系
    Task -->|拥有| TaskExt
    TaskExt -->|引用| Thread
    Thread -->|属于| Process
    Thread -->|拥有| ThreadData
    Process -->|拥有| ProcessData
    Process -->|管理| ThreadGroup
    ThreadGroup -->|包含| Thread
    Process -->|归属于| ProcessGroup
    ProcessGroup -->|归属于| Session
    Process -->|子进程关系| Process
```



```rust
axprocess::Process::new_init(axtask::current().id().as_u64() as _).build();
```

相关数据结构

```rust
/// A builder for creating a new [`Process`].
pub struct ProcessBuilder {
    pid: Pid,
    parent: Option<Arc<Process>>,
    data: Box<dyn Any + Send + Sync>,
}

/// A process.
pub struct Process {
    pid: Pid,
    is_zombie: AtomicBool,
    pub(crate) tg: SpinNoIrq<ThreadGroup>,

    data: Box<dyn Any + Send + Sync>,

    // TODO: child subreaper
    children: SpinNoIrq<StrongMap<Pid, Arc<Process>>>,
    parent: SpinNoIrq<Weak<Process>>,

    group: SpinNoIrq<Arc<ProcessGroup>>,
}

pub(crate) struct ThreadGroup {
    pub(crate) threads: WeakMap<Pid, Weak<Thread>>,
    pub(crate) exit_code: i32,
    pub(crate) group_exited: bool,
}

/// A [`ProcessGroup`] is a collection of [`Process`]es.
pub struct ProcessGroup {
    pgid: Pid,
    pub(crate) session: Arc<Session>,
    pub(crate) processes: SpinNoIrq<WeakMap<Pid, Weak<Process>>>,
}

/// A [`Session`] is a collection of [`ProcessGroup`]s.
pub struct Session {
    sid: Pid,
    pub(crate) process_groups: SpinNoIrq<WeakMap<Pid, Weak<ProcessGroup>>>,
    // TODO: shell job control
}
```

### 资源限制

## 信号机制

````mermaid
classDiagram
    class Signo {
        +SIGHUP, SIGINT, SIGKILL, etc.
        +is_realtime() bool
        +default_action() DefaultSignalAction
    }
    
    class DefaultSignalAction {
        <<enumeration>>
        Terminate
        Ignore
        CoreDump
        Stop
        Continue
    }
    
    class SignalOSAction {
        <<enumeration>>
        Terminate
        CoreDump
        Stop
        Continue
        Handler
    }
    
    class SignalSet {
        -u64 value
        +add(signal: Signo) bool
        +remove(signal: Signo) bool
        +has(signal: Signo) bool
        +dequeue(mask: SignalSet) Option~Signo~
        +to_ctype(dest: kernel_sigset_t)
    }
    
    class SignalInfo {
        -siginfo_t raw_info
        +new(signo: Signo, code: u32)
        +signo() Signo
        +set_signo(signo: Signo)
        +code() u32
        +set_code(code: u32)
    }
    
    class SignalActionFlags {
        <<bitflags>>
        +SIGINFO
        +NODEFER
        +RESETHAND
        +RESTART
        +ONSTACK
        +RESTORER
    }
    
    class SignalDisposition {
        <<enumeration>>
        Default
        Ignore
        Handler
    }
    
    class SignalAction {
        +flags: SignalActionFlags
        +mask: SignalSet
        +disposition: SignalDisposition
        +restorer: __sigrestore_t
        +to_ctype(dest: kernel_sigaction)
    }
    
    class SignalStack {
        +sp: usize
        +flags: u32
        +size: usize
        +disabled() bool
    }
    
    class PendingSignals {
        +set: SignalSet
        -info_std: [Option~SignalInfo~; 32]
        -info_rt: [VecDeque~SignalInfo~; 33]
        +new()
        +put_signal(sig: SignalInfo) bool
        +dequeue_signal(mask: SignalSet) Option~SignalInfo~
    }
    
    Signo --> DefaultSignalAction: defines default action
    SignalDisposition --> Signo: references for handlers
    SignalInfo --> Signo: contains signal number
    SignalSet --> Signo: manages set of signals
    SignalAction --> SignalDisposition: defines action
    SignalAction --> SignalSet: holds blocked signals
    SignalAction --> SignalActionFlags: configures behavior
    PendingSignals --> SignalSet: tracks pending signals
    PendingSignals --> SignalInfo: stores signal info
```

````



### 注册信号

### 发送信号

## Futex

## FileSystem

### axfs-ng

```mermaid
graph TD
    %% Top level - Userspace syscalls
    Syscalls["System Calls (open, read, write, etc.)"]
    
    %% API Layer
    ApiLayer["API Layer (starry-Mivik/api)"]
    FileOps["FileLike Trait\n(read, write, stat, etc.)"]
    FdTable["FD_TABLE\n(File Descriptor Management)"]
    KStat["Kstat Struct\n(File metadata in Linux format)"]
    
    %% File implementations
    FileImpl["File Implementations"]
    RegularFile["File\n(Regular File)"]
    DirFile["Directory\n(Directory File)"]
    Pipe["Pipe"]
    Socket["Socket"]
    Stdio["Standard IO"]
    
    %% AXFS-NG Layer
    AxfsLayer["axfs-ng Layer"]
    FsContext["FsContext<M>\n(Filesystem Context)"]
    HighLevelFile["File<M>\n(High-level file operations)"]
    OpenOptions["OpenOptions\n(File open parameters)"]
    ReadDir["ReadDir\n(Directory iterator)"]
    
    %% VFS Layer
    VfsLayer["VFS Layer (axfs-ng-vfs)"]
    Location["Location<M>\n(File/Dir reference)"]
    Metadata["Metadata\n(File attributes)"]
    Path["Path\n(Filesystem paths)"]
    FileNode["FileNode\n(File operations)"]
    
    %% Filesystem implementations
    FsImpl["Filesystem Implementations"]
    Ext4["Ext4 Filesystem\n(lwext4_rust)"]
    Fat["FAT Filesystem\n(fatfs)"]
    
    %% Physical storage
    BlockDevice["Block Device Layer"]
    
    %% Relationships - Top to bottom
    Syscalls --> ApiLayer
    
    ApiLayer --> FileOps
    ApiLayer --> FdTable
    ApiLayer --> KStat
    
    FileOps --> FileImpl
    
    FileImpl --> RegularFile
    FileImpl --> DirFile
    FileImpl --> Pipe
    FileImpl --> Socket
    FileImpl --> Stdio
    
    RegularFile --> AxfsLayer
    DirFile --> AxfsLayer
    
    AxfsLayer --> FsContext
    AxfsLayer --> HighLevelFile
    AxfsLayer --> OpenOptions
    AxfsLayer --> ReadDir
    
    FsContext --> VfsLayer
    HighLevelFile --> VfsLayer
    
    VfsLayer --> Location
    VfsLayer --> Metadata
    VfsLayer --> Path
    VfsLayer --> FileNode
    
    Location --> FsImpl
    
    FsImpl --> Ext4
    FsImpl --> Fat
    
    Ext4 --> BlockDevice
    Fat --> BlockDevice
    
    %% Key functional flows
    subgraph Key_Operations
        ResolveAt["resolve_at()\n(Path resolution)"]
        WithFs["with_fs()\n(FS context access)"]
    end
    
    ApiLayer --> ResolveAt
    ApiLayer --> WithFs
    ResolveAt --> FsContext
    WithFs --> FsContext
    
    FsContext -.-> |"resolve()"| Location
    HighLevelFile -.-> |"read/write/seek"| FileNode
```

starry

## PageCache

文件读取

```mermaid
sequenceDiagram
    participant 用户进程
    participant 内核
    participant 磁盘
    
    用户进程->>内核: read() 系统调用
    内核->>address_space: 通过 file->f_mapping 定位
    activate address_space
    address_space->>i_pages: 用文件偏移查找缓存页
    alt 缓存命中
        i_pages-->>address_space: 返回 struct page
        address_space-->>内核: 直接复制数据到用户空间
    else 缓存未命中
        address_space->>内存管理: 分配新 page
        address_space->>a_ops: 调用 readpage()
        a_ops->>磁盘: 发起 I/O 读取
        磁盘-->>a_ops: 返回数据
        a_ops->>address_space: 填充 page
        address_space->>i_pages: 插入新缓存页
        address_space-->>内核: 返回数据
    end
    deactivate address_space
    内核-->>用户进程: 返回数据
```

文件写入

```mermaid

sequenceDiagram
    participant 用户进程
    participant 内核
    participant 磁盘
    
    用户进程->>内核: write() 系统调用
    内核->>address_space: 获取缓存页（类似读取）
    activate address_space
    alt 缓存存在
        address_space->>page: 写入数据
        address_space->>a_ops: set_page_dirty()
    else 缓存不存在
        address_space->>内存管理: 分配新 page
        address_space->>page: 写入数据
        address_space->>i_pages: 插入缓存
        address_space->>a_ops: set_page_dirty()
    end
    deactivate address_space
    
    内核-->>用户进程: 返回成功
    
    后台->>address_space: 定期唤醒
    activate address_space
    address_space->>i_pages: 查找脏页
    loop 遍历脏页
        address_space->>a_ops: 调用 writepage()
        a_ops->>磁盘: 写入数据
        磁盘-->>a_ops: 确认写入
        a_ops->>address_space: 清除脏标志
    end
    deactivate address_space
```

###### 内存映射

```mermaid
sequenceDiagram
    participant 用户进程
    participant 内核
    participant CPU MMU
    
    用户进程->>内核: mmap() 系统调用
    内核->>address_space: 创建 VMA 并插入 i_mmap 树
    内核-->>用户进程: 返回虚拟地址
    
    用户进程->>CPU MMU: 首次访问虚拟地址
    CPU MMU->>内核: 触发缺页中断
    内核->>address_space: 通过 VMA 定位
    activate address_space
    address_space->>i_pages: 用文件偏移查找缓存页
    alt 缓存存在
        i_pages-->>address_space: 返回 struct page
    else 缓存不存在
        address_space->>a_ops: 调用 readpage() 加载数据
    end
    address_space->>MMU: 设置页表映射
    deactivate address_space
    内核-->>CPU MMU: 恢复执行
    CPU MMU-->>用户进程: 正常访问内存
```























## ArceOS change

### axhal

```rust
// arch/loongarch64
pub use self::context::{TaskContext, TrapFrame, GeneralRegisters};
```

# Questions

- C类型使用混乱，调用core::ffi，linux_raw_sys，如果要调用areaos_posix_api，还需要调用areos_posix_api中的ctypes
- 同步原语使用混乱，axsync，spin，lock_api(axsignal)

# Testcases

## libc

### musl

- all
  - pthread related
  - socket

- x86_64:

  - fwscanf

  - snprintf
  - sscanf/sscanf_long
  - strtod/strtod_simple/strtof/strtold
  - swprintf
  - fpclassify_invalid_ld80
  - printf_1e9_oob/printf_fmt_g_round/printf_fmt_g_zeros/sscanf_eof

x86的部分测例在本地也无法通过，考虑是测例本身的问题

### glibc



