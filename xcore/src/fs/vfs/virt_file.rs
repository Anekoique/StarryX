use alloc::{
    borrow::{Cow, ToOwned},
    collections::btree_map::BTreeMap,
    string::String,
    sync::Arc,
    vec::Vec,
};
use core::{any::Any, iter};

use axfs_ng_vfs::{
    DirEntry, DirEntrySink, DirNode, DirNodeOps, FileNode, FileNodeOps, FilesystemOps, Metadata,
    MetadataUpdate, NodeOps, NodePermission, NodeType, Reference, VfsError, VfsResult,
    WeakDirEntry,
    path::{DOT, DOTDOT},
};
use axsync::RawMutex;
use inherit_methods_macro::inherit_methods;

use super::virt_fs::{VirtFs, VirtNode, VirtNodeOps};

/// Type alias for directory maker function
pub type DirMaker =
    Arc<dyn Fn(WeakDirEntry<RawMutex>) -> Arc<dyn DirNodeOps<RawMutex>> + Send + Sync>;

pub trait VirtFileOps: Send + Sync {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize>;
    fn write_at(&self, data: &[u8], offset: u64) -> VfsResult<usize>;
    fn len(&self) -> VfsResult<u64>;
}

impl<F, R> VirtFileOps for F
where
    F: Fn() -> R + Send + Sync + 'static,
    R: Into<Vec<u8>>,
{
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let data = self().into();
        if offset >= data.len() as u64 {
            return Ok(0);
        }
        let data = &data[offset as usize..];
        let read = data.len().min(buf.len());
        buf[..read].copy_from_slice(&data[..read]);
        Ok(read)
    }

    fn write_at(&self, _data: &[u8], _offset: u64) -> VfsResult<usize> {
        Err(VfsError::EBADF)
    }

    fn len(&self) -> VfsResult<u64> {
        Ok(self().into().len() as u64)
    }
}

pub trait VirtDirOps: Send + Sync {
    fn read_dir(&self) -> impl Iterator<Item = Cow<'_, str>>;
    fn lookup(&self, name: &str) -> Option<VirtNodeOps>;
}

impl VirtDirOps for () {
    fn read_dir(&self) -> impl Iterator<Item = Cow<'_, str>> {
        iter::empty()
    }

    fn lookup(&self, _name: &str) -> Option<VirtNodeOps> {
        None
    }
}

pub struct VirtFile {
    node: VirtNode,
    ops: Arc<dyn VirtFileOps>,
}
impl VirtFile {
    pub fn new(fs: Arc<VirtFs>, ops: impl VirtFileOps + 'static) -> Arc<Self> {
        let node = VirtNode::new(fs, NodeType::RegularFile, NodePermission::default());
        Arc::new(Self {
            node,
            ops: Arc::new(ops),
        })
    }

    pub fn new_symlink(fs: Arc<VirtFs>, ops: impl VirtFileOps + 'static) -> Arc<Self> {
        let node = VirtNode::new(
            fs,
            NodeType::Symlink,
            NodePermission::from_bits_truncate(0o777),
        );
        Arc::new(Self {
            node,
            ops: Arc::new(ops),
        })
    }
}

#[inherit_methods(from = "self.node")]
impl NodeOps<RawMutex> for VirtFile {
    fn inode(&self) -> u64;
    fn metadata(&self) -> VfsResult<Metadata>;
    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;
    fn filesystem(&self) -> &dyn FilesystemOps<RawMutex>;
    fn sync(&self, data_only: bool) -> VfsResult<()>;
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn len(&self) -> VfsResult<u64> {
        self.ops.len()
    }
}

impl FileNodeOps<RawMutex> for VirtFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.ops.read_at(buf, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        self.ops.write_at(buf, offset)
    }

    fn append(&self, buf: &[u8]) -> VfsResult<(usize, u64)> {
        let length = self.ops.len()?;
        let written = self.ops.write_at(buf, length)?;
        Ok((written, length + written as u64))
    }

    fn set_len(&self, _len: u64) -> VfsResult<()> {
        Err(VfsError::EACCES)
    }

    fn set_symlink(&self, _target: &str) -> VfsResult<()> {
        Err(VfsError::EACCES)
    }
}

