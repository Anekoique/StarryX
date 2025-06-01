use core::ops::Deref;

use alloc::sync::Arc;

use crate::{VfsError, VfsResult};

use super::NodeOps;

/// Operations specific to file nodes
///
/// This trait extends [`NodeOps`] with file-specific operations like
/// reading, writing, and resizing files.
pub trait FileNodeOps<M>: NodeOps<M> {
    /// Reads a number of bytes starting from a given offset.
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize>;

    /// Writes a number of bytes starting from a given offset.
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize>;

    /// Appends data to the file.
    ///
    /// Returns `(written, offset)` where `written` is the number of bytes
    /// written and `offset` is the new file size.
    fn append(&self, buf: &[u8]) -> VfsResult<(usize, u64)>;

    /// Sets the size of the file.
    fn set_len(&self, len: u64) -> VfsResult<()>;

    /// Sets the file's symlink target.
    fn set_symlink(&self, target: &str) -> VfsResult<()>;
}

/// A wrapper for file node operations
///
/// This struct provides a type-safe interface for working with file nodes
/// while hiding the implementation details behind a trait object.
#[repr(transparent)]
pub struct FileNode<M>(Arc<dyn FileNodeOps<M>>);

impl<M> Deref for FileNode<M> {
    type Target = dyn FileNodeOps<M>;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<M> From<FileNode<M>> for Arc<dyn NodeOps<M>> {
    fn from(node: FileNode<M>) -> Self {
        node.0.clone()
    }
}

impl<M> FileNode<M> {
    /// Creates a new file node from operations
    pub fn new(ops: Arc<dyn FileNodeOps<M>>) -> Self {
        Self(ops)
    }

    /// Returns a reference to the inner operations
    pub fn inner(&self) -> &Arc<dyn FileNodeOps<M>> {
        &self.0
    }

    /// Attempts to downcast to a specific file implementation type
    pub fn downcast<T: Send + Sync + 'static>(self: &Arc<Self>) -> VfsResult<Arc<T>> {
        self.0
            .clone()
            .into_any()
            .downcast()
            .map_err(|_| VfsError::EINVAL)
    }
}
