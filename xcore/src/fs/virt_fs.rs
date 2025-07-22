use core::{any::Any, time::Duration};

use alloc::borrow::ToOwned;
use alloc::{collections::btree_map::BTreeMap, string::String, sync::Arc};
use axfs_ng_vfs::{
    DeviceId, DirEntry, DirEntrySink, DirNode, DirNodeOps, FileNode, FileNodeOps, Filesystem,
    FilesystemOps, Metadata, MetadataUpdate, NodeOps, NodePermission, NodeType, Reference, StatFs,
    VfsError, VfsResult, WeakDirEntry,
    path::{DOT, DOTDOT},
};
use axsync::{Mutex, RawMutex};
use inherit_methods_macro::inherit_methods;
use slab::Slab;

use super::dummy_stat;

/// Type alias for directory maker function
pub type DirMaker =
    Arc<dyn Fn(WeakDirEntry<RawMutex>) -> Arc<dyn DirNodeOps<RawMutex>> + Send + Sync>;

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

/// Virtual directory node
pub struct VirtDir {
    node: VirtNode,
    this: WeakDirEntry<RawMutex>,
    children: Arc<BTreeMap<String, VirtNodeOps>>,
}

impl VirtDir {
    /// Create a new virtual directory
    fn new(
        node: VirtNode,
        children: Arc<BTreeMap<String, VirtNodeOps>>,
        this: WeakDirEntry<RawMutex>,
    ) -> Arc<VirtDir> {
        Arc::new(Self {
            node,
            this,
            children,
        })
    }

    /// Create a new directory builder
    pub fn builder(fs: Arc<VirtFs>) -> VirtDirBuilder {
        VirtDirBuilder::new(fs)
    }
}

#[inherit_methods(from = "self.node")]
impl NodeOps<RawMutex> for VirtDir {
    fn inode(&self) -> u64;
    fn metadata(&self) -> VfsResult<Metadata>;
    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;
    fn filesystem(&self) -> &dyn FilesystemOps<RawMutex>;
    fn sync(&self, data_only: bool) -> VfsResult<()>;
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

impl DirNodeOps<RawMutex> for VirtDir {
    fn read_dir(&self, offset: u64, sink: &mut dyn DirEntrySink) -> VfsResult<usize> {
        let this_entry = self.this.upgrade().unwrap();
        let this_dir = this_entry.as_dir()?;

        let entries = [DOT, DOTDOT]
            .into_iter()
            .chain(self.children.keys().map(String::as_str))
            .enumerate()
            .skip(offset as usize);

        let mut count = 0;
        for (i, name) in entries {
            let metadata = match name {
                DOT => this_entry.metadata()?,
                DOTDOT => this_entry
                    .parent()
                    .map_or_else(|| this_entry.metadata(), |parent| parent.metadata())?,
                _ => this_dir.lookup(name)?.metadata()?,
            };

            if !sink.accept(name, metadata.inode, metadata.node_type, i as u64 + 1) {
                break;
            }
            count += 1;
        }

        Ok(count)
    }

    fn lookup(&self, name: &str) -> VfsResult<DirEntry<RawMutex>> {
        let ops = self.children.get(name).ok_or(VfsError::ENOENT)?;
        let reference = Reference::new(self.this.upgrade(), name.to_owned());

        Ok(match ops {
            VirtNodeOps::Dir(maker) => {
                DirEntry::new_dir(|this| DirNode::new(maker(this)), reference)
            }
            VirtNodeOps::File(ops) => {
                let node_type = ops.metadata()?.node_type;
                DirEntry::new_file(FileNode::new(ops.clone()), node_type, reference)
            }
        })
    }

    fn create(
        &self,
        _name: &str,
        _node_type: NodeType,
        _permission: NodePermission,
    ) -> VfsResult<DirEntry<RawMutex>> {
        Err(VfsError::EROFS) // Read-only filesystem
    }

    fn link(&self, _name: &str, _node: &DirEntry<RawMutex>) -> VfsResult<DirEntry<RawMutex>> {
        Err(VfsError::EROFS)
    }

    fn unlink(&self, _name: &str) -> VfsResult<()> {
        Err(VfsError::EROFS)
    }

    fn rename(
        &self,
        _src_name: &str,
        _dst_dir: &DirNode<RawMutex>,
        _dst_name: &str,
    ) -> VfsResult<()> {
        Err(VfsError::EROFS)
    }
}

/// Builder for virtual directories
pub struct VirtDirBuilder {
    fs: Arc<VirtFs>,
    children: BTreeMap<String, VirtNodeOps>,
}

impl VirtDirBuilder {
    /// Create a new directory builder
    pub fn new(fs: Arc<VirtFs>) -> Self {
        Self {
            fs,
            children: BTreeMap::new(),
        }
    }

    /// Add a child entry to the directory
    pub fn add(&mut self, name: impl Into<String>, ops: impl Into<VirtNodeOps>) -> &mut Self {
        self.children.insert(name.into(), ops.into());
        self
    }

    /// Build the directory maker
    pub fn build(self) -> DirMaker {
        let children = Arc::new(self.children);
        let fs = self.fs;

        Arc::new(move |this| {
            VirtDir::new(
                VirtNode::new(
                    fs.clone(),
                    NodeType::Directory,
                    NodePermission::from_bits_truncate(0o755),
                ),
                children.clone(),
                this,
            )
        })
    }
}
