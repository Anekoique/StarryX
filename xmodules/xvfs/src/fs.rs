use alloc::sync::Arc;
use inherit_methods_macro::inherit_methods;
use lock_api::RawMutex;

use crate::{DirEntry, VfsResult};

/// Filesystem statistics information
///
/// Provides information about filesystem capacity, usage, and properties.
pub struct StatFs {
    /// Filesystem type identifier
    pub fs_type: u32,
    /// Block size for filesystem I/O
    pub block_size: u32,
    /// Total number of blocks in filesystem
    pub blocks: u64,
    /// Number of free blocks available
    pub blocks_free: u64,
    /// Number of free blocks available to unprivileged user
    pub blocks_available: u64,

    /// Total number of file nodes (inodes)
    pub file_count: u64,
    /// Number of free file nodes
    pub free_file_count: u64,

    /// Maximum filename length
    pub name_length: u32,
    /// Fragment size
    pub fragment_size: u32,
    /// Mount flags
    pub mount_flags: u32,
}

/// Trait for filesystem operations
///
/// This trait defines the core operations that any filesystem implementation
/// must provide to work with the VFS layer.
pub trait FilesystemOps<M>: Send + Sync {
    /// Returns the name of this filesystem type
    fn name(&self) -> &str;
    /// Returns the root directory entry for this filesystem
    fn root_dir(&self) -> DirEntry<M>;
    /// Returns filesystem statistics
    fn stat(&self) -> VfsResult<StatFs>;
}

/// A handle to a mounted filesystem
///
/// This struct wraps filesystem operations and provides a convenient
/// interface for working with mounted filesystems.
pub struct Filesystem<M> {
    ops: Arc<dyn FilesystemOps<M>>,
}
impl<M> Clone for Filesystem<M> {
    fn clone(&self) -> Self {
        Self {
            ops: self.ops.clone(),
        }
    }
}

#[inherit_methods(from = "self.ops")]
impl<M: RawMutex> Filesystem<M> {
    /// Returns the name of this filesystem type
    pub fn name(&self) -> &str;
    /// Returns the root directory entry for this filesystem
    pub fn root_dir(&self) -> DirEntry<M>;
    /// Returns filesystem statistics
    pub fn stat(&self) -> VfsResult<StatFs>;
}

impl<M: RawMutex> Filesystem<M> {
    /// Creates a new filesystem handle from the given operations implementation
    pub fn new(ops: Arc<dyn FilesystemOps<M>>) -> Self {
        Self { ops }
    }
}
