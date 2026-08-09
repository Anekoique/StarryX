# 文件系统

## 整体架构

StarryX的文件系统架构可分为三层，最底层为arceos中支撑xfs运行的虚拟文件系统xvfs，在其之上是xfs，其实现了xvfs的具体文件系统实例，如ext4、vfat，最上层为X Core，其对磁盘文件和宏内核的抽象文件进行封装，通过统一的trait进行文件操作抽象。

## 虚拟文件系统

## 文件系统实例

## 缓存设计

为了弥合高速 CPU 与慢速磁盘之间的性能鸿沟，文件系统严重依赖缓存。StarryX的文件系统中有多层缓存协同工作：

1. 页缓存（PageCache）：这是最重要的缓存层，用于缓存文件内容。当进程读写文件时，实际上是与内存中的 xcache进行数据交换。如果数据在 xcache中（命中），则无需访问磁盘。如果不在（未命中），文件系统层才会从磁盘读取数据块，填充到xcache的一个页面中，然后再提供给进程。它以页（Page）为单位进行管理，与内存管理系统紧密集成。我们将PageCache解耦为单个组件，将在第七章详细介绍。
2. 目录项缓存（DentryCache）：用于缓存路径查找结果。它存储了key(父目录inode, 文件名)到inode的映射。当内核需要解析一个路径时，它首先查询 dentry 缓存。如果命中，就可以立即获得目标的inode，避免了从磁盘上反复读取目录文件来查找 dentry的开销。这对于频繁访问相同文件或目录的场景，性能提升巨大。

```rust
pub type ReferenceKey = (usize, String);// (父目录指针, 文件名)

pub struct Reference<M> {
    parent: Option<DirEntry<M>>,
    name: String,
}
impl<M> Reference<M> {
    pub fn key(&self) -> ReferenceKey {
        let address = self
            .parent
            .as_ref()
            .map_or(0, |it| Arc::as_ptr(&it.0) as usize);
        (address, self.name.clone())  // 快速哈希键
    }
}
pub struct DirNode<M> {
    ops: Arc<dyn DirNodeOps<M>>,
    cache: Mutex<M, BTreeMap<String, DirEntry<M>>>,   // Direntry Cache
    pub(crate) mountpoint: Mutex<M, Option<Arc<Mountpoint<M>>>>,
}
```

3. 块缓存（BlockCache）：这是最底层的缓存，用于缓存磁盘的物理块（Block）。它位于具体文件系统和块设备驱动之间。即使是文件系统的元数据（如 inode 表、位图）的读写，也会经过 block cache。pagecache 通常构建在 block cache 之上，或者在某些设计中，page cache 直接管理文件数据页，而block cache 专注于元数据块

```rust
pub struct SeekableDisk {
    write_buffer: Box<[u8]>,
    write_buffer_dirty: bool,
}

impl SeekableDisk {
    pub fn flush(&mut self) -> DevResukt<()> {
        if self.write_buffer_dirty {
            self.dev.write_block(self.block_id, &self.write_buffer)?;
            self.write_buffer_dirty = false;       // 延迟写入
        }
        Ok(())
    }
}
```

## 抽象文件

Unix 的“一切皆文件”哲学是操作系统设计的黄金标准，宏内核中有大量的抽象文件，比如套接字、管道等，对于C语言来说，它们通过函数指针表扩展并实现各种抽象文件，但这种方式抽象层次低、管理困难；而对于Rust语言，我们利用Rust的Trait设计了FileLike trait，实现了对文件的高级抽象，通过FileLike trait我们具体实现了包括普通文件、普通目录、套接字、文件系统通知、进程文件描述符等丰富的抽象文件。

FileLike Trait通过一系列操作高度抽象了文件的各种行为：

```rust
pub trait FileLike: Send + Sync {
    fn read()..                 // 文件读
    fn write()..				// 文件
    fn stat()..					// 文件属性
    fn into_any()..				// 变为任意对象
    fn poll()..					// 轮询
    fn set_nonblocking()..		// 设置阻塞
    fn is_nonblocking()..		// 是否阻塞
    fn from_fd()..				// 从fd构造
    fn add_to_fd_table()..		// 加入fd_table
    fn get_location()..			// 得到位置
    fn len()..					// 得到长度
}
```

每个抽象文件打开时可能会有权限字段，我们将相关字段FileFlags与FileLike对象实体进行封装形成StarryX的抽象文件对象XFile，它继承了FileLike的方法，同时可以在进行操作前进行权限检查：

```rust
pub struct XFile {
    pub file: Arc<dyn FileLike>,
    pub flags: FileFlags,
}
impl XFile {
	// 权限检查
    pub fn validate(
        &self,
        required: FileFlags,
        forbidden: FileFlags,
    ) -> LinuxResult<&Arc<dyn FileLike>> {
        if self.flags.contains(required) && !self.flags.intersects(forbidden) {
            Ok(&self.file)
        } else {
            Err(LinuxError::EBADF)
        }
    }
}

// 方法继承
#[inherit_methods(from = "self.file")]
impl XFile {
    pub fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        self.validate(FileFlags::READ, FileFlags::PATH)?.read(buf)
    }
    pub fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        self.validate(FileFlags::WRITE, FileFlags::PATH)?.write(buf)
    }
    pub fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self.file.clone().into_any()
    }
    pub fn stat(&self) -> LinuxResult<Kstat>;
    pub fn poll(&self) -> LinuxResult<PollState>;
    pub fn set_nonblocking(&self, nonblocking: bool);
    pub fn is_nonblocking(&self) -> bool;
    pub fn get_location(&self) -> Option<Location<RawMutex>>;
    pub fn len(&self) -> LinuxResult<u64>;
}
```

