# StarryX page cache 重构研究

## 1. 结论

StarryX 不应复制 Linux 的 `address_space/XArray/LRU/zone/bdi` 全套机制，也不应继续扩展当前基于裸 `PhysAddr` 和单一 `LruCache` 的原型。适合当前边界的最小设计是：

1. `xcache` 以“稳定文件对象 + 页索引”作为唯一页身份，拥有缓存页、加载合并、dirty/writeback/error、候选隔离和统计；页内存只由 `xmm::Frame` 持有。
2. `xfs/xvfs` 保持原始 backing I/O 和 inode 语义，不依赖 `xcache`；`xkernel` 用 adapter 把稳定 VFS node 包成 `xcache::Backing`，并保证普通读写、mmap、truncate、fsync 都经过同一 `FileMapping`。
3. `xvma` 继续独占 VMA、fault、COW、MAP_PRIVATE/MAP_SHARED 策略，只看 `Frame` 和抽象文件页源，不依赖 `xcache/xfs`；桥接由 `xkernel` 完成。
4. 等待和后台执行只用 `xtask`，worker 由 `xkernel` 启动；`xcache` 不得依赖或遍历 `xprocess`。
5. 首版用一个全局 worker、三个内存水位、两个脏页阈值、clean-only direct reclaim。可写 MAP_SHARED 在缺少反向映射/write-protect dirty tracking 时采用保守 pin，不能把仍可被用户写的页清成 clean。

这不是 Linux clone。应吸收的是稳定页身份、每页状态机、I/O 与索引锁分离。Asterinas 的“每 inode 一个 cache、buffered I/O 与 mmap 共用 VMO、页级 single-flight、快照写回允许 redirty”与 StarryX 接近；其 OSTD frame metadata、反向映射和完整 VMO 锁网则不应搬入当前项目。

## 2. 证据标记与范围

- **[现状]**：来自当前 StarryX 源码或已确认 SPEC。
- **[上游]**：来自文末 Linux/Asterinas 官方源码或文档。
- **[设计]**：针对 StarryX 的推断和取舍，不表示上游已经如此实现。

本报告只研究通用 page cache 机制及其 MM/FS/task 接口，不设计进程模型、不引入 `xprocess`，也不改变 xtest 框架职责。

## 3. StarryX 现状

### 3.1 当前 xcache 原型应替换而非修补

**[现状]** `xmodules/xcache/src/lib.rs` 存在结构性问题：

- 页由裸 `PhysAddr` 表示并手工分配/释放，绕过已确认的 `xmm::Frame` 所有权。
- `load_page` 在 backing I/O 后才插入；并发 miss 会重复分配、重复读取并相互覆盖，没有 single-flight。
- `sync` 持有整张 LRU 的互斥锁做 I/O，慢设备会阻塞 lookup、fault、dirty 和 reclaim。
- `Dirty -> ToWrite -> WriteBack -> UpToDate` 没有代际。写回期间 redirty 后，旧完成会覆盖新 dirty。
- 错误没有稳定记录和 fsync 可观察游标，部分路径会永久停在 `WriteBack`。
- `evict_lru` 没有候选隔离，也不知道页是否仍被 PTE、fault 或 buffered I/O 使用。
- `file_size` 与 VFS inode 重复，却没有 append/truncate 串行规则。

### 3.2 已确认边界

**[现状]** `.ark/specs/features/kernel/mm/redesign-mm-subsystem/SPEC.md` 已规定：

- `xmm::FrameMeta` 只有一个私有原子引用计数，page-cache 状态不得进入 FrameMeta。
- `xmm::Frame` 是 allocator-backed 4 KiB frame 的所有权句柄。
- `xmm::AddressSpace` 独占 PTE 修改和本地 TLB；`xvma::VmSpace` 独占 VMA、fault、fork/COW/backing policy。
- `xcache` 可持有 `Frame` 和自身状态，`xmm` 不承载 page-cache policy。

**[现状]** 当前 `xvma` 的文件 MAP_PRIVATE 在 fault 时经 `VmObject::read_at` 复制到新 Frame；文件 MAP_SHARED 则由 `xkernel/src/syscall/mm/mmap.rs` 预读到匿名 `SharedObject`。后者只在 fork 间共享，不与另一次 mmap 或 buffered I/O coherent，也不写回。`msync` 仅告警后返回成功。