/// Virtual directory node
pub struct VirtDir<O: VirtDirOps + 'static> {
    node: VirtNode,
    this: WeakDirEntry<RawMutex>,
    children: Arc<BTreeMap<String, VirtNodeOps>>,
    ops: Option<Arc<O>>,
}

impl<O: VirtDirOps + 'static> VirtDir<O> {
    /// Create a new virtual directory
    fn new(
        node: VirtNode,
        children: Arc<BTreeMap<String, VirtNodeOps>>,
        this: WeakDirEntry<RawMutex>,
        ops: Option<Arc<O>>,
    ) -> Arc<VirtDir<O>> {
        Arc::new(Self {
            node,
            this,
            children,
            ops,
        })
    }

    /// Create a new directory builder
    pub fn builder(fs: Arc<VirtFs>, ops: Option<Arc<O>>) -> VirtDirBuilder<O> {
        VirtDirBuilder::new(fs, ops)
    }
}

#[inherit_methods(from = "self.node")]
impl<O: VirtDirOps + 'static> NodeOps<RawMutex> for VirtDir<O> {
    fn inode(&self) -> u64;
    fn metadata(&self) -> VfsResult<Metadata>;
    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;
    fn filesystem(&self) -> &dyn FilesystemOps<RawMutex>;
    fn sync(&self, data_only: bool) -> VfsResult<()>;
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

impl<O: VirtDirOps + 'static> DirNodeOps<RawMutex> for VirtDir<O> {
    fn read_dir(&self, offset: u64, sink: &mut dyn DirEntrySink) -> VfsResult<usize> {
        let this_entry = self.this.upgrade().unwrap();
        let this_dir = this_entry.as_dir()?;

        let entries = [DOT, DOTDOT]
            .into_iter()
            .map(Cow::Borrowed)
            .chain(self.children.keys().map(String::as_str).map(Cow::Borrowed))
            .chain(
                self.ops
                    .as_ref()
                    .map(|ops| ops.read_dir())
                    .into_iter()
                    .flatten(),
            )
            .enumerate()
            .skip(offset as usize);

        let mut count = 0;
        for (i, name) in entries {
            let name_str = name.as_ref();
            let metadata = match name_str {
                DOT => this_entry.metadata()?,
                DOTDOT => this_entry
                    .parent()
                    .map_or_else(|| this_entry.metadata(), |parent| parent.metadata())?,
                _ => this_dir.lookup(name_str)?.metadata()?,
            };

            if !sink.accept(name_str, metadata.inode, metadata.node_type, i as u64 + 1) {
                break;
            }
            count += 1;
        }

        Ok(count)
    }

    fn lookup(&self, name: &str) -> VfsResult<DirEntry<RawMutex>> {
        let reference = Reference::new(self.this.upgrade(), name.to_owned());

        if let Some(ops) = self.children.get(name) {
            return Ok(match ops {
                VirtNodeOps::Dir(maker) => {
                    DirEntry::new_dir(|this| DirNode::new(maker(this)), reference)
                }
                VirtNodeOps::File(ops) => {
                    let node_type = ops.metadata()?.node_type;
                    DirEntry::new_file(FileNode::new(ops.clone()), node_type, reference)
                }
            });
        }

        if let Some(ops) = self.ops.as_ref().and_then(|ops| ops.lookup(name)) {
            return Ok(match &ops {
                VirtNodeOps::Dir(maker) => {
                    DirEntry::new_dir(|this| DirNode::new(maker(this)), reference)
                }
                VirtNodeOps::File(ops) => {
                    let node_type = ops.metadata()?.node_type;
                    DirEntry::new_file(FileNode::new(ops.clone()), node_type, reference)
                }
            });
        }

        Err(VfsError::ENOENT)
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
pub struct VirtDirBuilder<O: VirtDirOps + 'static> {
    fs: Arc<VirtFs>,
    children: BTreeMap<String, VirtNodeOps>,
    ops: Option<Arc<O>>,
}

impl<O: VirtDirOps + 'static> VirtDirBuilder<O> {
    /// Create a new directory builder
    pub fn new(fs: Arc<VirtFs>, ops: Option<Arc<O>>) -> Self {
        Self {
            fs,
            children: BTreeMap::new(),
            ops,
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
                self.ops.clone(),
            )
        })
    }
}
