// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

mod resize;
mod writeback;

use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use core::{
    ops::Range,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use spin::Mutex;
use weak_map::WeakMap;
use xerrno::{LinuxError, LinuxResult};
use xsync::Mutex as SleepMutex;
use xtask::WaitQueue;

pub use resize::{InvalidationObserver, ObserverRegistration};
pub use writeback::WritebackCursor;

use crate::{
    Backing, CacheManager, PAGE_SIZE,
    page::{CachedPage, PageLease},
    page_index, page_offset,
};

/// Splits `length` bytes at `offset` into per-page steps of
/// `(page index, offset within the page, byte range within the buffer)`.
fn page_chunks(offset: u64, length: usize) -> impl Iterator<Item = (u64, usize, Range<usize>)> {
    let mut done = 0;
    core::iter::from_fn(move || {
        (done < length).then(|| {
            let position = offset + done as u64;
            let in_page = page_offset(position);
            let chunk = done..done + (PAGE_SIZE - in_page).min(length - done);
            done = chunk.end;
            (page_index(position), in_page, chunk)
        })
    })
}

enum PageSlot {
    Loading(Arc<LoadAttempt>),
    Resident(Arc<CachedPage>),
}

impl PageSlot {
    fn resident(&self) -> Option<&Arc<CachedPage>> {
        match self {
            Self::Resident(page) => Some(page),
            Self::Loading(_) => None,
        }
    }

    /// Whether no transient owner and no PTE clone still reference the page.
    fn is_idle(&self) -> bool {
        self.resident()
            .is_some_and(|page| page.state.lock().is_quiet() && page.frame.is_unique())
    }

    /// Whether the page may be dropped outright: idle and holding no data the
    /// backing has not persisted.
    fn is_reclaimable(&self) -> bool {
        self.resident()
            .is_some_and(|page| page.state.lock().reclaimable() && page.frame.is_unique())
    }
}

struct LoadAttempt {
    page_index: u64,
    result: Mutex<Option<LinuxResult<Arc<CachedPage>>>>,
    wait: WaitQueue,
}

impl LoadAttempt {
    fn new(page_index: u64) -> Self {
        Self {
            page_index,
            result: Mutex::new(None),
            wait: WaitQueue::new(),
        }
    }

    fn publish(&self, result: LinuxResult<Arc<CachedPage>>) {
        let mut terminal = self.result.lock();
        if terminal.is_none() {
            *terminal = Some(result);
        }
        drop(terminal);
        self.wait.notify_all(false);
    }
}

struct LoadOwner {
    mapping: Weak<FileMapping>,
    attempt: Arc<LoadAttempt>,
    finished: bool,
}

impl LoadOwner {
    fn finish(&mut self, result: LinuxResult<Arc<CachedPage>>) {
        if let Some(mapping) = self.mapping.upgrade() {
            mapping.finish_load(&self.attempt, result);
        } else {
            self.attempt.publish(Err(LinuxError::EAGAIN));
        }
        self.finished = true;
    }
}

impl Drop for LoadOwner {
    fn drop(&mut self) {
        if !self.finished {
            self.finish(Err(LinuxError::EAGAIN));
        }
    }
}

pub struct FileMapping {
    id: u64,
    manager: Weak<CacheManager>,
    backing: Arc<dyn Backing>,
    logical_size: AtomicU64,
    accepting_operations: AtomicBool,
    active_operations: AtomicUsize,
    append_lock: SleepMutex<()>,
    pages: Mutex<BTreeMap<u64, PageSlot>>,
    /// Dirty pages in this mapping, letting writeback skip clean mappings
    /// without touching their page trees.
    dirty_pages: AtomicUsize,
    /// Where the next background writeback batch resumes scanning.
    writeback_cursor: AtomicU64,
    observers: Mutex<WeakMap<u64, Weak<dyn InvalidationObserver>>>,
    next_observer_id: AtomicU64,
    errors: Mutex<(u64, Option<LinuxError>)>,
    drain_wait: WaitQueue,
}

struct MappingOperation<'a> {
    mapping: &'a FileMapping,
}

