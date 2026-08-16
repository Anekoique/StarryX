//! Futex implementation.

use alloc::{
    collections::btree_map::BTreeMap,
    sync::{Arc, Weak},
};
use core::{ops::Deref, sync::atomic::AtomicBool};

use memory_addr::VirtAddr;
use xsync::Mutex;
use xtask::WaitQueue;
use xvma::SharedObject;

use crate::task::api::with_uspace;

/// A key that uniquely identifies a futex in the system.
pub enum FutexKey {
    /// A futex that is private to the current process.
    Private {
        /// The memory address of the futex.
        address: usize,
    },

    /// A futex in a shared memory region.
    Shared {
        /// The offset of the futex within the shared memory region.
        offset: usize,
        /// The shared memory region, represented as a weak reference to the
        /// shared pages.
        region: Weak<SharedObject>,
    },
}
impl FutexKey {
    /// Creates a new `FutexKey`.
    pub fn new(address: usize) -> Self {
        with_uspace(|uspace| {
            let aspace = &uspace.aspace.lock();
            if let Some((offset, object)) = aspace.shared_mapping_at(VirtAddr::from_usize(address))
            {
                return Self::Shared {
                    offset,
                    region: Arc::downgrade(&object),
                };
            }
            Self::Private { address }
        })
    }

    fn table_key(&self) -> (usize, usize) {
        match self {
            FutexKey::Private { address } => (0, *address),
            FutexKey::Shared { offset, region } => (region.as_ptr() as usize, *offset),
        }
    }
}

pub struct FutexEntry {
    pub wq: WaitQueue,
    pub owner_dead: AtomicBool,
}
impl FutexEntry {
    fn new() -> Self {
        Self {
            wq: WaitQueue::new(),
            owner_dead: AtomicBool::new(false),
        }
    }
}

pub struct FutexTable(Mutex<BTreeMap<(usize, usize), Arc<FutexEntry>>>);
impl FutexTable {
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(Mutex::new(BTreeMap::new()))
    }

    pub fn get(&self, key: &FutexKey) -> Option<FutexGuard<'_>> {
        let key = key.table_key();
        let entry = self.0.lock().get(&key).cloned()?;
        Some(FutexGuard {
            table: self,
            key,
            inner: entry,
        })
    }

    pub fn get_or_insert(&self, key: &FutexKey) -> FutexGuard<'_> {
        let key = key.table_key();
        let mut table = self.0.lock();
        let entry = table
            .entry(key)
            .or_insert_with(|| Arc::new(FutexEntry::new()));
        FutexGuard {
            table: self,
            key,
            inner: entry.clone(),
        }
    }
}

pub struct FutexGuard<'a> {
    table: &'a FutexTable,
    key: (usize, usize),
    inner: Arc<FutexEntry>,
}
impl Deref for FutexGuard<'_> {
    type Target = Arc<FutexEntry>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl Drop for FutexGuard<'_> {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) <= 2 && self.inner.wq.is_empty() {
            self.table.0.lock().remove(&self.key);
        }
    }
}
