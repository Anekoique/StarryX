use core::{
    iter,
    sync::atomic::{AtomicU64, Ordering},
};

use alloc::{
    collections::btree_map::BTreeMap,
    string::String,
    sync::{Arc, Weak},
    vec,
};
use inherit_methods_macro::inherit_methods;
use lock_api::{Mutex, RawMutex};

use crate::{
    DirEntry, DirEntrySink, FileNodeOps, Filesystem, FilesystemOps, Metadata, MetadataUpdate,
    NodeOps, NodePermission, NodeType, ReferenceKey, VfsError, VfsResult,
    path::{DOT, DOTDOT, PathBuf},
};

/// A mountpoint in the filesystem hierarchy
///
/// Represents a point where a filesystem is mounted in the directory tree.
/// Each mountpoint contains a root directory entry and may have child mountpoints.
pub struct Mountpoint<M> {
    /// Root dir entry in the mountpoint.
    root: DirEntry<M>,
    /// Location in the parent mountpoint.
    location: Option<Location<M>>,
    /// Children of the mountpoint.
    children: Mutex<M, BTreeMap<ReferenceKey, Weak<Self>>>,
    /// Device ID
    device: u64,
}
impl<M: RawMutex> Mountpoint<M> {
    /// Creates a new mountpoint for the given filesystem
    ///
    /// # Arguments
    /// * `fs` - The filesystem to mount
    /// * `location_in_parent` - Optional location where this mountpoint is mounted in its parent
    pub fn new(fs: &Filesystem<M>, location_in_parent: Option<Location<M>>) -> Arc<Self> {
        static DEVICE_COUNTER: AtomicU64 = AtomicU64::new(1);

        let root = fs.root_dir();
        Arc::new(Self {
            root,
            location: location_in_parent,
            children: Mutex::default(),
            device: DEVICE_COUNTER.fetch_add(1, Ordering::Relaxed),
        })
    }

    /// Creates a new root mountpoint (with no parent)
    pub fn new_root(fs: &Filesystem<M>) -> Arc<Self> {
        Self::new(fs, None)
    }

    /// Returns a location pointing to the root of this mountpoint
    pub fn root_location(self: &Arc<Self>) -> Location<M> {
        Location::new(self.clone(), self.root.clone())
    }

    /// Returns the location in the parent mountpoint.
    pub fn location(&self) -> Option<Location<M>> {
        self.location.clone()
    }

    /// Returns true if this is a root mountpoint (has no parent)
    pub fn is_root(&self) -> bool {
        self.location.is_none()
    }

    /// Returns the effective mountpoint.
    ///
    /// For example, first `mount /dev/sda1 /mnt` and then `mount /dev/sda2
    /// /mnt`. After the second mount is completed, the content of the first
    /// mount will be overridden (root mount -> mnt1 -> mnt2). We need to
    /// return `mnt2` for `mnt1.effective_mountpoint()`.
    pub(crate) fn effective_mountpoint(self: &Arc<Self>) -> Arc<Mountpoint<M>> {
        let mut mountpoint = self.clone();
        while let Some(mount) = mountpoint.root.as_dir().unwrap().mountpoint() {
            mountpoint = mount;
        }
        mountpoint
    }

    /// Returns the device ID for this mountpoint
    pub fn device(self: &Arc<Self>) -> u64 {
        self.device
    }
}

/// A location within the filesystem tree
///
/// Represents a specific location that combines a mountpoint with
/// a directory entry within that mountpoint. This allows for
/// traversing across mount boundaries.
pub struct Location<M> {
    mountpoint: Arc<Mountpoint<M>>,
    entry: DirEntry<M>,
}
impl<M> Clone for Location<M> {
    fn clone(&self) -> Self {
        Self {
            mountpoint: self.mountpoint.clone(),
            entry: self.entry.clone(),
        }
    }
}