**[现状]** `xkernel/src/mm/uspace.rs` 已将 `FileWrapper` 标为临时桥接。接口需要从“读取字节”升级为“取得稳定缓存页 Frame + 映射生命周期/失效通知”。

**[现状]** `xfs::FsFile` 是带 file position 的 open handle；`xvfs::FileNodeOps` 提供原始 `read_at/write_at/set_len/sync`。cache identity 不能使用 fd、pathname 或 `Arc<Mutex<FsFile>>`。

**[现状]** `xalloc::global_allocator()` 已提供 `used_pages()/available_pages()`；`xtask` 已有 `WaitQueue`、`spawn_raw` 和 yield/sleep。首版无需让 allocator 或 process 层承担 cache policy。

### 3.3 稳定文件身份

**[现状]** `Location::metadata().device` 来自 `Mountpoint` 的递增编号；同一 filesystem 再次挂载会得到不同 device。因此 `(mount device, inode)` 会把同一文件拆成多个 cache。

**[设计]** `xvfs` 应暴露不含 cache policy 的 opaque `FileIdentity`，语义为 `(filesystem instance identity, inode)`。过渡期可由 xkernel 用 `FilesystemOps` trait object 的 data pointer 加 inode，并由 backing 强引用保证 filesystem 生命周期；正式实现应由 xvfs 分配 filesystem ID。注册表存 `Weak<FileMapping>` 并惰性清理。

## 4. 上游事实与取舍

### 4.1 Linux

**[上游]**

- VFS 用每 inode 的 `address_space` 组织 page cache/mmap，页按 mapping 内 index 查找并带 dirty/writeback 标记。
- `filemap_add_folio` 在 mapping 索引中原子插入；并发加载者必须处理已存在页。这支持“mapping + index 唯一”和 single-flight。
- 当前 vmscan 把普通文件 dirty writeback 与 reclaim 分开；reclaim 隔离候选、优先回收 clean 页，避免 direct reclaim 进行不可控随机 I/O。
- 低水位唤醒后台 reclaim，临界水位触发 direct reclaim；dirty background 阈值唤醒写回，高阈值节流 writer。StarryX 应保留水位/滞回思想，不复制 zone/NUMA/bdi。
- mapping error sequence 配合每 open file 的错误游标，让不同 open description 各自观察自上次检查后的写回错误。
- truncate/invalidate 与 fault/writeback 有显式串行；只删索引页不能使已安装 PTE 失效。

### 4.2 Asterinas

**[上游]** 以下以官方提交 `9503fbdb07ec6d5e8470de9956348c660261b487` 为准：

- 每文件 inode 一个 `PageCache`，其 VMO 同时服务 buffered I/O、fault、resize、flush 和 invalidation。
- cache page 有初始化/最新/脏/淘汰状态、页锁和独立 writeback 位；加载采用“检查—锁页—再检查—唯一初始化者”。
- 写回复制稳定快照后释放页锁提交 I/O，期间可 redirty。提交失败重新 dirty；完成错误结束 writeback，并留下需要错误传播的 TODO，不会无限自动重试。
- 缩小时先处理 cache，扩展时先扩文件；部分尾页清零，完整越界页 decommit；invalidate 先 flush dirty 再 evict。

**[设计]** 采用其页级 single-flight、快照写回和 file-level resize 串行；拒绝其 frame metadata、全量 VMO/rmap。Asterinas 当前 error propagation 的 TODO 也不能照搬。

## 5. 目标职责

| 层 | 负责 | 不负责 |
|---|---|---|
| `xmm` | Frame 分配/计数/受控页字节拷贝；AddressSpace PTE/TLB | 文件身份、dirty、LRU、写回 |
| `xcache` | FileMapping、页索引、single-flight、dirty/writeback/error、reclaim、统计 | 路径、fd、VMA、进程、设备实现 |
| `xvma` | VMA、fault、MAP_PRIVATE COW、MAP_SHARED 权限/失效 token | xfs 类型、全局 cache 注册表、backing I/O |
| `xfs/xvfs` | inode 和原始 read/write/resize/sync | Frame/PTE/page-cache policy |
| `xkernel` | identity 注册表、adapters、syscall 语义、worker 配置 | 复制 cache 状态机 |
| `xtask` | wait/wake 和 worker 执行 | cache policy |
| `xprocess` | 无 | 不得成为依赖 |

依赖方向：

