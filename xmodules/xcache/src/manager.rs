// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use alloc::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Weak},
    vec::Vec,
};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use spin::Mutex;
use xerrno::{LinuxError, LinuxResult};
use xmm::Frame;
use xtask::WaitQueue;

use crate::{Backing, FileMapping, page::CachedPage};

#[derive(Clone, Copy, Debug)]
pub struct CachePolicy {
    pub free_low: usize,
    pub free_high: usize,
    pub dirty_background: usize,
    pub dirty_limit: usize,
    pub writeback_batch_pages: usize,
}

impl CachePolicy {
    fn validate(self) -> LinuxResult<Self> {
        if self.free_low > self.free_high
            || self.dirty_background > self.dirty_limit
            || self.writeback_batch_pages == 0
        {
            return Err(LinuxError::EINVAL);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    Running,
    Closing,
    Closed,
}

struct ManagerState {
    lifecycle: Lifecycle,
    worker_running: bool,
    worker_result: Option<LinuxResult>,
}

pub struct CacheManager {
    policy: CachePolicy,
    lifecycle: Mutex<ManagerState>,
    registry: Mutex<BTreeMap<u64, Arc<FileMapping>>>,
    candidates: Mutex<VecDeque<Weak<CachedPage>>>,
    accepting_operations: AtomicBool,
    active_operations: AtomicUsize,
    writeback_requested: AtomicBool,
    /// Id of the mapping the last background batch serviced, so batches
    /// rotate across mappings instead of always starting at the lowest id.
    writeback_resume: AtomicU64,
    progress_epoch: AtomicU64,
    active_wait: WaitQueue,
    worker_wait: WaitQueue,
    resident_pages: AtomicUsize,
    dirty_pages: AtomicUsize,
}

pub(crate) struct OperationPermit<'a> {
    manager: &'a CacheManager,
}

impl CacheManager {
    pub fn new(policy: CachePolicy) -> LinuxResult<Arc<Self>> {
        Ok(Arc::new(Self {
            policy: policy.validate()?,
            lifecycle: Mutex::new(ManagerState {
                lifecycle: Lifecycle::Running,
                worker_running: false,
                worker_result: None,
            }),
            registry: Mutex::new(BTreeMap::new()),
            candidates: Mutex::new(VecDeque::new()),
            accepting_operations: AtomicBool::new(true),
            active_operations: AtomicUsize::new(0),
            writeback_requested: AtomicBool::new(false),
            writeback_resume: AtomicU64::new(0),
            progress_epoch: AtomicU64::new(0),
            active_wait: WaitQueue::with_capacity(1),
            worker_wait: WaitQueue::with_capacity(1),
            resident_pages: AtomicUsize::new(0),
            dirty_pages: AtomicUsize::new(0),
        }))
    }

    /// Creates a mapping under the caller's identity and pins it in the
    /// registry.
    ///
    /// `id` must be boot-global and never reused. Deduplication per file
    /// happens at the caller's attachment point; a caller that loses that race
    /// releases its mapping with [`FileMapping::release_hint`].
    pub fn create_mapping(
        self: &Arc<Self>,
        id: u64,
        backing: Arc<dyn Backing>,
    ) -> LinuxResult<Arc<FileMapping>> {
        let _permit = self.permit()?;
        let length = backing.byte_len()?;
        let mapping = FileMapping::new(id, Arc::downgrade(self), backing, length);
        self.registry.lock().insert(id, mapping.clone());
        Ok(mapping)
    }

    /// Re-pins a mapping a concurrent prune may have just unregistered.
    ///
    /// A caller that revives a mapping through a weak reference must call this
    /// before using it, so writeback and shutdown always see the mapping.
    pub fn ensure_registered(&self, mapping: &Arc<FileMapping>) -> LinuxResult {
        let _permit = self.permit()?;
        self.registry
            .lock()
            .entry(mapping.id())
            .or_insert_with(|| mapping.clone());
        Ok(())
    }

