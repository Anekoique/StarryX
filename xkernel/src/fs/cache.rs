// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use alloc::sync::Arc;
use core::{any::Any, ops::Range};

use lazy_static::lazy_static;
use xcache::{Backing, CacheManager, CachePolicy, FileMapping, WritebackCursor};
use xerrno::{LinuxError, LinuxResult};
use xsync::{Mutex, RawMutex};
use xvfs::{CacheSlot, FileNodeOps};

lazy_static! {
    static ref CACHE_MANAGER: Arc<CacheManager> =
        CacheManager::new(default_policy()).expect("the static page-cache policy must be valid");
}

fn default_policy() -> CachePolicy {
    let pages = xconfig::plat::PHYS_MEMORY_SIZE / xcache::PAGE_SIZE;
    CachePolicy {
        free_low: (pages / 32).max(64),
        free_high: (pages / 16).max(128),
        dirty_background: (pages / 32).max(64),
        dirty_limit: (pages / 16).max(128),
        writeback_batch_pages: 64,
    }
}

pub fn init() {
    let manager = CACHE_MANAGER.clone();
    xtask::spawn(move || {
        manager.run_worker(|| xalloc::global_allocator().available_pages());
    });
}

pub fn shutdown() -> LinuxResult {
    CACHE_MANAGER.shutdown()
}

/// One consumer's handle on a file's cache: the shared mapping plus a private
/// writeback-error cursor, releasing its pin hint on drop.
///
/// Open files and mmap objects both embed this, so the release-hint lifetime
/// rule and per-observer error reporting exist once.
pub struct CachedMapping {
    mapping: Arc<FileMapping>,
    cursor: Mutex<WritebackCursor>,
}

impl CachedMapping {
    pub fn new(mapping: Arc<FileMapping>) -> Self {
        Self {
            cursor: Mutex::new(mapping.new_cursor()),
            mapping,
        }
    }

    pub fn mapping(&self) -> &Arc<FileMapping> {
        &self.mapping
    }

    /// Synchronizes `range`, reporting an error once through this handle.
    pub fn sync_range(&self, range: Range<u64>, data_only: bool, wait: bool) -> LinuxResult {
        self.mapping
            .sync_range(range, data_only, wait, &mut self.cursor.lock())
    }
}

impl Drop for CachedMapping {
    fn drop(&mut self) {
        self.mapping.release_hint();
    }
}

/// Returns the mapping attached to the node's cache slot, creating it first
/// if the file has none.
///
/// Every alias of one file shares one slot, so buffered I/O and mmap converge
/// on the same mapping without a global lookup. The slot holds the mapping
/// weakly; the cache manager pins it strongly for as long as pages remain.
pub fn mapping_for(node: Arc<dyn FileNodeOps<RawMutex>>) -> LinuxResult<Option<Arc<FileMapping>>> {
    let Some(slot) = node.cache_slot().cloned() else {
        return Ok(None);
    };
    if let Some(mapping) = attached_mapping(&slot) {
        // An idle prune may have unregistered the mapping between the weak
        // upgrade and here; re-pin it so writeback and shutdown see it.
        CACHE_MANAGER.ensure_registered(&mapping)?;
        return Ok(Some(mapping));
    }
    let backing: Arc<dyn Backing> = Arc::new(VfsBacking(node));
    let id = xvma::allocate_object_id()?;
    let mapping = CACHE_MANAGER.create_mapping(id, backing)?;
    let attachment: Arc<dyn Any + Send + Sync> = mapping.clone();
    let winner = slot
        .attach_if_empty(attachment)
        .downcast::<FileMapping>()
        .map_err(|_| LinuxError::EINVAL)?;
    if !Arc::ptr_eq(&winner, &mapping) {
        mapping.release_hint();
        CACHE_MANAGER.ensure_registered(&winner)?;
    }
    Ok(Some(winner))
}

/// Returns the cached logical length when the node has a live mapping, whose
/// length is authoritative while unflushed writes exist.
pub fn cached_len(node: &dyn FileNodeOps<RawMutex>) -> Option<u64> {
    node.cache_slot()
        .and_then(attached_mapping)
        .map(|mapping| mapping.size())
}

/// Discards an unlinked file's cache when no open file or VMA still owns it.
///
/// This runs only after the filesystem removed the directory entry. A
/// remaining hard link keeps both the cache identity and dirty data intact.
pub fn complete_unlink(node: &dyn FileNodeOps<RawMutex>) {
    let Some(slot) = node.cache_slot() else {
        return;
    };
    match node.metadata() {
        Ok(metadata) if metadata.nlink == 0 => {
            // Deciding under the slot lock blocks a concurrent open from
            // reviving the mapping mid-discard; a successful discard also
            // clears the slot so a later open starts from the backing.
            slot.detach_if(|attachment| {
                let Ok(mapping) = attachment.downcast::<FileMapping>() else {
                    return false;
                };
                let id = mapping.id();
                drop(mapping);
                let discarded = CACHE_MANAGER.discard_unowned(id);
                if !discarded {
                    // A busy unlinked file keeps its dirty pages; the registry
                    // holds them until writeback, reclaim, or shutdown.
                    debug!("unlinked cache mapping {id} still owned; retained");
                }
                discarded
            });
        }
        Ok(_) => {}
        Err(error) => warn!("cannot inspect unlinked cache mapping: {error}"),
    }
}

fn attached_mapping(slot: &Arc<CacheSlot<RawMutex>>) -> Option<Arc<FileMapping>> {
    slot.get().and_then(|attachment| attachment.downcast().ok())
}

struct VfsBacking(Arc<dyn FileNodeOps<RawMutex>>);

impl Backing for VfsBacking {
    fn byte_len(&self) -> LinuxResult<u64> {
        self.0.len()
    }

    fn read_at(&self, offset: u64, destination: &mut [u8]) -> LinuxResult<usize> {
        self.0.read_at(destination, offset)
    }

    fn write_at(&self, offset: u64, source: &[u8]) -> LinuxResult<usize> {
        self.0.write_at(source, offset)
    }

    fn set_len(&self, len: u64) -> LinuxResult {
        self.0.set_len(len)
    }

    fn sync(&self, data_only: bool) -> LinuxResult {
        self.0.sync(data_only)
    }
}
