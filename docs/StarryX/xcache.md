## 页缓存

`xcache` 是 StarryX 早期页缓存原型，包含基于 LRU 的缓存页、脏页、回写和回收机制。当前实现已与 `xkernel` 断开，普通文件 I/O 和 file-backed mmap 均直接访问 `xfs`/VFS；以下内容描述保留组件的内部结构，不代表当前运行时已启用页缓存。在组件化 OS 中重新设计页缓存需要面对以下挑战：

- 涉及多个模块，包括文件系统、内存管理和进程管理
- 原本独立的模块引入依赖，耦合度高（如文件系统引入内存管理依赖）
- 上层无法对缓存直接控制，回收困难

为了降低模块耦合度，保证底层arceos的`xfs`和`xmm`的独立性，我们将`PageCache`的实现抽象为一个独立组件，解除了多个模块的依赖关系，同时其不依赖特定的文件系统和内存管理系统实现，可以为不同类型的文件系统提供统一的缓存服务。

![page_cache](./images/pagecache.png)

`xcache`维护了一个PageCache数据结构，其与文件系统的inode相对应（类似于Linux inode与addr_space设计），其内部的LRUCache通过HashMap实现快速查找缓存页，同时基于LRU算法管理缓存页；PageCache通过trait实现泛型抽象设计，将页缓存底层操作抽象为文件系统相关的读写操作和内存管理相关的页面分配及操作：

```rust
// 页缓存结构
pub struct PageCache<N: InodeOps, P: PageOps> {
    pub host: N,                               // 文件操作接口
    pages: Mutex<LruCache<u64, CachePage>>,    // 缓存页集合（key为偏移量）
    file_size: AtomicU64,                      // 文件大小
    _marker: core::marker::PhantomData<P>,     // 页面操作接口
}

// 缓存页
pub struct CachePage {
    pub addr: PhysAddr,    // 页物理地址
    pub state: PageState,  // 页面状态
}
```

与文件系统相关的操作为缓存未命中时对底层磁盘的读写操作：

```rust
/// 文件节点操作接口
pub trait InodeOps {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> LinuxResult<usize>;
    fn write_at(&self, buf: &[u8], offset: u64) -> LinuxResult<usize>;
}
```

与内存管理相关的操作为缓存未命中时的物理页分配操作和命中时的读写操作：

```rust
/// 页面操作接口
pub trait PageOps {
    fn alloc_page() -> Option<PhysAddr>;
    fn dealloc_page(addr: PhysAddr);
    fn read_page(addr: VirtAddr, buf: &mut [u8]) -> LinuxResult;
    fn write_page(addr: VirtAddr, buf: &[u8]) -> LinuxResult;
}
```

在实现上述接口后，OS即可实现自己的PageCache管理器。

PageCache内部实现了完整的页面加载、写入操作、同步会写流程（UpToDate->Dirty->WriteBack->ToWrite），保证了页面缓存的数据数据一致性、操作安全性和性能优化：

```rust
pub enum PageState {
    UpToDate,          // 干净页面
    Dirty,             // 脏页
    WriteBack,         // 待回写 （提供一致性快照，确保同步操作的原子性）
    ToWrite,           // 写回中 （提供I/O并发控制，防止重复回写）
}
```

PageCache也提供了灵活的页面回收机制，包括局部回收，全局回收以及LRU回收，可以灵活应用于各个场景

```rust
impl PageCache {
    pub fn evict() ...          // 全局回收
    pub fn evict_range() ...    // 回收指定范围
    pub fn evict_from_pos() ... // 从指定位置回收(ftruncate)
    pub fn evict_lru() ...      // 使用LRU算法回收
}
```

该原型当前不参与 StarryX 运行时。后续实现需要围绕稳定的 VFS inode
所有权统一 buffered I/O、mmap、truncate、writeback 和回收语义，并以真实
ext4 路径上的[无页缓存 iozone 基线](../benchmarks/iozone-no-page-cache.md)
作为性能对照。