    /// Discards an unreferenced mapping for a file that is about to vanish.
    ///
    /// The caller must block concurrent revival of `key` (the integration
    /// layer holds the file's attachment lock), so a discarded mapping cannot
    /// hand out its pages.
    pub fn discard_unowned(&self, key: u64) -> bool {
        let mut registry = self.registry.lock();
        let Some(mapping) = registry.get(&key) else {
            return true;
        };
        if Arc::strong_count(mapping) != 1 {
            return false;
        }
        let Some((resident, dirty)) = mapping.discard_unowned_pages() else {
            return false;
        };
        registry.remove(&key);
        self.remove_resident(resident, dirty);
        self.prune_candidates();
        true
    }

    fn maintain(&self, available_pages: usize) -> LinuxResult {
        let _permit = self.permit()?;
        let written = if self.writeback_requested.swap(false, Ordering::AcqRel)
            || self.dirty_pages.load(Ordering::Acquire) >= self.policy.dirty_background
            || available_pages < self.policy.free_low
        {
            self.writeback_inner(self.policy.writeback_batch_pages, false)?
        } else {
            0
        };
        // Idle mappings keep their clean pages while memory is plentiful, so a
        // close/reopen cycle still hits the cache; pressure reclaims them.
        let reclaimed = if available_pages < self.policy.free_high {
            self.reclaim_clean(self.policy.free_high - available_pages)
                + self.prune_idle_mappings()?
        } else {
            0
        };
        if written != 0 || reclaimed != 0 {
            self.progress_epoch.fetch_add(1, Ordering::AcqRel);
        }
        Ok(())
    }

    fn snapshot_registry(&self) -> LinuxResult<Vec<Arc<FileMapping>>> {
        let registry = self.registry.lock();
        let mut mappings = Vec::new();
        mappings
            .try_reserve(registry.len())
            .map_err(|_| LinuxError::ENOMEM)?;
        mappings.extend(registry.values().cloned());
        Ok(mappings)
    }

    fn prune_candidates(&self) {
        self.candidates
            .lock()
            .retain(|candidate| candidate.strong_count() != 0);
    }

    fn writeback_inner(&self, max_pages: usize, explicit: bool) -> LinuxResult<usize> {
        let mappings = self.snapshot_registry()?;
        // Resume after the last-serviced mapping so one continuously dirtied
        // file cannot starve the others of background writeback.
        let resume = self.writeback_resume.load(Ordering::Relaxed);
        let pivot = mappings
            .iter()
            .position(|mapping| mapping.id() > resume)
            .unwrap_or(0);
        let mut written = 0;
        for mapping in mappings[pivot..].iter().chain(&mappings[..pivot]) {
            if written == max_pages {
                break;
            }
            written += mapping.writeback_some(max_pages - written, explicit)?;
            self.writeback_resume.store(mapping.id(), Ordering::Relaxed);
        }
        Ok(written)
    }

    pub(crate) fn throttle_dirty(&self) -> LinuxResult {
        if self.dirty_pages.load(Ordering::Acquire) < self.policy.dirty_limit {
            return Ok(());
        }
        self.writeback_inner(self.policy.writeback_batch_pages, false)?;
        self.wake();
        Ok(())
    }

    pub(crate) fn request_writeback(&self) {
        self.writeback_requested.store(true, Ordering::Release);
        self.wake();
    }

    fn prune_idle_mappings(&self) -> LinuxResult<usize> {
        let mut reclaimed = 0;
        for mapping in self.snapshot_registry()? {
            if Arc::strong_count(&mapping) != 2 {
                continue;
            }
            reclaimed += mapping.reclaim_idle_pages();
            self.prune_mapping(&mapping);
        }
        self.resident_pages.fetch_sub(reclaimed, Ordering::Relaxed);
        self.prune_candidates();
        Ok(reclaimed)
    }