#[inherit_methods(from = "self.entry")]
impl<M: RawMutex> Location<M> {
    /// Returns the inode number of this location
    pub fn inode(&self) -> u64;
    /// Returns the filesystem operations for this location
    pub fn filesystem(&self) -> &dyn FilesystemOps<M>;
    /// Updates the metadata of this location
    pub fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;
    /// Returns the size of this location in bytes
    pub fn len(&self) -> VfsResult<u64>;
    /// Syncs this location to persistent storage
    pub fn sync(&self, data_only: bool) -> VfsResult<()>;

    /// Returns true if this location represents a file
    pub fn is_file(&self) -> bool;
    /// Returns true if this location represents a directory
    pub fn is_dir(&self) -> bool;

    /// Returns the node type of this location
    pub fn node_type(&self) -> NodeType;
    /// Returns true if this location is the root of a mount
    pub fn is_root_of_mount(&self) -> bool;

    /// Reads the target of a symbolic link
    pub fn read_link(&self) -> VfsResult<String>;

    /// Returns whether the location is empty (has zero length).
    pub fn is_empty(&self) -> VfsResult<bool> {
        self.len().map(|len| len == 0)
    }

    /// Get the file node
    pub fn get_file_node(&self) -> Arc<dyn FileNodeOps<M>> {
        self.entry.get_file_node()
    }
    pub fn node(&self) -> Weak<dyn NodeOps<M>> {
        self.entry.node()
    }
}

impl<M: RawMutex> Location<M> {
    /// Creates a new location
    pub fn new(mountpoint: Arc<Mountpoint<M>>, entry: DirEntry<M>) -> Self {
        Self { mountpoint, entry }
    }

    /// Wraps a directory entry with this location's mountpoint
    fn wrap(&self, entry: DirEntry<M>) -> Self {
        Self::new(self.mountpoint.clone(), entry)
    }

    /// Returns a reference to the mountpoint
    pub fn mountpoint(&self) -> &Arc<Mountpoint<M>> {
        &self.mountpoint
    }

    /// Returns a reference to the directory entry
    pub fn entry(&self) -> &DirEntry<M> {
        &self.entry
    }

    /// Returns the name of this location
    pub fn name(&self) -> &str {
        if self.is_root_of_mount() {
            self.mountpoint.location.as_ref().map_or("", Location::name)
        } else {
            self.entry.name()
        }
    }

    /// Returns the parent location, if any
    pub fn parent(&self) -> Option<Self> {
        if !self.is_root_of_mount() {
            return Some(self.wrap(self.entry.parent().unwrap()));
        }
        self.mountpoint.location()?.parent()
    }

    /// Returns true if this is the root location of the entire filesystem
    pub fn is_root(&self) -> bool {
        self.mountpoint.is_root() && self.entry.is_root_of_mount()
    }

    /// Returns true if this location is a symbolic link
    pub fn is_symlink(&self) -> bool {
        self.node_type() == NodeType::Symlink
    }

    /// Checks that this location is a directory, returning an error if not
    pub fn check_is_dir(&self) -> VfsResult<()> {
        self.entry.as_dir().map(|_| ())
    }

    /// Checks that this location is a file, returning an error if not
    pub fn check_is_file(&self) -> VfsResult<()> {
        self.entry.as_file().map(|_| ())
    }

    /// Returns the metadata for this location
    pub fn metadata(&self) -> VfsResult<Metadata> {
        let mut metadata = self.entry.metadata()?;
        metadata.device = self.mountpoint.device();
        Ok(metadata)
    }

    /// Returns the absolute path to this location
    pub fn absolute_path(&self) -> VfsResult<PathBuf> {
        let mut components = vec![];
        let mut cur = self.clone();
        loop {
            cur.entry.collect_absolute_path(&mut components);
            cur = match cur.mountpoint.location() {
                Some(loc) => loc,
                None => break,
            }
        }
        Ok(iter::once("/")
            .chain(components.iter().map(String::as_str).rev())
            .collect())
    }

