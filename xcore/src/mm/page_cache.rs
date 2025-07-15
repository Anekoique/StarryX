use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use lazy_static::lazy_static;
use page_cache::PageCache;
use spin::RwLock;

use super::XUserSpace;
use crate::mm::InodeWrapper;

lazy_static! {
    pub static ref PAGE_CACHE_MANAGER: PageCacheManager = PageCacheManager::new();
}

pub struct PageCacheManager {
    caches: RwLock<BTreeMap<u64, Arc<PageCache<InodeWrapper, XUserSpace>>>>,
}

impl PageCacheManager {
    pub fn new() -> Self {
        Self {
            caches: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn get_cache(&self, inode: u64) -> Option<Arc<PageCache<InodeWrapper, XUserSpace>>> {
        self.caches.read().get(&inode).cloned()
    }

    pub fn get_or_create(&self, inode: InodeWrapper) -> Arc<PageCache<InodeWrapper, XUserSpace>> {
        self.caches
            .write()
            .entry(inode.inode())
            .or_insert_with(|| {
                debug!("cache create: {:?}", inode.inode());
                Arc::new(PageCache::new(inode))
            })
            .clone()
    }

    pub fn remove(&self, inode: u64) {
        self.caches.write().remove(&inode);
    }

    pub fn clear(&self) {
        self.caches.write().clear();
    }

    pub fn clear_stale_cache(&self) {
        let mut caches = self.caches.write();
        let stale_keys: Vec<u64> = caches
            .iter()
            .filter_map(|(key, cache)| {
                if cache.host.is_stale() {
                    Some(*key)
                } else {
                    None
                }
            })
            .collect();

        for key in stale_keys {
            debug!("Removing stale cache for inode: {}", key);
            if let Some(cache) = caches.remove(&key) {
                let _ = cache.clear();
            }
        }
        drop(caches);
        debug!("clear stale cache done");
    }
}