    pub(crate) fn prune_mapping(&self, mapping: &FileMapping) {
        if !mapping.has_no_pages() {
            return;
        }
        let key = mapping.id();
        let mut registry = self.registry.lock();
        if registry.get(&key).is_some_and(|current| {
            core::ptr::eq(current.as_ref(), mapping) && Arc::strong_count(current) == 2
        }) {
            registry.remove(&key);
        }
    }

    /// Reclaims clean, unreferenced cache owners without allocation, waits, or I/O.
    fn reclaim_clean(&self, target_pages: usize) -> usize {
        let mut reclaimed = 0;
        let mut examined = 0;
        let initial = self.candidates.lock().len();
        // One clock pass clears the referenced bit; the second can reclaim
        // pages that were otherwise idle throughout the scan.
        let scan_budget = initial.saturating_mul(2);
        while reclaimed < target_pages && examined < scan_budget {
            examined += 1;
            let candidate = self.candidates.lock().pop_front();
            let Some(page) = candidate.and_then(|candidate| candidate.upgrade()) else {
                continue;
            };
            if page.take_reference() || !self.try_reclaim(&page) {
                // A concurrent registration may have consumed the popped slot,
                // so re-reserve instead of allocating in the reclaim path. A
                // dropped candidate stays resident and is still freed by idle
                // pruning or discard.
                let mut candidates = self.candidates.lock();
                if candidates.try_reserve(1).is_ok() {
                    candidates.push_back(Arc::downgrade(&page));
                }
                continue;
            }
            reclaimed += 1;
        }
        reclaimed
    }

    fn try_reclaim(&self, page: &Arc<CachedPage>) -> bool {
        let Some(mapping) = page.mapping.upgrade() else {
            return true;
        };
        if mapping.try_reclaim(page) {
            self.resident_pages.fetch_sub(1, Ordering::Relaxed);
            self.prune_mapping(&mapping);
            true
        } else {
            false
        }
    }

    fn wait_for_work(&self, seen_epoch: &mut u64) -> bool {
        loop {
            let epoch = self.progress_epoch.load(Ordering::Acquire);
            {
                let state = self.lifecycle.lock();
                if state.lifecycle != Lifecycle::Running {
                    return false;
                }
                if *seen_epoch != epoch {
                    *seen_epoch = epoch;
                    return true;
                }
            }
            self.worker_wait.wait_until(|| {
                let state = self.lifecycle.lock();
                state.lifecycle != Lifecycle::Running
                    || self.progress_epoch.load(Ordering::Acquire) != epoch
            });
        }
    }

    /// Runs maintenance until [`Self::shutdown`] closes admission.
    pub fn run_worker(&self, available_pages: impl Fn() -> usize) {
        {
            let mut state = self.lifecycle.lock();
            if state.lifecycle != Lifecycle::Running {
                return;
            }
            assert!(!state.worker_running, "page-cache worker started twice");
            state.worker_running = true;
        }
        let mut seen_epoch = 0;
        while self.wait_for_work(&mut seen_epoch) {
            if let Err(error) = self.maintain(available_pages()) {
                log::warn!("page-cache maintenance failed: {error}");
            }
        }
        let result = self.shutdown_flush();
        let mut state = self.lifecycle.lock();
        state.worker_running = false;
        state.worker_result = Some(result);
        drop(state);
        self.active_wait.notify_all(false);
    }

    pub(crate) fn wake(&self) {
        self.progress_epoch.fetch_add(1, Ordering::AcqRel);
        self.worker_wait.notify_one(false);
    }

