## 内存映射管理

`xvma`组件是StarryX中专门处理文件支持的虚拟内存区域(mmap分配)管理模块，它实现了高效的按需加载内存映射机制。该组件专注于文件映射场景，提供精确的地址范围管理和智能的页面加载策略。

在原本arceos的内存管理设计中xmm模块已经实现了地址空间`memory_set`的管理，POSIX标准下mmap映射文件会引入虚拟内存区域与文件相关联的操作，这与页缓存一样会使基座OS独立的模块引入依赖，在与arceos的开发者交流后，我们选择将这一层功能放在宏内核实现，避免引入依赖，保持模块的低耦合。

在组件内部我们实现了文件映射区域的有效管理，并且未来可以扩展到对所有mmap区域进行管理，与PageCache等组件配合工作完整实现mmap的所有功能：

```rust
/// 文件支持的内存映射区域
pub struct MmapRegion<F: VmFile> {
    pub range: VirtAddrRange,                    // 虚拟地址范围
    pub file: F,                                 // 支持的文件对象
    pub offset: isize,                           // 文件偏移量
    pub populated: Mutex<BTreeSet<VirtAddr>>,    // 已加载页面集合
    pub align: PageSize,                         // 页面对齐大小
}

/// 虚拟内存区域管理器
pub struct VmaManager<F: VmFile> {
    regions: Vec<MmapRegion<F>>,                 // 内存映射区域集合
}
```

mmap系统调用会对映射的文件页和内存区域在`VmaManager`注册虚拟内存区域，并对虚拟内存区域进行管理，完成精确的地址范围管理以及区域分割与合并，保持页面加载状态的一致性，并实现高效的地址查找和范围操作：

```rust
impl<F: VmFile> VmaManager<F> {
    add_region()...             // 添加映射区域
    find_region()...            // 查找包含地址的区域
    remove_overlapped()...      // 移除重叠区
    split_at_range()...         // 区域分割
}
```

通过`xvma`StarryX实现了文件页的延迟加载策略，支持文件数据的按需读取和缓存，支持文件数据的按需读取和缓存；在发生缺页异常时，内核会先对发生缺页异常的地址进行快速区域查找，找到则读取文件数据，未找到则再交付给底层xmm执行缺页异常处理，实现了高效的文件页懒分配机制。

目前`xvma`主要支持了mmap的文件页映射管理，未来我们希望扩展xvma更多功能，使其可以成为一个高效独立管理mmap区域的组件，通过该组件减少进程管理、内存管理和文件系统间复杂的内核耦合关系。