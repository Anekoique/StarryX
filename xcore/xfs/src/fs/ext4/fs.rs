use core::cell::OnceCell;

use alloc::sync::{Arc, Weak};

use lock_api::{Mutex, MutexGuard, RawMutex};
use lwext4_rust::ffi::EXT4_ROOT_INO;
use weak_map::WeakMap;
use xdriver::XBlockDevice;
use xvfs::{
    CacheSlot, DirEntry, DirNode, Filesystem, FilesystemOps, Reference, StatFs, VfsResult,
    path::MAX_NAME_LEN,
};

use super::{
    Ext4Disk, Inode,
    util::{LwExt4Filesystem, into_vfs_err},
};

pub struct Ext4Filesystem<M: RawMutex> {
    inner: Mutex<M, LwExt4Filesystem>,
    inode_slots: Mutex<M, WeakMap<u32, Weak<InodeSlot<M>>>>,
    root_dir: OnceCell<DirEntry<M>>,
}

/// Shared by every live alias of one regular file: the cache attachment point
/// plus exactly-once deferred release of a fully unlinked inode.
///
/// `Arc` ownership guarantees the release runs once, when the last alias,
/// open handle, or cache backing disappears — however those drops interleave.
pub(crate) struct InodeSlot<M: RawMutex> {
    slot: Arc<CacheSlot<M>>,
    fs: Arc<Ext4Filesystem<M>>,
    ino: u32,
}

impl<M: RawMutex> InodeSlot<M> {
    pub(crate) fn cache_slot(&self) -> &Arc<CacheSlot<M>> {
        &self.slot
    }
}

impl<M: RawMutex> Drop for InodeSlot<M> {
    fn drop(&mut self) {
        if let Err(error) = self.fs.lock().release_unlinked(self.ino) {
            log::error!(
                "failed to release unlinked ext4 inode {}: {error:?}",
                self.ino
            );
        }
    }
}
impl<M: RawMutex> Ext4Filesystem<M> {
    pub fn mount(dev: XBlockDevice) -> VfsResult<Filesystem<M>>
    where
        M: Send + Sync + 'static,
    {
        let ext4 = lwext4_rust::Ext4Filesystem::new(Ext4Disk(dev)).map_err(into_vfs_err)?;

        let fs = Arc::new(Self {
            inner: Mutex::new(ext4),
            inode_slots: Mutex::new(WeakMap::new()),
            root_dir: OnceCell::new(),
        });
        let _ = fs.root_dir.set(DirEntry::new_dir(
            |this| {
                DirNode::new(Inode::new(
                    fs.clone(),
                    EXT4_ROOT_INO,
                    xvfs::NodeType::Directory,
                    Some(this),
                ))
            },
            Reference::root(),
        ));
        Ok(Filesystem::new(fs))
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, M, LwExt4Filesystem> {
        self.inner.lock()
    }

    /// Returns the slot shared by every live alias of `ino`.
    ///
    /// The map holds only self-cleaning weak references, so once the last
    /// alias drops, a recycled inode number receives a fresh slot and never
    /// inherits stale cache state.
    pub(crate) fn inode_slot(self: &Arc<Self>, ino: u32) -> Arc<InodeSlot<M>> {
        let mut slots = self.inode_slots.lock();
        if let Some(slot) = slots.get(&ino) {
            return slot;
        }
        let slot = Arc::new(InodeSlot {
            slot: CacheSlot::new(),
            fs: self.clone(),
            ino,
        });
        slots.insert(ino, &slot);
        slot
    }
}

// SAFETY: `root_dir` is written once during mount before the filesystem is
// shared; every other field sits behind `Mutex<M, _>`, and the lwext4 handles
// inside are heap state that one exclusive holder may use from any thread.
// `M: Send + Sync` carries the locks' own thread-safety.
unsafe impl<M: RawMutex + Send + Sync> Send for Ext4Filesystem<M> {}
unsafe impl<M: RawMutex + Send + Sync> Sync for Ext4Filesystem<M> {}

impl<M: RawMutex + Send + Sync + 'static> FilesystemOps<M> for Ext4Filesystem<M> {
    fn name(&self) -> &str {
        "ext4"
    }

    fn root_dir(&self) -> DirEntry<M> {
        self.root_dir.get().unwrap().clone()
    }

    fn stat(&self) -> VfsResult<StatFs> {
        let mut fs = self.lock();
        let stat = fs.stat().map_err(into_vfs_err)?;
        Ok(StatFs {
            fs_type: 0xef53,
            block_size: stat.block_size as _,
            blocks: stat.blocks_count,
            blocks_free: stat.free_blocks_count,
            blocks_available: stat.free_blocks_count,

            file_count: stat.inodes_count as _,
            free_file_count: stat.free_inodes_count as _,

            name_length: MAX_NAME_LEN as _,
            fragment_size: 0,
            mount_flags: 0,
        })
    }
}
