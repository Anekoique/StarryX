//! Futex implementation.

use core::{ops::Deref, sync::atomic::AtomicBool};

use alloc::{collections::btree_map::BTreeMap, sync::Arc};
use axsync::Mutex;
use axtask::WaitQueue;

use crate::task::api::with_xprocess;

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

pub struct FutexTable(Mutex<BTreeMap<usize, Arc<FutexEntry>>>);
impl FutexTable {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(Mutex::new(BTreeMap::new()))
    }

    pub fn get(&self, addr: usize) -> Option<FutexGuard> {
        let entry = self.0.lock().get(&addr).cloned()?;
        Some(FutexGuard {
            key: addr,
            inner: entry,
        })
    }

    pub fn get_or_insert(&self, addr: usize) -> FutexGuard {
        let mut table = self.0.lock();
        let entry = table
            .entry(addr)
            .or_insert_with(|| Arc::new(FutexEntry::new()));
        FutexGuard {
            key: addr,
            inner: entry.clone(),
        }
    }
}

pub struct FutexGuard {
    key: usize,
    inner: Arc<FutexEntry>,
}
impl Deref for FutexGuard {
    type Target = Arc<FutexEntry>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl Drop for FutexGuard {
    fn drop(&mut self) {
        with_xprocess(|proc| {
            let mut table = proc.futex_table.0.lock();
            if Arc::strong_count(&self.inner) == 1 && self.inner.wq.is_empty() {
                table.remove(&self.key);
            }
        });
    }
}
