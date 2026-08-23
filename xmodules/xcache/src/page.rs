// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use alloc::sync::{Arc, Weak};
use core::{
    any::Any,
    sync::atomic::{AtomicBool, Ordering},
};

use spin::Mutex;
use xerrno::{LinuxError, LinuxResult};
use xmm::Frame;
use xtask::WaitQueue;

use crate::mapping::FileMapping;

pub(crate) struct CachedPage {
    pub(crate) mapping: Weak<FileMapping>,
    pub(crate) index: u64,
    pub(crate) frame: Frame,
    pub(crate) state: Mutex<PageState>,
    pub(crate) wait: WaitQueue,
    pub(crate) referenced: AtomicBool,
}

#[derive(Debug)]
pub(crate) struct PageState {
    pub(crate) leases: u32,
    pub(crate) dirty_seq: u64,
    pub(crate) submitted_seq: u64,
    pub(crate) persisted_seq: u64,
    pub(crate) writeback_seq: Option<u64>,
    pub(crate) failed_seq: Option<u64>,
    pub(crate) shared_guard_groups: u32,
}

impl PageState {
    pub(crate) const fn clean() -> Self {
        Self {
            leases: 0,
            dirty_seq: 0,
            submitted_seq: 0,
            persisted_seq: 0,
            writeback_seq: None,
            failed_seq: None,
            shared_guard_groups: 0,
        }
    }

    pub(crate) fn mark_dirty(&mut self) -> u64 {
        self.dirty_seq = self
            .dirty_seq
            .checked_add(1)
            .expect("page dirty sequence exhausted");
        self.failed_seq = None;
        self.dirty_seq
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.persisted_seq < self.dirty_seq || self.shared_guard_groups != 0
    }

    pub(crate) fn reclaimable(&self) -> bool {
        !self.is_dirty()
            && self.writeback_seq.is_none()
            && self.leases == 0
            && self.shared_guard_groups == 0
    }

    /// Whether no transient owner is using the page right now.
    ///
    /// Unlike [`Self::reclaimable`] this ignores dirtiness, so it answers
    /// "has every in-flight user let go" rather than "may the data be dropped".
    pub(crate) fn is_quiet(&self) -> bool {
        self.leases == 0 && self.writeback_seq.is_none() && self.shared_guard_groups == 0
    }

    /// Decides what writeback owes this page, evaluated under `state`.
    pub(crate) fn writeback_action(&self, explicit: bool) -> WritebackAction {
        if !self.is_dirty() {
            return WritebackAction::Skip;
        }
        if self.writeback_seq.is_some() {
            // A concurrent writeback already owns the current data; only an
            // explicit sync must observe its outcome.
            return if explicit {
                WritebackAction::Wait
            } else {
                WritebackAction::Skip
            };
        }
        let retry_would_repeat = self.failed_seq == Some(self.dirty_seq);
        let guard_holds_data =
            self.shared_guard_groups != 0 && self.submitted_seq == self.dirty_seq;
        if !explicit && (retry_would_repeat || guard_holds_data) {
            return WritebackAction::Skip;
        }
        WritebackAction::Submit
    }
}

pub(crate) enum WritebackAction {
    Skip,
    Wait,
    Submit,
}

impl CachedPage {
    pub(crate) fn new(mapping: Weak<FileMapping>, index: u64, frame: Frame) -> Self {
        Self {
            mapping,
            index,
            frame,
            state: Mutex::new(PageState::clean()),
            wait: WaitQueue::new(),
            referenced: AtomicBool::new(true),
        }
    }

    pub(crate) fn mark_referenced(&self) {
        self.referenced.store(true, Ordering::Release);
    }

    pub(crate) fn take_reference(&self) -> bool {
        self.referenced.swap(false, Ordering::AcqRel)
    }
}

/// A counted transient user of one cached frame.
pub struct PageLease {
    pub(crate) page: Arc<CachedPage>,
}

impl PageLease {
    pub fn frame(&self) -> Frame {
        self.page.frame.clone()
    }

    pub fn shared_write_guard(&self) -> LinuxResult<Arc<dyn Any + Send + Sync>> {
        let mapping = self.page.mapping.upgrade().ok_or(LinuxError::ESHUTDOWN)?;
        let manager = mapping.manager_ref().ok_or(LinuxError::ESHUTDOWN)?;
        manager.throttle_dirty()?;
        let mut state = self.page.state.lock();
        let was_dirty = state.is_dirty();
        state.shared_guard_groups = state
            .shared_guard_groups
            .checked_add(1)
            .ok_or(LinuxError::EOVERFLOW)?;
        state.mark_dirty();
        drop(state);
        if !was_dirty {
            mapping.note_dirtied(&manager);
        }
        Ok(Arc::new(SharedWriteGuard {
            page: self.page.clone(),
        }))
    }
}

impl Drop for PageLease {
    fn drop(&mut self) {
        let mut state = self.page.state.lock();
        debug_assert_ne!(state.leases, 0);
        state.leases -= 1;
        drop(state);
        if let Some(mapping) = self.page.mapping.upgrade()
            && mapping.is_invalidating()
        {
            mapping.notify_drain();
        }
    }
}

/// One writable shared-mapping group retained across fork clones.
struct SharedWriteGuard {
    page: Arc<CachedPage>,
}

impl Drop for SharedWriteGuard {
    fn drop(&mut self) {
        let mut state = self.page.state.lock();
        debug_assert_ne!(state.shared_guard_groups, 0);
        state.shared_guard_groups -= 1;
        state.mark_dirty();
        drop(state);
        if let Some(mapping) = self.page.mapping.upgrade() {
            if let Some(manager) = mapping.manager_ref() {
                manager.wake();
            }
            if mapping.is_invalidating() {
                mapping.notify_drain();
            }
        }
    }
}
