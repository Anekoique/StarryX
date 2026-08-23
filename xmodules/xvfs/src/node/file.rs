use core::{any::Any, ops::Deref};

use alloc::sync::{Arc, Weak};

use lock_api::{Mutex, RawMutex};

use crate::{VfsError, VfsResult};

use super::NodeOps;

/// The cache-attachment point shared by every live alias of one file.
///
/// A filesystem hands the same slot to every node object representing one
/// file incarnation and installs a fresh slot before a recycled inode number
/// represents another file. The attachment itself is opaque here: only the
/// kernel composition layer knows its concrete type, so the VFS carries no
/// cache dependency.
pub struct CacheSlot<M> {
    attachment: Mutex<M, Option<Weak<dyn Any + Send + Sync>>>,
}

impl<M: RawMutex> CacheSlot<M> {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            attachment: Mutex::new(None),
        })
    }

    /// Returns the live attachment, if any.
    pub fn get(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.attachment.lock().as_ref().and_then(Weak::upgrade)
    }

    /// Installs `attachment` unless a live one exists, returning the winner.
    pub fn attach_if_empty(
        &self,
        attachment: Arc<dyn Any + Send + Sync>,
    ) -> Arc<dyn Any + Send + Sync> {
        let mut guard = self.attachment.lock();
        if let Some(existing) = guard.as_ref().and_then(Weak::upgrade) {
            return existing;
        }
        *guard = Some(Arc::downgrade(&attachment));
        attachment
    }

    /// Runs `f` on the live attachment, clearing the slot when `f` accepts.
    ///
    /// `f` runs under the slot lock, so no other holder can attach to or
    /// upgrade through this slot while it decides.
    pub fn detach_if(&self, f: impl FnOnce(Arc<dyn Any + Send + Sync>) -> bool) {
        let mut guard = self.attachment.lock();
        let Some(attachment) = guard.as_ref().and_then(Weak::upgrade) else {
            *guard = None;
            return;
        };
        if f(attachment) {
            *guard = None;
        }
    }
}

/// Operations specific to file nodes
///
/// This trait extends [`NodeOps`] with file-specific operations like
/// reading, writing, and resizing files.
pub trait FileNodeOps<M>: NodeOps<M> {
    /// Returns the shared attachment slot when ordinary data is page-cache
    /// coherent.
    fn cache_slot(&self) -> Option<&Arc<CacheSlot<M>>> {
        None
    }
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