```text
xmm  <--- xcache <--- xkernel cache/fs adapter ---> xfs/xvfs
              ^                 |
              |                 v
            xtask       xvma file-object adapter
                              |
                         xmm::AddressSpace
```

`xcache` 可使用 `xtask::WaitQueue`，但 worker 的创建、周期和关机由 `xkernel` 管理。

## 6. 核心对象、身份和所有权

**[设计]** 建议形态：

```rust
pub trait Backing: Send + Sync {
    fn read_at(&self, offset: u64, dst: &mut [u8]) -> LinuxResult<usize>;
    fn write_at(&self, offset: u64, src: &[u8]) -> LinuxResult<usize>;
    fn set_len(&self, len: u64) -> LinuxResult<()>;
    fn sync(&self, data_only: bool) -> LinuxResult<()>;
}

pub struct FileMapping {
    identity: FileIdentity,
    backing: Arc<dyn Backing>,
    logical_size: AtomicU64,
    operation: Mutex<FileOperationState>,
    pages: Mutex<BTreeMap<u64, PageSlot>>,
    errors: MappingErrorState,
    observers: Mutex<Vec<Weak<dyn InvalidationObserver>>>,
}

enum PageSlot {
    Loading(Arc<CachePage>),
    Resident(Arc<CachePage>),
    Isolating(Arc<CachePage>),
}

struct PageStatus {
    dirty_seq: u64,
    settled_seq: u64,
    writeback_seq: Option<u64>,
    shared_writable_pins: u32,
}
```

`CachePage` 持有一个 cache-owner `Frame`；PageLease、PTE、临时 I/O 通过 clone 延长生命周期。`xmm` 需要安全的页字节拷入/拷出及“是否仅 cache owner”查询，但不得加入 dirty/LRU，也不得要求 xcache 写 unsafe。

硬不变量：

1. 一个活跃 `FileIdentity + page index` 至多一个 mapping/slot。
2. 仅 `Loading` winner 初始化 Frame；失败删除同一 slot、唤醒 waiter，绝不发布半初始化页。
3. mapping/index/page 锁不得跨 backing I/O、wait 或 xvma 回调。
4. 页仅在 clean、非 writeback/loading、无 writable pin、无 lease/PTE Frame 引用时回收。
5. 回收前 slot 必须进入 `Isolating`；新 lookup 只能等待/重试，不能创建第二页。

## 7. Lookup 与 single-flight

**[设计]** miss 路径：

1. 短暂 index lock 查询 page index。
2. `Resident`：取 lease、更新访问代际并返回。
3. `Loading/Isolating`：取得 wait queue，释放锁后睡眠，再重查。
4. 真 miss：先分配页并插入 `Loading` placeholder，再释放锁。
5. winner 做 backing read；短读与 EOF 尾部清零。成功后先 publish initialized，再改为 `Resident` 并唤醒。
6. 失败时仅在 slot 仍指向该页时删除，保存错误给 waiter；重试由上层决定。

## 8. Dirty、writeback、redirty 与错误

**[设计]** 每次 cache 写在页锁下递增 `dirty_seq`。写回：

1. 取 `dirty_seq = S`，复制稳定 4 KiB 快照；
2. 设置 `writeback_seq = S`，释放页锁；
3. 无 cache 内部锁提交 backing I/O；
4. 完成后：成功推进 `settled_seq`；提交前失败不推进，保持 dirty；已完成 I/O 错误记录 mapping error sequence，并把该尝试标为 settled-failed，避免无限重试；若 `dirty_seq > S`，页始终仍 dirty。

“clean”定义为 `dirty_seq <= settled_seq` 且无 writable shared pin。永久错误后允许离开 dirty 队列，但 `fsync/fdatasync/msync` 必须返回错误。

`FileMapping` 记录单调 `error_seq` 和最近错误；每个 xkernel open file description 保存 `seen_error_seq`，`dup` 共享 cursor，不同 open 各自观察。fsync 捕获调用开始时的目标 dirty sequence，排队并等待其 settle，调用 `Backing::sync`，最后查询并推进 cursor；任一步失败都返回错误。

**[风险]** 当前 ext4 `NodeOps::sync` 是 no-op，而 FAT 有实际 flush。page cache 可保证交付给 xfs，不能虚构掉电持久性；若要宣称 ext4 fsync durability，需单独补足 xfs/lwext4 flush contract。

## 9. Buffered I/O 与 mmap coherence