对于进程的文件描述符，我们通过一个文件描述符表进行维护，同时该表属于进程命名空间，在clone等行为发生时可能发生复制。我们学习了与Linux类似的文件描述符表与Close-on-exe的设置：

```rust
// 文件描述符表
pub struct FdTable {
    inner: RwLock<FlattenObjects<Arc<XFile>, FD_LIMIT>>,
    flags: RwLock<Bitmap<FD_LIMIT>>,
}
```

通过文件描述符表将文件描述符与抽象文件XFile正式建立了联系，当执行具体文件相关系统调用时具体执行以下几个阶段：

1. 从文件描述符表取得抽象文件
2. 判断是否具有访问权限
3. 该抽象文件是否为要求的目标文件
4. 执行具体操作

以pread为例：

1. from_fd 执行了前三个操作
2. 得到具体文件实体后执行read_at

```rust
pub fn sys_pread64(
    fd: c_int,
    buf: UserPtr<u8>,
    len: usize,
    offset: __kernel_off_t,
) -> LinuxResult<isize> {
    File::from_fd(fd, FileFlags::read, FileFlags::PATH)? 
        .read_at(buf, offset as _)
        .map(|read| read as isize)
}
```

## 伪文件系统

伪文件系统（pseudo filesystem）是操作系统内核中一种**不直接对应真实存储设备**的文件系统,和 ext4、xfs、fat 这类“真实文件系统”不同，伪文件系统里的数据并不是来自磁盘，而是内核自己生成或维护的。它们主要有以下的作用：

1. **抽象统一**
    让应用程序通过“文件”接口来访问各种内核功能，避免专门系统调用。

2. **内核与用户空间的桥梁**
 内核可以通过伪文件系统向用户空间导出信息，用户可以通过读写伪文件来配置内核。

3. **增强灵活性**
    支持诸如进程管理、设备管理、调试监控等高级功能，而不依赖具体的硬件存储。

StarryX实现了较为完善的伪文件系统，包括 `/dev`、`/tmp`、`/proc` 和 `/etc`。相关实现位于 `xkernel::fs::pseudofs`，它们通过统一的Virt_file和Virt_fs抽象实现了xvfs的接口（FileNode等），成为名义上的”文件系统实体“。

对于伪文件系统的具体实现，我们首先实现了结构体VirtFs，其将inode通过Slab进行管理，并实现了具体的FilesystemOps与磁盘文件系统实例相对应：

```rust
/// Virtual filesystem implementation
pub struct VirtFs {
    name: String,
    fs_type: u32,
    inodes: Mutex<Slab<()>>,
    root: Mutex<Option<DirEntry<RawMutex>>>,
}

impl FilesystemOps<RawMutex> for VirtFs {
    fn name(&self) -> &str {
        &self.name
    }

    fn root_dir(&self) -> DirEntry<RawMutex> {
        self.root.lock().clone().unwrap()
    }

    fn stat(&self) -> VfsResult<StatFs> {
        Ok(dummy_stat(self.fs_type))
    }
}
```

对于伪文件系统中的虚拟文件，StarryX实现了统一的VirtNode抽象，并实现了vfs的NodeOps与磁盘文件相统一：

```rust
/// Virtual filesystem node
pub struct VirtNode {
    fs: Arc<VirtFs>,
    ino: u64,
    pub(crate) metadata: Mutex<Metadata>,
}
impl NodeOps<RawMutex> for VirtNode {
    fn inode()..
    fn metadata()..
    fn len()..
    fn update_metadata()..
    fn filesystem()..
    fn sync()..
    fn into_any()..
}
```

在VirtNode之上我们封装了不同伪文件系统文件的具体实现，比如VirtFile和VirtDevice，它们与实际磁盘的inode相统一：

```rust
// /dev文件
pub struct VirtDevice {
    node: VirtNode,
    ops: Arc<dyn VirtDeviceOps>,
}

pub trait VirtDeviceOps: Send + Sync {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize>;
    fn ioctl(&self, op: usize, argp: UserPtr<c_void>) -> VfsResult<isize>;
}

#[inherit_methods(from = "self.node")]
impl NodeOps<RawMutex> for VirtDevice {
    fn inode()...
    fn metadata()...
    fn update_metadata()..
    fn filesystem()...
    fn sync()...
    fn into_any()...
    fn len()...
}

impl FileNodeOps<RawMutex> for VirtDevice {
    fn read_at() ...
    fn write_at()...
    fn append()...
    fn set_len()...
    fn set_symlink()...
}
```

```rust
// /proc文件
pub struct VirtFile {
    node: VirtNode,
    ops: Arc<dyn VirtFileOps>,
}

pub trait VirtFileOps: Send + Sync {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    fn write_at(&self, data: &[u8], offset: u64) -> VfsResult<usize>;
    fn len(&self) -> VfsResult<u64>;
}

#[inherit_methods(from = "self.node")]
impl NodeOps<RawMutex> for VirtFile {
    fn inode()...
    fn metadata()...
    fn update_metadata()..
    fn filesystem()...
    fn sync()...
    fn into_any()...
    fn len()...
}

impl FileNodeOps<RawMutex> for VirtFile {
    fn read_at() ...
    fn write_at()...
    fn append()...
    fn set_len()...
    fn set_symlink()...
}

```
