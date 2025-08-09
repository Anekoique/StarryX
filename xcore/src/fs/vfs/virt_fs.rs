use alloc::{string::String, sync::Arc};
use core::{any::Any, ffi::c_void, time::Duration};

use axfs_ng_vfs::{
    DeviceId, DirEntry, DirNode, FileNodeOps, Filesystem, FilesystemOps, Metadata, MetadataUpdate,
    NodeOps, NodePermission, NodeType, Reference, StatFs, VfsError, VfsResult, path::MAX_NAME_LEN,
};
use axsync::{Mutex, RawMutex};
use inherit_methods_macro::inherit_methods;

use xuspace::UserPtr;
use xutils::collections::slab::Slab;

use super::virt_file::DirMaker;

/// Create a dummy statfs for virtual filesystems
pub(crate) fn dummy_stat(fs_type: u32) -> StatFs {
    StatFs {
        fs_type,
        block_size: 4096,
        blocks: 0,
        blocks_free: 0,
        blocks_available: 0,
        file_count: 0,
        free_file_count: 0,
        name_length: MAX_NAME_LEN as _,
        fragment_size: 0,
        mount_flags: 0,
    }
}

/// Virtual filesystem implementation
pub struct VirtFs {
    name: String,
    fs_type: u32,
    inodes: Mutex<Slab<()>>,
    root: Mutex<Option<DirEntry<RawMutex>>>,
}

impl VirtFs {
    /// Create a new virtual filesystem with a custom root builder
    pub fn new_with(
        name: String,
        fs_type: u32,
        root_builder: impl FnOnce(Arc<VirtFs>) -> DirMaker,
    ) -> Filesystem<RawMutex> {
        let fs = Arc::new(Self {
            name,
            fs_type,
            inodes: Mutex::default(),
            root: Mutex::default(),
        });

        let root_maker = root_builder(fs.clone());
        fs.set_root(DirEntry::new_dir(
            |this| DirNode::new(root_maker(this)),
            Reference::root(),
        ));

        Filesystem::new(fs)
    }

    /// Set the root directory entry
    pub fn set_root(&self, root: DirEntry<RawMutex>) {
        *self.root.lock() = Some(root);
    }

    /// Allocate a new inode number
    pub fn alloc_inode(&self) -> u64 {
        self.inodes.lock().insert(()) as u64 + 1
    }

    /// Release an inode number
    pub fn release_inode(&self, ino: u64) {
        self.inodes.lock().remove(ino as usize - 1);
    }
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

/// Node operations for virtual filesystem entries
pub enum VirtNodeOps {
    Dir(DirMaker),
    File(Arc<dyn FileNodeOps<RawMutex>>),
}

impl From<DirMaker> for VirtNodeOps {
    fn from(maker: DirMaker) -> Self {
        Self::Dir(maker)
    }
}

impl<T: FileNodeOps<RawMutex> + 'static> From<Arc<T>> for VirtNodeOps {
    fn from(ops: Arc<T>) -> Self {
        Self::File(ops)
    }
}

/// Virtual filesystem node
pub struct VirtNode {
    fs: Arc<VirtFs>,
    ino: u64,
    pub(crate) metadata: Mutex<Metadata>,
}

impl VirtNode {
    /// Create a new virtual node
    pub fn new(fs: Arc<VirtFs>, node_type: NodeType, mode: NodePermission) -> Self {
        let ino = fs.alloc_inode();
        let metadata = Metadata {
            device: 0,
            inode: ino,
            nlink: 1,
            mode,
            node_type,
            uid: 0,
            gid: 0,
            size: 0,
            block_size: 4096,
            blocks: 0,
            rdev: DeviceId::default(),
            atime: Duration::default(),
            mtime: Duration::default(),
            ctime: Duration::default(),
        };

        Self {
            fs,
            ino,
            metadata: Mutex::new(metadata),
        }
    }
}

impl Drop for VirtNode {
    fn drop(&mut self) {
        self.fs.release_inode(self.ino);
    }
}

impl NodeOps<RawMutex> for VirtNode {
    fn inode(&self) -> u64 {
        self.ino
    }

    fn metadata(&self) -> VfsResult<Metadata> {
        let mut metadata = self.metadata.lock().clone();
        metadata.size = self.len()?;
        Ok(metadata)
    }

    fn len(&self) -> VfsResult<u64> {
        Ok(0)
    }

    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()> {
        let mut metadata = self.metadata.lock();

        if let Some(mode) = update.mode {
            metadata.mode = mode;
        }
        if let Some((uid, gid)) = update.owner {
            metadata.uid = uid;
            metadata.gid = gid;
        }
        if let Some(atime) = update.atime {
            metadata.atime = atime;
        }
        if let Some(mtime) = update.mtime {
            metadata.mtime = mtime;
        }

        Ok(())
    }

    fn filesystem(&self) -> &dyn FilesystemOps<RawMutex> {
        self.fs.as_ref()
    }

    fn sync(&self, _data_only: bool) -> VfsResult<()> {
        Ok(())
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

pub trait VirtDeviceOps: Send + Sync {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize>;
    fn as_any(&self) -> &dyn Any;
    fn ioctl(&self, op: usize, argp: UserPtr<c_void>) -> VfsResult<isize>;
}

impl<F> VirtDeviceOps for F
where
    F: Fn(&mut [u8], u64) -> VfsResult<usize> + Send + Sync + 'static,
{
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        (self)(buf, offset)
    }

    fn write_at(&self, _buf: &[u8], _offset: u64) -> VfsResult<usize> {
        Err(VfsError::EBADF)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn ioctl(&self, _op: usize, _argp: UserPtr<c_void>) -> VfsResult<isize> {
        Ok(0)
    }
}

pub struct VirtDevice {
    node: VirtNode,
    ops: Arc<dyn VirtDeviceOps>,
}
impl VirtDevice {
    pub fn new(
        fs: Arc<VirtFs>,
        node_type: NodeType,
        device_id: DeviceId,
        ops: impl VirtDeviceOps + 'static,
    ) -> Arc<Self> {
        let node = VirtNode::new(fs, node_type, NodePermission::default());
        node.metadata.lock().rdev = device_id;
        Arc::new(Self {
            node,
            ops: Arc::new(ops),
        })
    }

    /// Returns the inner device operations.
    pub fn inner(&self) -> &Arc<dyn VirtDeviceOps> {
        &self.ops
    }
}

#[inherit_methods(from = "self.node")]
impl NodeOps<RawMutex> for VirtDevice {
    fn inode(&self) -> u64;
    fn metadata(&self) -> VfsResult<Metadata>;
    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;
    fn filesystem(&self) -> &dyn FilesystemOps<RawMutex>;
    fn sync(&self, data_only: bool) -> VfsResult<()>;
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn len(&self) -> VfsResult<u64> {
        Ok(0)
    }
}

impl FileNodeOps<RawMutex> for VirtDevice {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.ops.read_at(buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        self.ops.write_at(buf, offset)
    }

    fn append(&self, _buf: &[u8]) -> VfsResult<(usize, u64)> {
        Err(VfsError::ENOTTY)
    }

    fn set_len(&self, _len: u64) -> VfsResult<()> {
        Err(VfsError::ENOTTY)
    }

    fn set_symlink(&self, _target: &str) -> VfsResult<()> {
        Err(VfsError::ENOTTY)
    }
}