impl FileMapping {
    pub(crate) fn new(
        id: u64,
        manager: Weak<CacheManager>,
        backing: Arc<dyn Backing>,
        logical_size: u64,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            manager,
            backing,
            logical_size: AtomicU64::new(logical_size),
            accepting_operations: AtomicBool::new(true),
            active_operations: AtomicUsize::new(0),
            append_lock: SleepMutex::new(()),
            pages: Mutex::new(BTreeMap::new()),
            dirty_pages: AtomicUsize::new(0),
            writeback_cursor: AtomicU64::new(0),
            observers: Mutex::new(WeakMap::new()),
            next_observer_id: AtomicU64::new(1),
            errors: Mutex::new((0, None)),
            drain_wait: WaitQueue::new(),
        })
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Hints that one external owner is about to disappear.
    pub fn release_hint(&self) {
        if let Some(manager) = self.manager.upgrade() {
            manager.prune_mapping(self);
        }
    }

    pub fn size(&self) -> u64 {
        self.logical_size.load(Ordering::Acquire)
    }

    pub fn read_at(self: &Arc<Self>, destination: &mut [u8], offset: u64) -> LinuxResult<usize> {
        let manager = self.manager()?;
        let _permit = manager.permit()?;
        let _operation = self.enter_operation()?;
        let length = self.size();
        if destination.is_empty() || offset >= length {
            return Ok(0);
        }
        let count = destination.len().min((length - offset) as usize);
        for (index, in_page, chunk) in page_chunks(offset, count) {
            let lease = self.acquire_page_inner(index, false, &manager)?;
            lease
                .page
                .frame
                .read_bytes(in_page, &mut destination[chunk])
                .map_err(LinuxError::from)?;
        }
        Ok(count)
    }

    pub fn write_at(self: &Arc<Self>, source: &[u8], offset: u64) -> LinuxResult<usize> {
        let manager = self.manager()?;
        let _permit = manager.permit()?;
        let _operation = self.enter_operation()?;
        self.write_at_inner(source, offset, &manager)
    }

    fn write_at_inner(
        self: &Arc<Self>,
        source: &[u8],
        offset: u64,
        manager: &Arc<CacheManager>,
    ) -> LinuxResult<usize> {
        offset
            .checked_add(source.len() as u64)
            .ok_or(LinuxError::EFBIG)?;
        let mut copied = 0;
        for (index, in_page, chunk) in page_chunks(offset, source.len()) {
            let step = manager.throttle_dirty().and_then(|()| {
                let full_page = in_page == 0 && chunk.len() == PAGE_SIZE;
                let lease = self.acquire_page_inner(index, full_page, manager)?;
                lease
                    .page
                    .frame
                    .write_bytes(in_page, &source[chunk.clone()])
                    .map_err(LinuxError::from)?;
                self.mark_dirty(&lease.page, manager);
                Ok(())
            });
            match step {
                Ok(()) => copied = chunk.end,
                Err(error) if copied == 0 => return Err(error),
                Err(_) => break,
            }
        }
        self.logical_size
            .fetch_max(offset + copied as u64, Ordering::AcqRel);
        Ok(copied)
    }

    pub fn append(self: &Arc<Self>, source: &[u8]) -> LinuxResult<(usize, u64)> {
        let manager = self.manager()?;
        let _permit = manager.permit()?;
        let _append = self.append_lock.lock();
        let _operation = self.enter_operation()?;
        let offset = self.size();
        let written = self.write_at_inner(source, offset, &manager)?;
        Ok((written, offset + written as u64))
    }

    pub fn acquire_page(self: &Arc<Self>, index: u64) -> LinuxResult<PageLease> {
        let manager = self.manager()?;
        let _permit = manager.permit()?;
        let _operation = self.enter_operation()?;
        self.acquire_page_inner(index, false, &manager)
    }

    fn acquire_page_inner(
        self: &Arc<Self>,
        index: u64,
        zero_on_miss: bool,
        manager: &Arc<CacheManager>,
    ) -> LinuxResult<PageLease> {
        loop {
            let (attempt, winner) = {
                let mut pages = self.pages.lock();
                match pages.get(&index) {
                    Some(PageSlot::Resident(page)) => {
                        page.mark_referenced();
                        return Self::lease_locked(page);
                    }
                    Some(PageSlot::Loading(attempt)) => (attempt.clone(), false),
                    None => {
                        let attempt = Arc::new(LoadAttempt::new(index));
                        pages.insert(index, PageSlot::Loading(attempt.clone()));
                        (attempt, true)
                    }
                }
            };
            if winner {
                let mut owner = LoadOwner {
                    mapping: Arc::downgrade(self),
                    attempt: attempt.clone(),
                    finished: false,
                };
                let result = self.load_page(index, zero_on_miss, manager);
                owner.finish(result);
            } else {
                attempt
                    .wait
                    .try_wait_until(|| attempt.result.lock().is_some())
                    .map_err(|_| LinuxError::ENOMEM)?;
            }
            match attempt.result.lock().as_ref().cloned() {
                Some(Ok(_)) => continue,
                Some(Err(error)) => return Err(error),
                None => return Err(LinuxError::EAGAIN),
            }
        }
    }

    fn lease_locked(page: &Arc<CachedPage>) -> LinuxResult<PageLease> {
        let mut state = page.state.lock();
        state.leases = state.leases.checked_add(1).ok_or(LinuxError::EOVERFLOW)?;
        drop(state);
        Ok(PageLease { page: page.clone() })
    }

    fn load_page(
        self: &Arc<Self>,
        index: u64,
        zero_on_miss: bool,
        manager: &Arc<CacheManager>,
    ) -> LinuxResult<Arc<CachedPage>> {
        let mut frame = manager.allocate_frame()?;
        if !zero_on_miss {
            let offset = index
                .checked_mul(PAGE_SIZE as u64)
                .ok_or(LinuxError::EFBIG)?;
            let length = self.size();
            if offset < length {
                let count = PAGE_SIZE.min((length - offset) as usize);
                let mut data = [0_u8; PAGE_SIZE];
                Self::transfer_fully(count, true, |done| {
                    self.backing
                        .read_at(offset + done as u64, &mut data[done..count])
                })?;
                if !frame.try_write_at(0, &data) {
                    return Err(LinuxError::EIO);
                }
            }
        }
        let page = Arc::new(CachedPage::new(Arc::downgrade(self), index, frame));
        manager.register_candidate(&page)?;
        Ok(page)
    }

    fn finish_load(&self, attempt: &Arc<LoadAttempt>, result: LinuxResult<Arc<CachedPage>>) {
        let mut pages = self.pages.lock();
        let is_current = matches!(
            pages.get(&attempt.page_index),
            Some(PageSlot::Loading(current)) if Arc::ptr_eq(current, attempt)
        );
        let result = if is_current {
            result
        } else {
            Err(LinuxError::EAGAIN)
        };
        attempt.publish(result.clone());
        if is_current {
            match result {
                Ok(page) => {
                    pages.insert(attempt.page_index, PageSlot::Resident(page));
                    if let Some(manager) = self.manager.upgrade() {
                        manager.record_resident();
                    }
                }
                Err(_) => {
                    pages.remove(&attempt.page_index);
                }
            }
        }
    }

    fn mark_dirty(&self, page: &CachedPage, manager: &CacheManager) {
        let mut state = page.state.lock();
        let was_dirty = state.is_dirty();
        state.mark_dirty();
        drop(state);
        if !was_dirty {
            self.note_dirtied(manager);
        }
    }

    /// Records one page's clean-to-dirty transition.
    pub(crate) fn note_dirtied(&self, manager: &CacheManager) {
        self.dirty_pages.fetch_add(1, Ordering::Relaxed);
        manager.account_new_dirty();
    }

    /// Records one page's dirty-to-clean transition.
    pub(crate) fn note_cleaned(&self, manager: &CacheManager) {
        self.dirty_pages.fetch_sub(1, Ordering::Relaxed);
        manager.account_clean();
    }

    /// Drives `step(done)` until `count` bytes transfer. Zero progress ends a
    /// read short (the untouched tail stays zeroed) but fails a write; overrun
    /// is backing corruption either way.
    fn transfer_fully(
        count: usize,
        allow_short: bool,
        mut step: impl FnMut(usize) -> LinuxResult<usize>,
    ) -> LinuxResult {
        let mut done = 0;
        while done < count {
            let bytes = step(done)?;
            if bytes == 0 {
                return if allow_short {
                    Ok(())
                } else {
                    Err(LinuxError::EIO)
                };
            }
            done = done.checked_add(bytes).ok_or(LinuxError::EIO)?;
            if done > count {
                return Err(LinuxError::EIO);
            }
        }
        Ok(())
    }

    fn manager(&self) -> LinuxResult<Arc<CacheManager>> {
        self.manager.upgrade().ok_or(LinuxError::ESHUTDOWN)
    }

    fn enter_operation(&self) -> LinuxResult<MappingOperation<'_>> {
        if !self.accepting_operations.load(Ordering::Acquire) {
            return Err(LinuxError::EAGAIN);
        }
        self.active_operations.fetch_add(1, Ordering::AcqRel);
        if !self.accepting_operations.load(Ordering::Acquire) {
            if self.active_operations.fetch_sub(1, Ordering::AcqRel) == 1 {
                self.notify_drain();
            }
            return Err(LinuxError::EAGAIN);
        }
        Ok(MappingOperation { mapping: self })
    }

    pub(crate) fn try_reclaim(&self, page: &Arc<CachedPage>) -> bool {
        let mut pages = self.pages.lock();
        let Some(PageSlot::Resident(current)) = pages.get(&page.index) else {
            return false;
        };
        if !Arc::ptr_eq(current, page) {
            return false;
        }
        let state = page.state.lock();
        if !state.reclaimable() || !page.frame.is_unique() {
            return false;
        }
        drop(state);
        pages.remove(&page.index);
        true
    }

    pub(crate) fn has_no_pages(&self) -> bool {
        self.pages.lock().is_empty()
    }

    /// Drops clean pages after the last external mapping owner disappears.
    pub(crate) fn reclaim_idle_pages(&self) -> usize {
        let mut reclaimed = 0;
        self.pages.lock().retain(|_, slot| {
            let keep = !slot.is_reclaimable();
            reclaimed += usize::from(!keep);
            keep
        });
        reclaimed
    }

    pub(crate) fn discard_unowned_pages(&self) -> Option<(usize, usize)> {
        if self.active_operations.load(Ordering::Acquire) != 0 || self.is_invalidating() {
            return None;
        }
        let mut pages = self.pages.lock();
        if !pages.values().all(PageSlot::is_idle) {
            return None;
        }
        let (resident, dirty) = Self::tally(core::mem::take(&mut *pages));
        self.dirty_pages.fetch_sub(dirty, Ordering::Relaxed);
        Some((resident, dirty))
    }

    /// Counts resident and dirty pages in a set of removed slots.
    fn tally(removed: BTreeMap<u64, PageSlot>) -> (usize, usize) {
        let mut resident = 0;
        let mut dirty = 0;
        for slot in removed.into_values() {
            if let Some(page) = slot.resident() {
                resident += 1;
                dirty += usize::from(page.state.lock().is_dirty());
            }
        }
        (resident, dirty)
    }

    pub(crate) fn notify_drain(&self) {
        self.drain_wait.notify_all(false);
    }

    pub(crate) fn is_invalidating(&self) -> bool {
        !self.accepting_operations.load(Ordering::Acquire)
    }

    pub(crate) fn manager_ref(&self) -> Option<Arc<CacheManager>> {
        self.manager.upgrade()
    }
}

impl Drop for MappingOperation<'_> {
    fn drop(&mut self) {
        if self
            .mapping
            .active_operations
            .fetch_sub(1, Ordering::AcqRel)
            == 1
            && self.mapping.is_invalidating()
        {
            self.mapping.notify_drain();
        }
    }
}