**[设计]** 普通 `read/write/pread/pwrite/readv/writev/append` 必须经同一 mapping；raw `FsFile` I/O 只供 Backing 或明确 bypass。file position 仍在 open description。

- read 按页 lookup 并按 EOF 截断。
- write 对部分页先加载，整页覆盖可免读；页锁内复制并 dirty。
- append 由 file operation mutex 原子分配 EOF，不能让多个 fd 各自读 size。

MAP_PRIVATE：xvma 可在只读 fault 映射 cache Frame clone；首个私有写 fault 仍由 xvma deep-copy 并 COW。xcache 不感知 VMA 类型。

MAP_SHARED：所有映射引用同一 FileMapping。读 fault 取 cache Frame；写 fault 先 `mark_shared_writable` 再由 xvma 安装 writable PTE。`MS_SYNC` 做范围 writeback barrier + backing sync；`MS_ASYNC` 只排队。

只在第一次 write fault 标 dirty 不足：若写回把页清成 clean，仍 writable 的 PTE 后续写不会再 fault。首版采用 conservative pin：

- 可写 shared VMA/驻留页持有 shared-writable pin；
- pin 存在时页一直视为 dirty、不可 reclaim；
- fsync/msync 可写当前快照，但不使其变为可回收 clean；
- 最后一个 pin 释放时再次 dirty，后续成功写回后才 clean。

代价是可写共享映射占内存和重复写回；只有 xvma 后续支持 rmap、写保护全部共享 PTE、TLB shootdown 和再次 write fault 后才能移除此限制。

## 10. Truncate、失效与文件大小

**[设计]** `FileMapping::logical_size` 是 cache/fault 的快速权威值，所有修改由 xkernel 的 mapping operation lock 串行；xfs inode size 是 backing 权威值，只允许 adapter 的 resize 流程共同更新。

缩短文件必须：

1. 发布 `invalidating + generation`，使新 fault/lookup 等待或 `Retry`；
2. 在不持 cache 内部锁时通知弱 xvma observer，由 `xmm::AddressSpace` 失效相关 PTE/TLB；
3. 等待 in-flight load/writeback 到安全点；
4. 删除完整越界页，最后保留页从新 EOF 到页尾清零；
5. backing `set_len` 成功后发布新 logical size；
6. 清 invalidating、推进 generation 并唤醒。

扩展先成功扩 backing，再发布新 size；洞页读零。backing 失败时 size 不变。

xcache 不得遍历进程或直接改 PTE。xvma 注册弱 `InvalidationObserver` token；xcache 收集 token 后释放 index/page/observer 锁再回调。fault 在取得页前后核对 generation，变化则丢弃结果并 `Retry`。否则 truncate 若只删除 cache entry，已驻留 PTE 仍能访问旧 Frame，不能算正确 MVP。

## 11. Reclaim、水位与脏页节流

### 11.1 一个 worker、三水位、两阈值

**[设计]** xkernel 启动一个 `xcache-worker`，使用 `xalloc::available_pages()`：

- `free_high`：后台 reclaim 停止目标；
- `free_low`：低于它唤醒 worker；
- `free_min`：低于它，cache 分配路径先 direct reclaim clean 页再重试；
- `dirty_background`：超过它唤醒 writeback；
- `dirty_limit`：超过它，新 writer 在无锁状态等待脏页下降或错误/进度。

阈值由 xconfig 的绝对页数或总页数比例给出，并保持 `free_min < free_low < free_high`、`dirty_background < dirty_limit`。不要照抄 Linux 默认百分比。

### 11.2 候选隔离

**[设计]** 首版用 generation/clock 或 active/inactive 队列，不实现 MGLRU：

1. 短锁选老页，把 slot 原子改为 `Isolating`；
2. 释放全局队列/索引锁；
3. 检查 clean、非 writeback、无 writable pin，且 CachePage/Frame 无外部引用；
4. 满足则删除 slot 并释放 cache-owner Frame；
5. 不满足则恢复 `Resident`，按原因降级/排队 writeback，唤醒 waiter。

direct reclaim 只回收 clean 页，不在分配者上下文写 dirty 页。若没有 clean 页，唤醒 worker、等待一次可观测进度并有限重试；仍无进展才返回 ENOMEM。worker 应批量写连续 index，合并为较大 `write_at`，避免每 4 KiB 一次 I/O。

### 11.3 生命周期

