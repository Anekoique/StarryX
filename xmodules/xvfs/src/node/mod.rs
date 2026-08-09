mod dir;
mod file;

use core::{iter, ops::Deref};

pub use dir::*;
pub use file::*;

use alloc::{
    borrow::ToOwned,
    string::String,
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};
use inherit_methods_macro::inherit_methods;
use lock_api::RawMutex;

use crate::{
    FilesystemOps, Metadata, MetadataUpdate, NodeType, VfsError, VfsResult, path::PathBuf,
};

/// Filesystem node operations
///
/// This trait defines the operations that can be performed on any filesystem node,
/// whether it's a file, directory, or other special node type.
pub trait NodeOps<M>: Send + Sync {
    /// Gets the inode number of the node.
    fn inode(&self) -> u64;

    /// Gets the metadata of the node.
    fn metadata(&self) -> VfsResult<Metadata>;

    /// Updates the metadata of the node.
    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;

    /// Gets the filesystem
    fn filesystem(&self) -> &dyn FilesystemOps<M>;

    /// Gets the size of the node.
    fn len(&self) -> VfsResult<u64> {
        self.metadata().map(|m| m.size)
    }

    /// Returns whether the node is empty (has zero length).
    fn is_empty(&self) -> VfsResult<bool> {
        self.len().map(|len| len == 0)
    }

    /// Synchronizes the file to disk.
    fn sync(&self, data_only: bool) -> VfsResult<()>;

    /// Casts the node to a `&dyn core::any::Any`.
    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync>;
}

/// Internal node representation that can be either a file or directory
pub enum Node<M> {
    File(FileNode<M>),
    Dir(DirNode<M>),
}
impl<M: RawMutex> Node<M> {
    pub fn clone_inner(&self) -> Arc<dyn NodeOps<M>> {
        match self {
            Node::File(file) => file.inner().clone(),
            Node::Dir(dir) => dir.inner().clone(),
        }
    }
}
impl<M> Deref for Node<M> {
    type Target = dyn NodeOps<M>;

    fn deref(&self) -> &Self::Target {
        match &self {
            Node::File(file) => file.deref(),
            Node::Dir(dir) => dir.deref(),
        }
    }
}

/// Reference key type for tracking node relationships
pub type ReferenceKey = (usize, String);

/// A reference to a parent directory and name within that directory
pub struct Reference<M> {
    parent: Option<DirEntry<M>>,
    name: String,
}
impl<M> Reference<M> {
    /// Creates a new reference
    pub fn new(parent: Option<DirEntry<M>>, name: String) -> Self {
        Self { parent, name }
    }

    /// Creates a root reference (no parent)
    pub fn root() -> Self {
        Self::new(None, String::new())
    }

    /// Returns a unique key for this reference
    pub fn key(&self) -> ReferenceKey {
        let address = self
            .parent
            .as_ref()
            .map_or(0, |it| Arc::as_ptr(&it.0) as usize);
        (address, self.name.clone())
    }
}

/// Internal structure holding node data and metadata
struct Inner<M> {
    node: Node<M>,
    node_type: NodeType,
    reference: Reference<M>,
}

/// A directory entry that can represent either a file or directory
///
/// This is the primary abstraction for filesystem nodes in the VFS.
/// It contains both the node implementation and metadata about the node's
/// location in the filesystem hierarchy.
pub struct DirEntry<M>(Arc<Inner<M>>);
impl<M> Clone for DirEntry<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// A weak reference to a directory entry
pub struct WeakDirEntry<M>(Weak<Inner<M>>);
impl<M> Clone for WeakDirEntry<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<M> WeakDirEntry<M> {
    /// Attempts to upgrade this weak reference to a strong reference
    pub fn upgrade(&self) -> Option<DirEntry<M>> {
        self.0.upgrade().map(DirEntry)
    }
}

impl<M> From<Node<M>> for Arc<dyn NodeOps<M>> {
    fn from(node: Node<M>) -> Self {
        match node {
            Node::File(file) => file.into(),
            Node::Dir(dir) => dir.into(),
        }
    }
}

#[inherit_methods(from = "self.0.node")]
impl<M: RawMutex> DirEntry<M> {
    /// Returns the inode number of this entry
    pub fn inode(&self) -> u64;
    /// Returns the filesystem operations for this entry
    pub fn filesystem(&self) -> &dyn FilesystemOps<M>;
    /// Updates the metadata of this entry
    pub fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;
    /// Returns the size of this entry in bytes
    pub fn len(&self) -> VfsResult<u64>;
    /// Syncs this entry to persistent storage
    pub fn sync(&self, data_only: bool) -> VfsResult<()>;

    /// Returns whether the entry is empty (has zero length).
    pub fn is_empty(&self) -> VfsResult<bool> {
        self.len().map(|len| len == 0)
    }
}

