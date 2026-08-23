# 页缓存

StarryX 的页缓存由 `xcache` 提供机制、由 `xkernel` 完成组合。普通文件的
buffered I/O 和 file-backed mmap 共享同一组缓存页；设备、伪文件和未声明
coherent 的文件系统显式绕过缓存。

```text
xvfs identity/raw I/O     xmm Frame      xtask WaitQueue
          \                  |                 /
           +-------------- xcache ------------+
                              |
                       page/object traits
                              v
                            xvma
                              ^
                              |
                 xkernel adapters + worker
```

## 分层边界

- `xmm` 只负责 `Frame` 生命周期和受限字节访问。`FrameMeta` 仍只有一个
  引用计数，不保存 dirty、reclaim 或文件信息。
- `xcache` 负责缓存索引、缺页合并、dirty/writeback、截断协调和回收。
  它是 `forbid(unsafe_code)` 的独立组件，不依赖 `xfs`、`xvfs`、
  `xvma`、`xprocess`、`xruntime` 或 `xkernel`。
- `xvma` 负责 VMA、PTE、私有 COW、共享写保护、fork、mprotect、unmap
  和 TLB 顺序。
- `xvfs` 只为参与缓存的文件提供所有别名共享的 `CacheSlot` 挂载点；返回
  `None` 的伪文件、设备和其他对象直接绕过缓存。slot 只保存一个类型不透明
  的 `Weak` 附件，因此 `xvfs` 不依赖任何缓存实现。
- `xkernel` 将 VFS 原始 I/O 适配为 cache backing，将缓存页适配给 xvma，
  并创建唯一的后台维护任务。

因此 `xcache` 不是通用的“进程页回收器”，也不是文件系统的一部分；它是
位于文件数据、物理页和映射机制之间的独立协调组件。

## 稳定映射与缓存页

文件身份采用 Linux 的“身份即对象”思想：缓存不以任何数值 key 全局索引，
而是直接挂在文件对象上。每个支持缓存的普通文件在文件系统内共享一个
`Arc<CacheSlot>` —— 同一 inode 的所有硬链接别名、open handle 和 cache
backing 持有同一个 slot；文件系统只保存 `ino → Weak<CacheSlot>`，因此最后
一个别名消失后，复用的 inode number 一定得到新 slot，不会继承旧缓存。slot
内保存 `Weak<FileMapping>`：打开路径先升级它，失败才创建新 mapping 并以
compare-and-attach 解决并发竞争，落败者随即释放自己的 mapping。

`FileMapping` 的 `u64` 身份来自 `xvma` 的统一对象 id 分配器：文件缓存和
匿名共享 `VmObject` 共用一个单调、不复用的计数器，由构造保证永不冲突。该
id 只用于 `VmObject`/futex 命名和 registry 钉住，不再充当查找 key。

`CacheManager` 的 registry 对仍含缓存页的 `FileMapping` 保持强引用，避免
最后一个 fd 关闭后脏页消失；此时 mapping 持有的 backing 保留文件对象与
slot，重新打开仍会聚合到同一 mapping。页面树按 4-KiB page index 保存：

```rust
enum PageSlot {
    Loading(Arc<LoadAttempt>),
    Resident(Arc<CachedPage>),
}
```

第一个 miss 安装 `LoadAttempt` 并执行 backing read；并发 miss 只等待同一个
attempt。结果先写入 attempt 自己的 completion，再唤醒等待者。失败只删除
仍与该 attempt 匹配的 slot，所以新的加载不会覆盖旧等待者应看到的错误。

`CachedPage` 独占一个 cache-owned `Frame`，`PageLease` 记录一次临时使用。
映射到 PTE 时会克隆 Frame；回收因此可以用 `Frame::is_unique()` 判断 cache
是否是最后一个所有者，而无需在 xmm 中增加 page-cache 元数据。

仅由 registry 持有的 clean 页和空 mapping 只在内存压力下由后台维护清除，
因此关闭后重新打开的文件仍能命中缓存。unlink 先由
文件系统提交目录项删除；只有提交成功、`nlink == 0` 且 mapping 没有外部
owner 时，xkernel 才直接丢弃缓存，因而不会在失败的 unlink 或仍有硬链接时
丢失 dirty 数据。若仍有 fd/VMA，ext4 只移除目录链接，inode 与数据块由最后
一个共享 lifetime token 延迟释放。因此从任意硬链接打开的旧 fd 在所有名称
unlink 后仍可读写、sync，并且不会污染复用后的新文件。

## 写入与回写

缓存页使用单调序列而不是互斥的枚举状态：

```rust
struct PageState {
    leases: u32,
    dirty_seq: u64,
    submitted_seq: u64,
    persisted_seq: u64,
    writeback_seq: Option<u64>,
    failed_seq: Option<u64>,
    shared_guard_groups: u32,
}
```

一次写入递增 `dirty_seq`。回写在页锁内复制序列 `S` 的 4-KiB snapshot，
标记 `writeback_seq = Some(S)`，随后释放全部 cache lock 再执行 backing I/O。
成功只推进 `persisted_seq` 到 `S`；若 I/O 期间又发生写入，新的
`dirty_seq > persisted_seq` 仍然是脏页。失败记录 `failed_seq`、保留脏页并
唤醒等待者。后台线程不会无限重试同一失败序列，显式 sync 可以重试。