- 注册表持有 `Weak<FileMapping>`；fd、VMA、worker 队列持有强引用。
- mapping index 持有 cache-owner `Arc<CachePage>`；lease/PTE 持额外 Arc/Frame。
- PTE 只持 Frame，所以 reclaim 还必须查询 Frame 引用计数。
- 关闭最后 fd 不等于丢 cache；VMA/worker 仍可保持 mapping。
- 卸载前阻止新 lookup、flush/error-drain、移除 registry，再释放 backing。

## 12. 锁序

**[设计]**

```text
xvma VmSpace lock
  -> FileMapping operation/generation gate
    -> page index / reclaim queue short lock
      -> CachePage state lock
        -> xmm Frame internal mechanism
```

强制规则：

- backing I/O、wait、worker join、xvma observer 回调时不持 index/reclaim/page 锁。
- truncate 不得按 `FileMapping lock -> VmSpace lock` 回调；用 invalidating generation、无锁回调和 fault 双重核对。
- writeback completion 只取目标页锁和 mapping error lock，不重入 xfs file operation mutex。
- dirty throttling 释放页锁后再等待。
- xkernel registry 锁只用于获取/插入 Weak，不跨 mapping 构造或 I/O。
- 不同锁序必须先写成 lock contract 并加入并发测试。

## 13. 最小可行设计与阶段

### Stage 0：基线与观测

- 冻结 correctness 清单和 iozone 参数。
- 加只读计数器：hit/miss、coalesced load、dirty/writeback/error、clean/failed eviction、direct-reclaim stall、backing bytes。
- cache 关闭时复跑历史命令确认基线。

### Stage 1：机制与 buffered I/O

- 用 `Frame + FileMapping + PageSlot` 替换裸 PhysAddr 原型。
- 实现稳定 registry、single-flight、整页覆盖免读、部分页 read-modify-dirty。
- xkernel 路由 read/write/pread/pwrite/append；xfs 仍是 raw backing。
- 暂不改变 xvma，先证明缓存 I/O、多 fd 和 hard-link coherence。

### Stage 2：writeback/fsync/truncate/reclaim

- 加 sequence 状态、快照 writeback、redirty、error cursor。
- fsync/fdatasync 成为真正 barrier；实现 tail-zero/invalidation。
- 启动一个 xkernel worker，接入水位、dirty threshold、clean-only direct reclaim 和连续页 batch。
- 这是“可安全默认开启 page cache”的最小门槛。

### Stage 3：统一 mmap

- 扩展 xvma 抽象 file object 为页级 Frame acquisition，xkernel adapter 桥接 xcache。
- MAP_PRIVATE 读共享 cache Frame、写 COW。
- MAP_SHARED 统一身份，接入 observer、msync、truncate PTE invalidation。
- 可写 MAP_SHARED 先 conservative pin；rmap/write-protect dirty tracking 另立任务。

### Stage 4：严格门禁调优

- correctness、并发和故障注入测试不削弱的前提下调 batch、预读窗口、队列和水位。
- 若所有历史指标未严格超过基线，任务不验收；不能删慢项、改参数或把 vector fallback 改名为无效。

## 14. 拒绝的替代方案

| 方案 | 结论与理由 |
|---|---|
| 修补 `LruCache<u64, PhysAddr>` | 拒绝：身份、所有权、single-flight、writeback 都是错误抽象 |
| 每 fd/路径一个 cache | 拒绝：hard link、多次 open/mmap 不 coherent |
| `(mount device, inode)` identity | 拒绝：同一 filesystem 多挂载会分裂 |
| xfs 依赖 xcache/Frame | 拒绝：raw backing 与缓存策略反向耦合 |
| dirty/LRU 放 `FrameMeta` | 拒绝：违反已确认 xmm SPEC |
| xcache 查询 xprocess | 拒绝：失效应用弱 observer token |
| 复制 Asterinas VMO/rmap 或 Linux XArray/MGLRU | 拒绝：超出当前需求并扩大锁网/unsafe |
| direct reclaim 同步写 dirty 页 | 首版拒绝：分配路径出现不可控设备延迟 |
| write fault 标脏一次后允许清 writable MAP_SHARED 页 | 拒绝：后续 CPU 写无 fault，会静默丢 dirty |
| fsync 只启动异步写回或用 sticky bool | 拒绝：不满足 barrier 和多 open error |
| truncate 只删 cache entry | 拒绝：驻留 PTE 仍访问旧 Frame |