impl<M: RawMutex> DirEntry<M> {
    /// Creates a new file entry
    pub fn new_file(node: FileNode<M>, node_type: NodeType, reference: Reference<M>) -> Self {
        Self(Arc::new(Inner {
            node: Node::File(node),
            node_type,
            reference,
        }))
    }

    /// Creates a new directory entry
    pub fn new_dir(
        node_fn: impl FnOnce(WeakDirEntry<M>) -> DirNode<M>,
        reference: Reference<M>,
    ) -> Self {
        Self(Arc::new_cyclic(|this| Inner {
            node: Node::Dir(node_fn(WeakDirEntry(this.clone()))),
            node_type: NodeType::Directory,
            reference,
        }))
    }

    /// Returns the metadata for this entry, including the correct node type
    pub fn metadata(&self) -> VfsResult<Metadata> {
        self.0.node.metadata().map(|mut metadata| {
            metadata.node_type = self.0.node_type;
            metadata
        })
    }

    /// Attempts to downcast the underlying node to a specific type
    pub fn downcast<T: Send + Sync + 'static>(&self) -> VfsResult<Arc<T>> {
        self.0
            .node
            .clone_inner()
            .into_any()
            .downcast()
            .map_err(|_| VfsError::EINVAL)
    }

    /// Creates a weak reference to this entry
    pub fn downgrade(&self) -> WeakDirEntry<M> {
        WeakDirEntry(Arc::downgrade(&self.0))
    }

    /// Returns a unique key for this entry based on its reference
    pub fn key(&self) -> ReferenceKey {
        self.0.reference.key()
    }

    /// Returns the node type of this entry
    pub fn node_type(&self) -> NodeType {
        self.0.node_type
    }

    /// Returns the parent directory entry, if any
    pub fn parent(&self) -> Option<Self> {
        self.0.reference.parent.clone()
    }

    /// Returns the name of this entry
    pub fn name(&self) -> &str {
        &self.0.reference.name
    }

    /// Checks if the entry is a root of a mount point.
    pub fn is_root_of_mount(&self) -> bool {
        self.0.reference.parent.is_none()
    }

    /// Checks if this entry is an ancestor of another entry
    pub fn is_ancestor_of(&self, other: &Self) -> VfsResult<bool> {
        let mut current = other.clone();
        loop {
            if current.ptr_eq(self) {
                return Ok(true);
            }
            if let Some(parent) = current.parent() {
                current = parent;
            } else {
                break;
            }
        }
        Ok(false)
    }

    /// Collects the path components for this entry into a vector
    pub(crate) fn collect_absolute_path(&self, components: &mut Vec<String>) {
        let mut current = self.clone();
        loop {
            components.push(current.name().to_owned());
            if let Some(parent) = current.parent() {
                current = parent;
            } else {
                break;
            }
        }
    }

    /// Returns the absolute path to this entry
    pub fn absolute_path(&self) -> VfsResult<PathBuf> {
        let mut components = vec![];
        self.collect_absolute_path(&mut components);
        Ok(iter::once("/")
            .chain(components.iter().map(String::as_str).rev())
            .collect())
    }

    /// Returns true if this entry represents a file
    pub fn is_file(&self) -> bool {
        matches!(self.0.node, Node::File(_))
    }

    /// Returns true if this entry represents a directory
    pub fn is_dir(&self) -> bool {
        matches!(self.0.node, Node::Dir(_))
    }

    /// Returns a reference to the file node, or an error if this is not a file
    pub fn as_file(&self) -> VfsResult<&FileNode<M>> {
        match &self.0.node {
            Node::File(file) => Ok(file),
            _ => Err(VfsError::EISDIR),
        }
    }

    /// Returns a reference to the directory node, or an error if this is not a directory
    pub fn as_dir(&self) -> VfsResult<&DirNode<M>> {
        match &self.0.node {
            Node::Dir(dir) => Ok(dir),
            _ => Err(VfsError::ENOTDIR),
        }
    }

    /// Checks if this entry points to the same object as another entry
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Reads the target of a symbolic link
    pub fn read_link(&self) -> VfsResult<String> {
        if self.node_type() != NodeType::Symlink {
            return Err(VfsError::EINVAL);
        }
        let file = self.as_file()?;
        let mut buf = vec![0; file.len()? as usize];
        file.read_at(&mut buf, 0)?;
        String::from_utf8(buf).map_err(|_| VfsError::EINVAL)
    }

    pub fn node(&self) -> Weak<dyn NodeOps<M>> {
        Arc::downgrade(&self.0.node.clone_inner())
    }

    /// Get the file node
    pub fn get_file_node(&self) -> Arc<dyn FileNodeOps<M>> {
        match &self.0.node {
            Node::File(file) => file.inner().clone(),
            _ => panic!("not a file node"),
        }
    }
}