    pub fn shutdown(&self) -> LinuxResult {
        if !self.accepting_operations.swap(false, Ordering::AcqRel) {
            return Err(LinuxError::ESHUTDOWN);
        }
        let flush_here = {
            let mut state = self.lifecycle.lock();
            if state.lifecycle != Lifecycle::Running {
                return Err(LinuxError::ESHUTDOWN);
            }
            state.lifecycle = Lifecycle::Closing;
            !state.worker_running
        };
        self.worker_wait.notify_all(false);
        self.active_wait.notify_all(false);
        if flush_here {
            let result = self.shutdown_flush();
            self.lifecycle.lock().worker_result = Some(result);
            self.active_wait.notify_all(false);
        }
        self.active_wait.wait_until(|| {
            let state = self.lifecycle.lock();
            self.active_operations.load(Ordering::Acquire) == 0 && state.worker_result.is_some()
        });
        let mut state = self.lifecycle.lock();
        if state.lifecycle != Lifecycle::Closing {
            return Err(LinuxError::ESHUTDOWN);
        }
        let result = state
            .worker_result
            .take()
            .expect("worker completion was observed");
        state.lifecycle = Lifecycle::Closed;
        result
    }

    fn shutdown_flush(&self) -> LinuxResult {
        self.active_wait
            .wait_until(|| self.active_operations.load(Ordering::Acquire) == 0);
        let writeback = self.writeback_inner(usize::MAX, true);
        self.reclaim_clean(usize::MAX);
        writeback?;
        if self.resident_pages.load(Ordering::Acquire) != 0
            || self.dirty_pages.load(Ordering::Acquire) != 0
        {
            return Err(LinuxError::EBUSY);
        }
        Ok(())
    }

    pub(crate) fn permit(&self) -> LinuxResult<OperationPermit<'_>> {
        if !self.accepting_operations.load(Ordering::Acquire) {
            return Err(LinuxError::ESHUTDOWN);
        }
        self.active_operations.fetch_add(1, Ordering::AcqRel);
        if !self.accepting_operations.load(Ordering::Acquire) {
            if self.active_operations.fetch_sub(1, Ordering::AcqRel) == 1 {
                self.active_wait.notify_all(false);
            }
            return Err(LinuxError::ESHUTDOWN);
        }
        Ok(OperationPermit { manager: self })
    }

    pub(crate) fn register_candidate(&self, page: &Arc<CachedPage>) -> LinuxResult {
        let mut candidates = self.candidates.lock();
        candidates.try_reserve(1).map_err(|_| LinuxError::ENOMEM)?;
        candidates.push_back(Arc::downgrade(page));
        Ok(())
    }

    pub(crate) fn allocate_frame(&self) -> LinuxResult<Frame> {
        if let Some(frame) = Frame::allocate_zeroed() {
            return Ok(frame);
        }
        self.reclaim_clean(1);
        if let Some(frame) = Frame::allocate_zeroed() {
            return Ok(frame);
        }
        self.wake();
        Err(LinuxError::ENOMEM)
    }

    pub(crate) fn record_resident(&self) {
        let resident = self.resident_pages.fetch_add(1, Ordering::Relaxed) + 1;
        if resident.is_multiple_of(self.policy.writeback_batch_pages) {
            self.wake();
        }
    }

    pub(crate) fn remove_resident(&self, resident: usize, dirty: usize) {
        self.resident_pages.fetch_sub(resident, Ordering::Relaxed);
        self.dirty_pages.fetch_sub(dirty, Ordering::Relaxed);
    }

    /// Records one page becoming dirty and wakes the worker at the threshold.
    pub(crate) fn account_new_dirty(&self) {
        let dirty = self.dirty_pages.fetch_add(1, Ordering::Relaxed) + 1;
        if dirty >= self.policy.dirty_background {
            self.wake();
        }
    }

    pub(crate) fn account_clean(&self) {
        self.dirty_pages.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Drop for OperationPermit<'_> {
    fn drop(&mut self) {
        if self
            .manager
            .active_operations
            .fetch_sub(1, Ordering::AcqRel)
            == 1
            && !self.manager.accepting_operations.load(Ordering::Acquire)
        {
            self.manager.active_wait.notify_all(false);
        }
    }
}

impl Drop for CacheManager {
    fn drop(&mut self) {
        debug_assert_eq!(self.lifecycle.get_mut().lifecycle, Lifecycle::Closed);
    }
}