## 15. 测试方法

### 15.1 不可削弱的 correctness

先用内存 `FakeBacking` 做可控并发/故障注入：

- N 个并发 miss 仅一次 backing read；winner 失败时 waiter 得到一致错误且可重试。
- 快照写回期间 redirty，旧完成不能清新 dirty。
- submission failure 保持 dirty；completion error 被每个 open cursor 恰好观察。
- reclaim 与 lookup/fault/writeback 竞态无双页、UAF、dirty 丢失。
- truncate 跨页/页边界/零长度，尾页清零，扩展洞读零。
- 多 fd append 原子 EOF；hard link/重复 open 命中同一 mapping。

xtest/QEMU 至少覆盖：

- buffered read/write/pread/pwrite/readv/writev/append、seek/EOF；
- fsync/fdatasync 后重启或重新挂载可读；
- MAP_PRIVATE 读共享、写 COW、fork COW；
- 两次 MAP_SHARED 与 buffered I/O 双向可见；
- MS_SYNC/MS_ASYNC、映射中 truncate 越界 fault 和部分尾页；
- 小内存并发读写 + reclaim + truncate，无死锁/永久 WriteBack；
- ext4 与受支持 backing；伪文件/设备文件明确 bypass。

现有测试不得删除、放宽期望或减少阶段；新增 cache 测试只能叠加。

### 15.2 强制性能门禁

**[现状/门禁]** 必须完全复用 `docs/benchmarks/iozone-no-page-cache.md`：

- baseline `b76f4d7138e1d9bd02d660cf0bbad1c9c611ded6`，xtest `59faed8281fd17234d682144a7fcd70accb0a6ad`；
- iozone 3.506 RISC-V musl static；
- riscv64 release，`SMP=1 MEM=1G LOG=off`；
- QEMU 11.0.0 virt、virtio-blk PCI、raw ext4；
- Apple M4 / 24 GiB / macOS 26.5，相同 Rust toolchain；
- `make test ARCH=riscv64 CASE=testsuit/iozone/run SMP=1 MEM=1G LOG=off MODE=release`；
- fresh disposable image，三次独立运行，比较三次中位数。

每个有效记录指标的新中位数都必须 **严格大于** 下表历史值（KiB/s），不是平均更高，也没有容差：

| 指标 | 基线 | 指标 | 基线 |
|---|---:|---|---:|
| auto.write | 19883 | auto.rewrite | 17512 |
| auto.read | 17395 | auto.reread | 17359 |
| auto.random_read | 15368 | auto.random_write | 14165 |
| auto.backward_read | 13549 | auto.record_rewrite | 11671 |
| auto.stride_read | 9899 | auto.fwrite | 7736 |
| auto.frewrite | 9601 | auto.fread | 5813 |
| auto.freread | 6595 | write_read.initial_writers | 14932.09 |
| write_read.rewriters | 14409.15 | write_read.readers | 15909.74 |
| write_read.re_readers | 14011.29 | random.initial_writers | 16475.96 |
| random.rewriters | 17756.47 | random.random_readers | 14813.51 |
| random.random_writers | 12720.95 | backward.initial_writers | 16234.92 |
| backward.rewriters | 12420.24 | backward.reverse_readers | 14715.40 |
| stride.initial_writers | 16874.84 | stride.rewriters | 15491.26 |
| stride.stride_readers | 12764.59 | stdio.fwriters | 34065.80 |
| stdio.freaders | 15157.02 | positional.pwrite_writers | 12099.74 |
| positional.pread_readers | 7222.52 | vector_fallback.initial_writers | 13022.66 |
| vector_fallback.rewriters | 13076.74 | | |

`vector_fallback.*` 是有效已记录门禁项，仍须严格超过；但基线已说明 iozone `-i 11/-i 12` 不可用，不能宣称它是真正 vector I/O。若以后增加真 vector case，baseline 和新实现必须用同一新 case 各跑三次，不能与上表混比。

serial log 和 cache counters 用于区分命中不足、batch 太小、dirty throttle、direct reclaim stall、重复加载，但不能替代吞吐门禁。

## 16. 风险与完成条件