每个打开文件持有独立 `WritebackCursor`。`fsync`、`fdatasync` 与同步
`msync` 完成范围内页写回、文件长度提交和 backing sync 后，按类似 errseq
的方式向每个 cursor 最多一次报告最新未观察错误。

## mmap 一致性

`xvma` 不区分“文件 backing”和“共享内存 backing”，而是把来源与写策略拆成
两个正交维度：`Source::{Zero, Static, Object}` 决定页面来自哪里，`private`
决定写入使用 COW 还是写透。缓存文件和匿名共享内存都只是 `VmObject` 的实现。
`VmObject::page` 返回 `Frame` 与可选的 opaque write guard；只有 xkernel 的
`FileVmObject` 知道如何把 `PageLease` 适配成这个通用接口。

`MAP_PRIVATE` 首次 fault 可以映射缓存 Frame，但 PTE 不带 WRITE；首次写
fault 通过 xvma 原有 COW 事务复制 Frame，此后不再修改文件页。

`MAP_SHARED` 的读 fault 同样先映射只读。写 fault 在开放 WRITE 之前取得
`SharedWriteGuard`，使页面变脏且不可回收。VmSpace 按虚拟页保存 guard：

- fork 克隆同一个 guard group；
- mprotect、munmap、truncate invalidation、exit 和 Drop 先撤销 WRITE 并
  flush TLB，再释放最后一个 guard；
- 失败事务保留旧 PTE 和旧 guard；
- buffered read/write 和所有 shared mapping 立即观察同一 Frame。

这种保守 pinning 不依赖反向映射或硬件 dirty bit，代价是 writable shared
mapping 存续期间页面不能被回收。

## truncate 与失效

`xkernel::MappedFiles` 为每个地址空间、每个缓存文件保存一个
`ObserverRegistration`。registration token 强持有 observer，`FileMapping`
只保存 Weak，因此 lifetime 随 VMA tree 结束且没有引用环；xvma 本身不保存
file/cache observer 状态。新增或 fork 得到的映射通过同一个 mapping admission
gate 注册，它要么在 shrink 的 observer snapshot 前完成，要么在 gate 关闭后
失败。munmap、MAP_FIXED、exec 和 exit 都以当前 VMA tree 为准清理过期 token。

缩短文件时 mapping 暂停新操作，等待已进入操作退出。所有 observer 首先通过
xmm 的只读 leaf 校验执行无副作用 `validate`；全部成功后，才用不分配、不可
失败的 `invalidate` 移除超出新 EOF 的 PTE。随后等待 load、lease、writeback、
guard 和 PTE Frame 引用排空，最后调用原始 `set_len`。

`set_len` 是唯一不可逆步骤；其后的 tail zero、slot 删除和 logical size
发布均不分配且不会失败。若任一 validate 失败，PTE、VMA、缓存和磁盘状态都
保持旧版本。访问完全越过当前 EOF 的文件页产生 bus fault。

## 回收与后台任务

manager 维护一个 weak-reference clock 候选队列。访问只设置原子
`referenced` 位；第一轮扫描清位，后续扫描才尝试回收。直接回收满足：

- 不分配、不等待、不执行 I/O；
- 不持有 allocator 或 filesystem lock；
- 只移除 clean、无 lease、无 guard、无 writeback 且 Frame unique 的页；
- 失败时页面和候选项保持可用。

`xkernel` 的单个 worker 根据空闲页和脏页水位执行批量 writeback 与 clean
reclaim。它由 `xtask` 执行，但 xcache 不依赖具体 task/process 类型。

关机采用显式 `Running -> Closing -> Closed` 协议：xkernel 只启动
`run_worker` 并调用 `shutdown`；xcache 内部停止接收操作并完成一次有限回写与
clean reclaim。worker 已启动时由它发布结果；尚未启动时由 shutdown 同步完成，
之后才获得调度的 worker 会直接退出。永久 writeback error 或残留
dirty/resident page 直接作为错误返回；Drop 不隐式执行 I/O 或丢弃数据。

## 锁与安全约束

cache lock 不跨越 backing I/O、sleep、observer callback 或 Frame allocation。
嵌套顺序为 manager lifecycle/registry、mapping gate/page tree、page state、
candidate queue，状态发布后再通知 waiter。observer callback 进入 VmSpace
时 xcache 不持有 mapping/page lock。

缓存通过 `Frame::read_bytes`/`write_bytes` 在当前支持的单 hart userspace
模型下复制数据。每次有界 copy 内部关闭抢占和中断，不暴露 Rust slice 或
reference，也无需公开一个只做转发的 access object；未发布 Frame 的初始化
仍使用要求唯一引用的 `Frame::try_write_at`。

## 验证与性能

新增 guest cases 覆盖 buffered I/O、unlink 后旧 fd、4096 文件 churn、私有/
共享 mmap、truncate、sync、低内存回收、泄漏核对和两次启动后的持久化。
两启动用同一 disposable image，第二次启动完成后才生成不可变报告。完整
lmbench 不删减命令或参数；仅按单 hart TCG 的实测运行时间设置监督预算。

iozone 保持历史 workload、`SMP=1 MEM=1G LOG=off MODE=release` 和三次独立
fresh boot。机器可读证据位于
[iozone-page-cache.json](../benchmarks/iozone-page-cache.json)，比较方法与
结果说明位于 [iozone-page-cache.md](../benchmarks/iozone-page-cache.md)。
门禁是 33 个指标的三次中位数全部严格高于无页缓存基线，不使用 aggregate、
tolerance 或 smoke workload。
