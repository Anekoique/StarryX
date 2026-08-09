# 内存管理

![XMM 结构](./images/xmm.png)

## 整体架构

StarryX的内存管理模块主要由内存分配模块和虚拟内存管理模块组成，它们的内核基础功能分别由arceos的xalloc模块和xmm模块提供。考虑到系统性能需求，StarryX使用单一页表架构，内核与用户共享地址空间，无需频繁切换根页表产生开销。对于内存分配模块，xalloc实现了一个全局内存分配器，支持多种内存分配算法，并提供api实现灵活内存分配；对于虚拟内存管理模块，xmm实现了`AddrSpace`管理任务地址空间，宏内核在其基础上扩展了进程地址空间。另外我们实现了写时复制、内存懒分配等高级机制，将用户空间访问解耦为组件提供复用。

## 内存分配

内存分配的主要逻辑在xalloc模块中实现，其核心为`GlobalAllocator`结构体：

```rust
pub struct GlobalAllocator {
    balloc: SpinNoIrq<DefaultByteAllocator>,
    palloc: SpinNoIrq<BitmapPageAllocator<PAGE_SIZE>>,
}
```

GlobalAllocator由两部分组成，palloc使用位图分配器管理物理页的分配，而balloc提供了字节范围的分配接口，当用户申请内存分配时，先分配balloc分配器的内存，若balloc分配器内存不足，则从palloc分配器分配内存。其中balloc实现了多种内存分配算法，包括Buddy、Slab和Tlsf，可以通过feature自由选择：

```rust
cfg_if::cfg_if! {
    if #[cfg(feature = "slab")] {
        /// The default byte allocator.
        pub type DefaultByteAllocator = allocator::SlabByteAllocator;
    } else if #[cfg(feature = "buddy")] {
        /// The default byte allocator.
        pub type DefaultByteAllocator = allocator::BuddyByteAllocator;
    } else if #[cfg(feature = "tlsf")] {
        /// The default byte allocator.
        pub type DefaultByteAllocator = allocator::TlsfByteAllocator;
    }
}
```

xalloc为内核提供内存分配的接口的同时，StarryX为该模块新添加了内存回收接口，通过使用crate_interface组件解决循环依赖问题，当内核内存不足时，可以回收别的内核功能实现的内存，比如页缓存：

```rust
#[crate_interface::def_interface]
pub trait XAllocIf {
    fn evict_cache(num_pages: usize) -> AllocResult;
}

```

## 地址空间管理

StarryX的地址空间通过`xmm`模块的`AddrSpace`结构体管理，其包括三个字段，va_range管理虚拟地址范围、areas管理具体的内存区域、pt为该地址空间下的虚拟页表：

```rust
/// The virtual memory address space.
pub struct AddrSpace {
    va_range: VirtAddrRange,
    areas: MemorySet<Backend>,
    pt: PageTable,
}
```

其中areas为内存区域集合MemorySet，MemorySet通过B树管理该地址空间的内存区域

```rust
pub struct MemorySet<B: MappingBackend> {
    areas: BTreeMap<B::Addr, MemoryArea<B>>,
}
```

对于每个内存区域，有特定的映射方式，每个特定映射都需要实现以下接口：

```rust
pub trait MappingBackend {
    /// What to do when mapping a region within the area with the given flags.
    fn map(
        &self,
        start: Self::Addr,
        size: usize,
        flags: Self::Flags,
        page_table: &mut Self::PageTable,
    ) -> bool;

    /// What to do when unmaping a memory region within the area.
    fn unmap(&self, start: Self::Addr, size: usize, page_table: &mut Self::PageTable) -> bool;

    /// What to do when changing access flags.
    fn protect(
        &self,
        start: Self::Addr,
        size: usize,
        new_flags: Self::Flags,
        page_table: &mut Self::PageTable,
    ) -> bool;
}
```

xmm一共实现了三种映射方式，每种映射方式分别应用于不同场景：

1. 线性映射（linear）：线性映射直接将虚拟内存按照偏移线性映射到特定的物理内存范围，线性映射一般常用于映射内核代码段、数据段以及设备MMIO区域

2. 动态映射（alloc）：alloc映射动态分配物理内存，通过alloc可以实现写时复制和懒分配机制，只有当程序访问该VMA内的某个地址并触发缺页异常时，才动态分配一个物理页帧并建立映射。这是实现按需分页的基础，常用于进程的堆和栈。

3. 共享映射（shared）：可用于实现进程间的共享内存(MAP_SHARED)和System V共享内存机制。它在创建时就分配好全部所需的物理页，并由一个共享对象 (SharedPages) 持有。其他进程映射同一块共享内存时，会复用这些已分配的物理页，从而实现对同一物理内存的并发读写。

## 延迟分配技术

延迟分配技术是操作系统重要的内存技术，其核心思想是不提前分配物理内存，而是等程序真正访问（触发缺页异常）时，再去分配并建立映射，StarryX在xmm中扩展实现了懒分配和写时复制两种重要机制，极大减少了进程复制时的内存复制开销。

