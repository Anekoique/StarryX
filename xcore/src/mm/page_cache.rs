use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use axerrno::LinuxResult;
use lazy_static::lazy_static;
use spin::RwLock;
use xcache::PageCache;

use super::{InodeWrapper, XUserSpace};

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
            .or_insert_with(|| Arc::new(PageCache::new(inode)))
            .clone()
    }

    pub fn remove(&self, inode: u64) {
        self.caches.write().remove(&inode);
    }

    pub fn clear(&self) {
        self.caches.write().clear();
    }

    pub fn sync_file(&self, inode: u64) -> LinuxResult<()> {
        if let Some(cache) = self.get_cache(inode) {
            cache.sync()?;
        }
        Ok(())
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
            if let Some(cache) = caches.remove(&key) {
                let _ = cache.evict();
            }
        }
    }
}

struct AxPageCacheImpl;
#[crate_interface::impl_interface]
impl axalloc::AxAllocIf for AxPageCacheImpl {
    fn evict_cache(num_pages: usize) -> axalloc::AllocResult {
        let caches = PAGE_CACHE_MANAGER.caches.write();
        let mut total_evicted = 0;
        for (_, cache) in caches.iter() {
            if let Ok(count) = cache.evict() {
                total_evicted += count;
            }
            if total_evicted >= num_pages {
                break;
            }
        }
        error!(
            "Evicted {} pages from cache, expected {}",
            total_evicted, num_pages
        );
        if total_evicted < num_pages {
            Err(axalloc::AllocError::NoMemory)
        } else {
            Ok(())
        }
    }
}
