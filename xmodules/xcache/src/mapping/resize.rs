// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

//! Truncation and the two-phase invalidation it drives.
//!
//! Shrinking must remove every user mapping of the vanished tail before the
//! backing loses the data. Observers are therefore asked to validate first —
//! the only phase allowed to fail — and only then to invalidate.

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{ops::Range, sync::atomic::Ordering};

use xerrno::{LinuxError, LinuxResult};

use super::{FileMapping, PageSlot};
use crate::{PAGE_SIZE, page_index, page_offset};

pub trait InvalidationObserver: Send + Sync {
    /// Validates that `range` can be invalidated without changing state.
    fn validate(&self, range: &Range<u64>) -> LinuxResult;

    /// Invalidates a range accepted by [`Self::validate`] without failure.
    fn invalidate(&self, range: &Range<u64>);
}

pub struct ObserverRegistration {
    mapping: Weak<FileMapping>,
    observer_id: u64,
    _observer: Arc<dyn InvalidationObserver>,
}

impl Drop for ObserverRegistration {
    fn drop(&mut self) {
        if let Some(mapping) = self.mapping.upgrade() {
            mapping.observers.lock().remove(&self.observer_id);
            mapping.notify_drain();
        }
    }
}

impl PageSlot {
    /// Resize-drain variant of [`Self::is_idle`]: shared guards are released by
    /// observer invalidation before the commit-side wait, so this deliberately does
    /// not require `shared_guard_groups == 0` and cannot deadlock against a
    /// live guard.
    fn is_idle_for_resize(&self) -> bool {
        self.resident().is_some_and(|page| {
            let state = page.state.lock();
            state.leases == 0 && state.writeback_seq.is_none() && page.frame.is_unique()
        })
    }
}

impl FileMapping {
    pub fn register_observer(
        self: &Arc<Self>,
        observer: Arc<dyn InvalidationObserver>,
    ) -> LinuxResult<ObserverRegistration> {
        let manager = self.manager()?;
        let _permit = manager.permit()?;
        let _operation = self.enter_operation()?;
        let id = self
            .next_observer_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
            .map_err(|_| LinuxError::ENOSPC)?;
        self.observers.lock().insert(id, &observer);
        Ok(ObserverRegistration {
            mapping: Arc::downgrade(self),
            observer_id: id,
            _observer: observer,
        })
    }

    pub fn resize(self: &Arc<Self>, new_len: u64) -> LinuxResult {
        let manager = self.manager()?;
        let _permit = manager.permit()?;
        if !self.accepting_operations.swap(false, Ordering::AcqRel) {
            return Err(LinuxError::EAGAIN);
        }

        self.drain_wait
            .try_wait_until(|| self.active_operations.load(Ordering::Acquire) == 0)
            .map_err(|_| LinuxError::ENOMEM)
            .inspect_err(|_| self.reopen_admission())?;

        // Operations admitted before the gate closed may have extended the
        // file, so the authoritative old length is sampled only after drain.
        let old_len = self.size();
        self.drain_wait
            .try_reserve(1)
            .map_err(|_| LinuxError::ENOMEM)
            .inspect_err(|_| self.reopen_admission())?;
        let result = self.resize_inactive(old_len, new_len);
        if result.is_err() {
            self.reopen_admission();
        }
        result
    }

    /// Resizes while admission is closed and every prior operation has drained.
    fn resize_inactive(&self, old_len: u64, new_len: u64) -> LinuxResult {
        if new_len < old_len {
            let observers = self.prepare_invalidations(new_len..old_len)?;
            for observer in observers {
                observer.invalidate(&(new_len..old_len));
            }
            // resize admission permits only this waiter. Its queue slot was
            // reserved before any observer removed a PTE, so this commit-side
            // wait cannot allocate or fail.
            self.drain_wait
                .wait_until(|| self.range_is_drained(new_len, old_len));
        }

        self.backing.set_len(new_len)?;
        if new_len < old_len {
            self.commit_shrink(new_len);
        }
        self.logical_size.store(new_len, Ordering::Release);
        self.reopen_admission();
        Ok(())
    }

    fn prepare_invalidations(
        &self,
        range: Range<u64>,
    ) -> LinuxResult<Vec<Arc<dyn InvalidationObserver>>> {
        let registry = self.observers.lock();
        let mut observers = Vec::new();
        observers
            .try_reserve(registry.len())
            .map_err(|_| LinuxError::ENOMEM)?;
        observers.extend(registry.values());
        drop(registry);
        for observer in &observers {
            observer.validate(&range)?;
        }
        Ok(observers)
    }

    fn range_is_drained(&self, start: u64, end: u64) -> bool {
        let first = page_index(start);
        let last = page_index(end.saturating_sub(1));
        self.pages
            .lock()
            .range(first..=last)
            .all(|(_, slot)| slot.is_idle_for_resize())
    }

    /// Drops the truncated tail once no user mapping can observe it.
    fn commit_shrink(&self, new_len: u64) {
        let first_removed = new_len.div_ceil(PAGE_SIZE as u64);
        let tail = page_offset(new_len);
        let tail_index = page_index(new_len);
        let mut pages = self.pages.lock();
        if tail != 0
            && let Some(PageSlot::Resident(page)) = pages.get(&tail_index)
        {
            let zeroes = [0_u8; PAGE_SIZE];
            page.frame
                .write_bytes(tail, &zeroes[..PAGE_SIZE - tail])
                .expect("tail-zero range is page bounded");
        }
        let removed = pages.split_off(&first_removed);
        if let Some(manager) = self.manager.upgrade() {
            let (resident, dirty) = FileMapping::tally(removed);
            self.dirty_pages.fetch_sub(dirty, Ordering::Relaxed);
            manager.remove_resident(resident, dirty);
        }
    }

    fn reopen_admission(&self) {
        self.accepting_operations.store(true, Ordering::Release);
        self.drain_wait.notify_all(false);
    }
}