1. **Frame 写接口**：当前 `try_write_at` 要求唯一引用，无法更新被只读 PTE 共享的 cache Frame。应在 xmm 增加不暴露 Rust 引用的受控 copy API，明确外部页锁/权限前置条件；不能在 xcache 放 unsafe。
2. **SMP**：当前用户地址空间按已确认 SPEC 仍是 SMP=1。设计不应阻止 SMP，但不能宣称跨 hart TLB shootdown 已完成。
3. **可写 MAP_SHARED**：conservative pin 正确但保守，大工作集会造成内存压力和重复写回。
4. **truncate observer**：弱 token 注销、VmSpace 销毁、fault generation retry 必须并发测试；cache-lock 内回调都可能死锁。
5. **ext4 durability**：raw sync no-op 限制 fsync 承诺，必须修复或明确测试仅验证交付 backing。
6. **强门禁**：33 个指标全部严格提升。整页覆盖免读、连续 batch、无全局锁 I/O、合理水位必须从 Stage 1 纳入，不能最后补。
7. **绕过路径**：启用前应搜索并审计所有 `FileNodeOps::write_at/set_len` 调用；普通文件 raw 写/截断若绕过 cache 会产生两个真相源。

完成条件不是仅编译：所有不变量有测试，现有测试未削弱，fsync/truncate/mmap 有可观察结果，无 xprocess 依赖，并通过完整三次中位数严格性能门禁。

## 17. 一手资料

### Linux

- [VFS / address_space operations（官方内核文档）](https://www.kernel.org/doc/html/latest/filesystems/vfs.html)
- [MM concepts and reclaim watermarks（官方内核文档）](https://www.kernel.org/doc/html/latest/admin-guide/mm/concepts.html)
- [filemap.c：mapping lookup/insert](https://github.com/torvalds/linux/blob/master/mm/filemap.c)
- [pagemap.h：filemap_get_folio 等 API](https://github.com/torvalds/linux/blob/master/include/linux/pagemap.h)
- [page-writeback.c：dirty thresholds/throttling](https://github.com/torvalds/linux/blob/master/mm/page-writeback.c)
- [vmscan.c：candidate isolation/reclaim](https://github.com/torvalds/linux/blob/master/mm/vmscan.c)
- [truncate.c：truncate/invalidate](https://github.com/torvalds/linux/blob/master/mm/truncate.c)
- [sync.c：fsync/fdatasync](https://github.com/torvalds/linux/blob/master/fs/sync.c)
- [memory.c：file fault/PTE](https://github.com/torvalds/linux/blob/master/mm/memory.c)

### Asterinas（固定提交 9503fbdb）

- [PageCache API、resize/flush/invalidate/writeback](https://github.com/asterinas/asterinas/blob/9503fbdb07ec6d5e8470de9956348c660261b487/kernel/core/src/vm/page_cache/mod.rs)
- [CachePage 状态、页锁/writeback bit](https://github.com/asterinas/asterinas/blob/9503fbdb07ec6d5e8470de9956348c660261b487/kernel/core/src/vm/page_cache/cache_page.rs)
- [VMO 初始化、dirty/flush/decommit](https://github.com/asterinas/asterinas/blob/9503fbdb07ec6d5e8470de9956348c660261b487/kernel/core/src/vm/page_cache/vmo/mod.rs)
- [并发/redirty/eviction 测试](https://github.com/asterinas/asterinas/blob/9503fbdb07ec6d5e8470de9956348c660261b487/kernel/core/src/vm/page_cache/tests/mod.rs)
- [ext2 file resize 与 buffered/direct I/O](https://github.com/asterinas/asterinas/blob/9503fbdb07ec6d5e8470de9956348c660261b487/kernel/core/src/fs/fs_impls/ext2/inode/file.rs)
- [ext2 data/metadata sync 顺序](https://github.com/asterinas/asterinas/blob/9503fbdb07ec6d5e8470de9956348c660261b487/kernel/core/src/fs/fs_impls/ext2/inode/sync.rs)

### StarryX 本地约束

- `.ark/specs/features/kernel/mm/redesign-mm-subsystem/SPEC.md`
- `.ark/specs/features/xtest/redesign-xtest-framework/SPEC.md`
- `docs/benchmarks/iozone-no-page-cache.md`
- `xmodules/xcache/src/lib.rs`、`xcore/xmm/src/frame.rs`
- `xmodules/xvma/src/`、`xcore/xfs/src/`、`xmodules/xvfs/src/`
- `xkernel/src/fs/fd/file.rs`、`xkernel/src/mm/uspace.rs`、`xkernel/src/syscall/mm/mmap.rs`
