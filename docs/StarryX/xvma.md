# 内存映射管理

`xvma` 是 StarryX 的安全用户虚拟内存策略组件。每个进程只拥有一个
`xvma::VmSpace`，它统一维护全部 VMA；`xkernel` 不再同时维护独立的
文件映射表。

```text
xkernel (Linux ABI、文件和信号适配)
                  |
                  v
xvma::VmSpace (VMA、权限、缺页和对象策略)
                  |
                  v
xmm::AddressSpace (页表、PTE、TLB 和映射引用机制)
```

`VmSpace` 使用以起始地址为 key 的 `BTreeMap` 保存三种 backing：
`Static`、`Private` 和 `Shared`，并私有持有 `xmm::AddressSpace`。`mmap`、
`munmap`、`mprotect`、`fork` 和缺页都通过这个单一所有者执行，因此区域
切分、合并与实际页表状态不会再由两个公共管理器分别协调。

源码布局按完整职责而不是单个操作拆分：`space.rs` 管理 `VmSpace` 生命周期、
地址布局以及 map/unmap/protect/fork；`area.rs` 维护 `VmArea` 的区间、切分与
合并不变量；`backend.rs` 封装 backing 状态和静态分发；`fault.rs` 实现缺页、
populate 与 COW；`object.rs` 定义独立的内存对象所有权契约。fork 只是
`VmSpace` 生命周期的一部分，不再为一个方法单独建立模块。

创建映射只使用一个入口：

```rust
VmSpace::map(start, size, flags, Backend::...)
```

公开的 `Backend` 还携带 `populate` 等一次性创建参数；安装完成后，VMA 只保留
`Static`、`Private` 或 `Shared` 所需的长期状态。crate-private
`AreaBackend` trait 统一分发 slice/merge、map/unmap、protect、fault 和 fork
行为。VMA 树仍存放封闭 enum 并使用静态分发，不使用 `dyn Backend`，因此没有
每个 VMA 的额外堆分配、vtable 或 downcast。

这个枚举表达的是生命周期而不是映射来源：匿名映射是无 source 的
`Private`，私有文件映射是有 source 的 `Private`，两者自然共享 fault、fork、
COW 和 mprotect 规则。`Shared` 由 `SharedObject` 保留稳定身份和
`Box<[Frame]>`；`Static` 则不拥有计数引用。

`Static` 并不接受裸 `PhysAddr`。安全的 `Backend::static_frames` 需要
`StaticFrameRange`，由受信任组件证明物理范围具有 kernel-long lifetime，
并记录允许的最大访问权限。这样 allocator-backed `Frame` 无法仅凭公开物理
地址通过安全 API 伪装成静态帧；VMA split/fork 只切分或复制该 proof token。
Static VMA 不参与相邻区域合并，从而保证一个 VMA 的完整范围始终由同一个
未扩张的 token 覆盖。

文件后端由 `VmObject` 抽象：

```rust
pub trait VmObject: Send + Sync {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> LinuxResult<usize>;
    fn byte_len(&self) -> LinuxResult<u64>;
}
```

该接口不依赖 `xfs`、`xkernel` 或 `xcache`。当前由内核文件包装器完成同步
读取；未来 `xcache::FileMapping` 可以实现同一边界，增加稳定页身份、脏页、
回写和回收，而无需让 VMA 组件了解具体文件系统。

缺页处理返回 `Resolved`、`Retry`、`Segv`、`Bus` 或 `NoMemory`。内核只负责
有界重试并将策略结果转换为 Linux 信号；文件偏移越界产生 `SIGBUS`，普通
权限或地址错误产生 `SIGSEGV`。

`xvma` 使用 `#![forbid(unsafe_code)]` 固定安全边界，只通过 `xmm` 操作页表。
共享内存对象直接持有 `Frame`，每个 resident `Alloc` PTE 也持有一份计数
引用。映射和 fork 只克隆普通 `Frame`，unmap 只释放相应 PTE 的引用；因此
shared lifetime 由最小 `FrameMeta` 自然维护，`xmm` 无需认识共享对象。匿名和
私有文件 COW 使用相同模型。普通权限页和 `PROT_NONE` 页都不维护第二套逐页
索引：页表的软件位标记 resident-but-inaccessible leaf，PTE 继续保存 PFN 和
`ALLOC_FRAME` owner。Static backing 同样保留 PTE，并由原
`StaticFrameRange` token 约束权限恢复。

Alloc Frame 与 SharedObject 当前固定为 4 KiB，相关 API 不再重复传递或保存
`PageSize::Size4K`。只有 Static backing 保留显式页表粒度，以支持 2 MiB/1 GiB
静态映射。munmap 与进程 teardown 直接遍历 authoritative PTE，不克隆 VMA 或
构造 resident-leaf journal。

跨多个 VMA 的 `mprotect` 由 `xmm::ProtectionTransaction` 执行：一次预检统计
resident leaves 并预留 journal，第二次 walk 记录 address、old flags 和目标
flags，随后直接 apply，不再经过另一轮普通 protect preflight。全部 backing
更新成功后才 commit VMA tree；任意中途失败都会按逆序恢复，rollback 不进行
分配。`xvma` 不保存 Static 专用快照，也不分别调用 Alloc/Static restore API；
它只向事务提供 backing policy，例如根据 xmm 提供的 Alloc-frame exclusivity
决定 Private COW 页是否保留 WRITE。

缺页路径同样避免零碎查询：`mapping_flags` 一次区分 absent、resident-accessible
和 resident-inaccessible；只有 Private write fault 才继续调用
`frame_if_shared`，它在独占时不产生临时 Frame clone，共享时才返回复制源。

`MAP_PRIVATE` 文件页按需读取到私有 `Frame`；当前 `MAP_SHARED` 文件映射保留
兼容性 snapshot 语义：一次 mmap 创建一个 `SharedObject`，fork 后继续共享，
但不同 mmap 之间暂不一致，也没有 writeback。稳定页身份由后续 xcache 接入。
