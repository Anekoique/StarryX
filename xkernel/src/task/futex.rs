//! Futex implementation.

use alloc::{collections::btree_map::BTreeMap, sync::Arc};
use core::{ops::Deref, sync::atomic::AtomicBool};

use memory_addr::VirtAddr;
use xsync::Mutex;
use xtask::WaitQueue;

use crate::task::api::with_uspace;

/// A key that uniquely identifies a futex in the system.
pub enum FutexKey {
    /// A futex that is private to the current process.
    Private {
        /// The memory address of the futex.
        address: usize,
    },

    /// A futex in a region shared between processes.
    Shared {
        /// The identity of the shared object holding the futex.
        object: u64,
        /// The offset of the futex within that object.
        offset: usize,
    },
}
impl FutexKey {
    /// Creates a new `FutexKey`.
    ///
    /// A write-through mapping is keyed by object identity so unrelated
    /// processes mapping it at different addresses agree on the same futex.
    pub fn new(address: usize) -> Self {
        with_uspace(|uspace| {
            let aspace = &uspace.aspace.lock();
            match aspace.shared_object_at(VirtAddr::from_usize(address)) {
                Some((object, offset)) => Self::Shared { object, offset },
                None => Self::Private { address },
            }
        })
    }

    fn table_key(&self) -> (u64, usize) {
        match self {
            FutexKey::Private { address } => (0, *address),
            FutexKey::Shared { object, offset } => (*object, *offset),
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

pub struct FutexTable(Mutex<BTreeMap<(u64, usize), Arc<FutexEntry>>>);
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
    key: (u64, usize),
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
        let mut table = self.table.0.lock();
        let unused = table.get(&self.key).is_some_and(|entry| {
            Arc::ptr_eq(entry, &self.inner)
                && Arc::strong_count(&self.inner) == 2
                && self.inner.wq.is_empty()
        });
        if unused {
            table.remove(&self.key);
        }
    }
}