    /// Checks if this location points to the same object as another location
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.mountpoint, &other.mountpoint) && self.entry.ptr_eq(&other.entry)
    }

    /// Returns true if this location is a mountpoint
    pub fn is_mountpoint(&self) -> bool {
        self.entry.as_dir().is_ok_and(|it| it.is_mountpoint())
    }

    /// See [`Mountpoint::effective_mountpoint`].
    fn resolve_mountpoint(self) -> Self {
        let Some(mountpoint) = self.entry.as_dir().ok().and_then(|it| it.mountpoint()) else {
            return self;
        };
        let mountpoint = mountpoint.effective_mountpoint();
        let entry = mountpoint.root.clone();
        Self::new(mountpoint, entry)
    }

    /// Looks up a child entry by name without following symbolic links
    pub fn lookup_no_follow(&self, name: &str) -> VfsResult<Self> {
        Ok(match name {
            DOT => self.clone(),
            DOTDOT => self.parent().unwrap_or_else(|| self.clone()),
            _ => {
                let loc = Self::new(self.mountpoint.clone(), self.entry.as_dir()?.lookup(name)?);
                loc.resolve_mountpoint()
            }
        })
    }

    /// Creates a new filesystem node at this location
    pub fn create(
        &self,
        name: &str,
        node_type: NodeType,
        permission: NodePermission,
    ) -> VfsResult<Self> {
        self.entry
            .as_dir()?
            .create(name, node_type, permission)
            .map(|entry| self.wrap(entry))
    }

    /// Creates a hard link to another location
    pub fn link(&self, name: &str, node: &Self) -> VfsResult<Self> {
        if !Arc::ptr_eq(&self.mountpoint, &node.mountpoint) {
            return Err(VfsError::EXDEV);
        }
        self.entry
            .as_dir()?
            .link(name, &node.entry)
            .map(|entry| self.wrap(entry))
    }

    /// Renames a file or directory
    pub fn rename(&self, src_name: &str, dst_dir: &Self, dst_name: &str) -> VfsResult<()> {
        if !Arc::ptr_eq(&self.mountpoint, &dst_dir.mountpoint) {
            return Err(VfsError::EXDEV);
        }
        if !self.ptr_eq(dst_dir) && self.entry.is_ancestor_of(&dst_dir.entry)? {
            return Err(VfsError::EINVAL);
        }
        self.entry
            .as_dir()?
            .rename(src_name, dst_dir.entry.as_dir()?, dst_name)
    }

    /// Removes a file or directory
    pub fn unlink(&self, name: &str, is_dir: bool) -> VfsResult<()> {
        self.entry.as_dir()?.unlink(name, is_dir)
    }

    /// Opens or creates a file at this location
    pub fn open_file_or_create(
        &self,
        name: &str,
        create: bool,
        create_new: bool,
        permission: NodePermission,
        user: Option<(u32, u32)>,
    ) -> VfsResult<Location<M>> {
        self.entry
            .as_dir()?
            .open_file_or_create(name, create, create_new, permission, user)
            .map(|entry| self.wrap(entry).resolve_mountpoint())
    }

    /// Reads directory entries
    pub fn read_dir(&self, offset: u64, sink: &mut dyn DirEntrySink) -> VfsResult<usize> {
        self.entry.as_dir()?.read_dir(offset, sink)
    }

    /// Mounts a filesystem at this location
    pub fn mount(&self, fs: &Filesystem<M>) -> VfsResult<Arc<Mountpoint<M>>> {
        let mut mountpoint = self.entry.as_dir()?.mountpoint.lock();
        if mountpoint.is_some() {
            return Err(VfsError::EBUSY);
        }
        let result = Mountpoint::new(fs, Some(self.clone()));
        *mountpoint = Some(result.clone());
        self.mountpoint
            .children
            .lock()
            .insert(self.entry.key(), Arc::downgrade(&result));
        Ok(result)
    }

    /// Unmounts the filesystem at this location
    pub fn unmount(&self) -> VfsResult<()> {
        let Some(mountpoint) = self.entry.as_dir()?.mountpoint.lock().take() else {
            return Err(VfsError::EINVAL);
        };
        mountpoint.root.as_dir()?.forget();
        if self
            .mountpoint
            .children
            .lock()
            .remove(&self.entry.key())
            .is_none()
        {
            return Err(VfsError::EINVAL);
        }
        Ok(())
    }
}