### 懒分配

我们在内存映射（mmap）、程序间断点（brk）、用户栈（stack）的分配过程中使用了懒分配技术，其实现核心是内存区域的alloc映射，alloc映射内部维护一个关键元数据populate，其可以通过调用者在创建内存区域时自由指定

```rust
pub enum {
    Alloc {
        /// Whether to populate the physical frames when creating the mapping.
        populate: bool,
    },
}
```

对于populate为true的情况，直接分配物理内存并建立分页映射；对于populate为false的情况将延迟分配内存，由于未在页表建立映射，当用户读取对应页面的数据时将触发缺页异常，从而使得访存行为被内存捕获，此时会执行页面错误处理程序，当从MemorySet中找到对应内存范围后执行特定函数，检查populate元数据并执行分配和映射：

```rust
pub(crate) fn handle_page_fault_alloc(
    vaddr: VirtAddr,
    orig_flags: MappingFlags,
    pt: &mut PageTable,
    populate: bool,
    align: PageSize,
) -> bool {
    if populate {
        false
    } else if let Some(frame) = alloc_frame(true, align) {
        pt.map(vaddr, frame, PageSize::Size4K, orig_flags)
            .map(|tlb| tlb.flush())
            .is_ok()
    } else {
        false
    }
}
```

特别地，mmap除了会建立直接的内存映射，还会创建文件映射（MAP_FILE)，对于这一类映射，将会在内存管理模块xmm和文件系统xfs建立练习，造成模块耦合且在arceos引入了宏内核内容，为了避免这种情况我们实现了模块解耦的虚拟内存管理模块（xvma），通过其我们实现了文件页的懒分配，有效解决了这一种情况，我们将在第七章深入介绍。

### 写时复制

宏内核的以clone()为代表的进程复制操作中，在创建新的子进程时需要将原有的内存信息完整地拷贝一份，这一行为通常会耗费大量的内存空间，为内核带来巨大的内存开销负担，为了尽量避免进程复制时的内存开销，我们引入了写时复制（copy-on-write）技术。

对于COW的实现，我们在全局维护一张物理页帧表(Frame Table), 每一个物理页帧对应一个FrameInfo结构体，FrameInfo结构体内部维护一个引用计数，当StarryX处理clone时会调用xmm的try_clone()，try_clone()将会对被复制的每一个内存区域进行处理，去除写标志位并对对应的Frameinfo增加引用计数，只有Frameinfo的引用计数为0时才会被真正释放。当用户真正写入时由于该页表项未设置W位，该操作将被操作系统捕获，进行实际的页面复制行为。

```rust
pub(crate) struct FrameRefTable {
    data: Box<[FrameInfo; MAX_FRAME_NUM]>,
}
pub(crate) struct FrameInfo {
    ref_count: AtomicUsize,
}
```

```rust
#[cfg(feature = "cow")]
fn handle_cow_fault(
    vaddr: VirtAddr,
    paddr: PhysAddr,
    flags: MappingFlags,
    align: PageSize,
    pt: &mut PageTable,
) -> bool {
    match frame_table().ref_count(paddr) {
        0 => unreachable!(),
        // There is only one AddrSpace reference to the page,
        // so there is no need to copy it.
        1 => pt.protect(vaddr, flags).map(|(_, tlb)| tlb.flush()).is_ok(),
        // Allocates the new page and copies the contents of the original page,
        // remapping the virtual address to the physical address of the new page.
        2.. => match alloc_frame(false, align) {
            Some(new_frame) => {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        phys_to_virt(paddr).as_ptr(),
                        phys_to_virt(new_frame).as_mut_ptr(),
                        align.into(),
                    )
                };

                dealloc_frame(paddr, align);

                pt.remap(vaddr, new_frame, flags)
                    .map(|(_, tlb)| {
                        tlb.flush();
                    })
                    .is_ok()
            }
            None => false,
        },
    }
}
```

## 用户地址访问

`xuspace`组件是StarryX中负责用户地址空间访问的核心模块，它为内核提供了安全、统一的用户空间内存访问接口，其封装了用户态地址访问的复杂性，确保内核在访问用户空间数据时的安全性和正确性。

在设计之初，`xuspace`与内核服务和系统调用实现紧密耦合，由于初期时内存相关机制尚未实现，解耦于StarryX的组件（比如xsignal）可以将用户指针转化为裸指针进行访问，但是这样的访问存在许多问题：

- 无法判断其合法性，无法安全访问用户地址
- 引入了大量对裸指针的unsafe操作
- 实现cow后可能导致在内核态发生缺页异常而发生致命错误

当实现内存延迟分配机制后，内核外组件无法再直接访问用户地址空间，因此我们将`xuspace`从内核实现中解耦成为一个独立组件，并提供抽象接口使其可以被其他系统所复用。我们将在第七章详细介绍这一组件。
