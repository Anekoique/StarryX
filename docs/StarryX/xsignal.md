## 信号系统

`xsignal`组件是StarryX中负责信号处理和管理的核心模块，它为内核提供了完整的UNIX风格信号处理机制。该模块实现了标准信号和实时信号的管理、信号动作配置、信号挂起队列管理以及跨平台的信号处理支持。

在传统的宏内核设计中，信号处理通常与进程管理、内存管理等子系统紧密耦合，这使得信号系统难以独立测试和维护。在StarryX的组件化设计中，我们将信号处理抽象为独立的`xsignal`组件，通过trait抽象解除与其他子系统的依赖关系，使其可以被不同的操作系统实现复用。在组件化OS中实现信号系统需要面对以下挑战：

- 多模块依赖: 信号处理涉及进程管理、线程管理、用户空间访问等多个模块
- 平台相关性: 不同架构下信号处理的细节存在差异

为了解决这些挑战，我们将`xsignal`设计为一个高度模块化的组件，通过trait抽象隐藏底层实现细节，同时保证信号处理的正确性和高效性。

在这个组件中，我们设计了核心的信号类型和管理结构：

```rust
// 信号类型
pub enum Signo { SIGHUP = 1, SIGINT = 2, ... SIGRT32 = 64 }
pub struct SignalSet(u64);
pub struct SignalInfo(pub siginfo_t);

// 信号动作
pub struct SignalAction {
    pub flags: SignalActionFlags,
    pub mask: SignalSet,
    pub disposition: SignalDisposition,
    pub restorer: __sigrestore_t,
}

// 挂起信号管理
pub struct PendingSignals {
    pub set: SignalSet,
    info_std: [Option<SignalInfo>; 32],      // 标准信号合并
    info_rt: [VecDeque<SignalInfo>; 33],     // 实时信号队列
}
```

针对其具体功能我们设计了`WaitQueue`trait抽象线程等待行为，再通过复用`xuspace`的`UserSpaceAccess`trait实现安全的用户空间访问

```rust
// 线程等待抽象
pub trait WaitQueue: Default {
    fn wait_timeout(&self, timeout: Option<Duration>) -> bool;
    fn wait(&self);
    fn notify_one(&self) -> bool;
    fn notify_all(&self);
}

// 信号处理函数中安全访问用户空间
fn handle_signal<A: UserSpaceAccess>(
    &self,
    uspace: &A,
    tf: &mut TrapFrame,
    // ...
) -> Option<SignalOSAction>
```

另外需要实现对于信号处理的接口，这里接口需要实现：

- 维护进程级和线程级的信号状态管理
- 提供安全的信号投递与处理接口
- 支持标准信号合并和实时信号队列

这里的接口实现依赖于宏内核具体的进程管理和线程调度方法，因此我们抽象了双层管理架构，其暴露接口让内核实现信号处理的状态管理，其余接口提供默认的信号处理方法给实现了该trait的管理器，这些方法封装了信号操作并调用内核实现的接口实现状态同步从而避免了直接操作底层数据。

```rust
/// 进程级信号管理器
pub struct ProcessSignalManager<M, WQ> {
    pending: Mutex<M, PendingSignals>,
    pub actions: Arc<Mutex<M, SignalActions>>,
    pub(crate) wq: WQ,
    pub(crate) default_restorer: usize,
}

/// 线程级信号管理器  
pub struct ThreadSignalManager<M, WQ> {
    proc: Arc<ProcessSignalManager<M, WQ>>,
    pending: Mutex<M, PendingSignals>,
    blocked: Mutex<M, SignalSet>,
    stack: Mutex<M, SignalStack>,
}
```

经过对双层信号管理和生命周期管理的实现，`xsignal`为宏内核提供了稳定可靠的信号抽象和处理能力，同时保持了良好的可扩展性和可维护性。

其他OS只要实现`WaitQueue`抽象接口就可以复用相关API实现信号处理。

```rust
impl WaitQueue for MyWaitQueue {
    fn wait_timeout(&self, timeout: Option<Duration>) -> bool { ... }
    fn notify_one(&self) -> bool { ... }
    // ... 其他方法
}

// 使用xsignal组件
let proc_mgr = ProcessSignalManager::<MyMutex, MyWaitQueue>::new(...);
let thread_mgr = ThreadSignalManager::new(Arc::new(proc_mgr));
```

通过`xsignal`组件，StarryX实现了高效且可扩展的信号处理系统，为宏内核提供了完整的UNIX信号语义支持，同时保持了良好的模块化设计和跨平台兼容性。
