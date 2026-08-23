// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

//! Persisting dirty pages and reporting the outcome to explicit syncs.

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::{ops::Range, sync::atomic::Ordering};

use xerrno::{LinuxError, LinuxResult};

use super::FileMapping;
use crate::{
    CacheManager, PAGE_SIZE,
    page::{CachedPage, WritebackAction},
    page_index,
};

/// One observer's position in a mapping's writeback error history.
///
/// Each explicit sync reports an error at most once, so a failure cannot be
/// swallowed by an earlier reader nor replayed to every later one.
pub struct WritebackCursor {
    seen_sequence: u64,
}

impl FileMapping {
    pub fn new_cursor(&self) -> WritebackCursor {
        WritebackCursor {
            seen_sequence: self.errors.lock().0,
        }
    }

    pub fn sync_range(
        &self,
        range: Range<u64>,
        data_only: bool,
        wait: bool,
        cursor: &mut WritebackCursor,
    ) -> LinuxResult {
        let manager = self.manager()?;
        let _permit = manager.permit()?;
        if !wait {
            manager.request_writeback();
            return Ok(());
        }
        let _operation = self.enter_operation()?;
        let pages = self.resident_pages(range)?;
        let mut buffer = Box::try_new([0_u8; PAGE_SIZE]).map_err(|_| LinuxError::ENOMEM)?;
        let mut first_error = None;
        for page in pages {
            if let Err(error) = self.writeback_page(&page, true, &manager, &mut buffer) {
                first_error.get_or_insert(error);
            }
        }
        if first_error.is_none() {
            if let Err(error) = self.backing.set_len(self.size()) {
                self.record_error(error);
                first_error = Some(error);
            } else if let Err(error) = self.backing.sync(data_only) {
                self.record_error(error);
                first_error = Some(error);
            }
        }
        self.consume_error(cursor)
            .map_err(|error| first_error.unwrap_or(error))
    }

    pub(crate) fn writeback_some(&self, max_pages: usize, explicit: bool) -> LinuxResult<usize> {
        if max_pages == 0 {
            return Ok(0);
        }
        // A clean mapping costs one atomic load; background batches resume at
        // a cursor so a large dirty file drains without rescanning its head.
        if !explicit && self.dirty_pages.load(Ordering::Acquire) == 0 {
            return Ok(0);
        }
        let _operation = match self.enter_operation() {
            Ok(operation) => operation,
            // A mapping mid-resize made no progress; the next batch retries
            // it. Erroring here would abort the whole batch and surface as a
            // spurious write(2) failure through the dirty throttle.
            Err(_) if !explicit => return Ok(0),
            Err(error) => return Err(error),
        };
        let pages = if explicit {
            self.resident_pages(0..self.size())?
        } else {
            self.dirty_batch(max_pages)?
        };
        let manager = self.manager()?;
        let mut buffer = Box::try_new([0_u8; PAGE_SIZE]).map_err(|_| LinuxError::ENOMEM)?;
        let mut written = 0;
        for page in pages {
            if written == max_pages {
                break;
            }
            written += usize::from(self.writeback_page(&page, explicit, &manager, &mut buffer)?);
        }
        Ok(written)
    }

    /// Collects up to `max_pages` dirty pages, resuming at the stored cursor
    /// and wrapping once, so successive batches cover the whole mapping.
    fn dirty_batch(&self, max_pages: usize) -> LinuxResult<Vec<Arc<CachedPage>>> {
        let limit = max_pages.min(self.dirty_pages.load(Ordering::Acquire));
        let mut batch = Vec::new();
        batch.try_reserve(limit).map_err(|_| LinuxError::ENOMEM)?;
        let cursor = self.writeback_cursor.load(Ordering::Acquire);
        let mut next = 0;
        let pages = self.pages.lock();
        for (index, slot) in pages.range(cursor..).chain(pages.range(..cursor)) {
            if batch.len() == limit {
                next = *index;
                break;
            }
            if let Some(page) = slot.resident()
                && page.state.lock().is_dirty()
            {
                batch.push(page.clone());
            }
        }
        drop(pages);
        self.writeback_cursor.store(next, Ordering::Release);
        Ok(batch)
    }

    fn resident_pages(&self, range: Range<u64>) -> LinuxResult<Vec<Arc<CachedPage>>> {
        if range.start >= range.end {
            return Ok(Vec::new());
        }
        let first = page_index(range.start);
        let last = page_index(range.end - 1);
        let pages = self.pages.lock();
        let mut resident = Vec::new();
        resident
            .try_reserve(pages.range(first..=last).count())
            .map_err(|_| LinuxError::ENOMEM)?;
        resident.extend(
            pages
                .range(first..=last)
                .filter_map(|(_, slot)| slot.resident().cloned()),
        );
        Ok(resident)
    }

    fn writeback_page(
        &self,
        page: &Arc<CachedPage>,
        explicit: bool,
        manager: &CacheManager,
        bytes: &mut [u8; PAGE_SIZE],
    ) -> LinuxResult<bool> {
        loop {
            // The snapshot must be taken under the same lock acquisition that
            // claims the sequence, so no write can slip between them.
            let page_seq = {
                let mut state = page.state.lock();
                match state.writeback_action(explicit) {
                    WritebackAction::Skip => return Ok(false),
                    WritebackAction::Wait => {
                        drop(state);
                        page.wait
                            .try_wait_until(|| page.state.lock().writeback_seq.is_none())
                            .map_err(|_| LinuxError::ENOMEM)?;
                        continue;
                    }
                    WritebackAction::Submit => {}
                }
                page.frame
                    .read_bytes(0, bytes.as_mut_slice())
                    .map_err(LinuxError::from)?;
                let sequence = state.dirty_seq;
                state.submitted_seq = sequence;
                state.writeback_seq = Some(sequence);
                sequence
            };
            let result = self.write_snapshot(page.index, bytes);
            let mut state = page.state.lock();
            let was_dirty = state.is_dirty();
            state.writeback_seq = None;
            match result {
                Ok(()) => {
                    state.persisted_seq = state.persisted_seq.max(page_seq);
                    if state.failed_seq.is_some_and(|failed| failed <= page_seq) {
                        state.failed_seq = None;
                    }
                }
                Err(error) => {
                    self.record_error(error);
                    state.failed_seq = Some(page_seq);
                }
            }
            let is_dirty = state.is_dirty();
            drop(state);
            if was_dirty && !is_dirty {
                self.note_cleaned(manager);
            }
            page.wait.notify_all(false);
            self.notify_drain();
            return result.map(|()| true);
        }
    }

    /// Writes back the part of one page that still lies inside the file.
    fn write_snapshot(&self, page_index: u64, bytes: &[u8; PAGE_SIZE]) -> LinuxResult {
        let offset = page_index
            .checked_mul(PAGE_SIZE as u64)
            .ok_or(LinuxError::EFBIG)?;
        let length = self.size();
        if offset >= length {
            return Ok(());
        }
        let count = PAGE_SIZE.min((length - offset) as usize);
        Self::transfer_fully(count, false, |done| {
            self.backing
                .write_at(offset + done as u64, &bytes[done..count])
        })
    }

    pub(super) fn record_error(&self, error: LinuxError) {
        let mut state = self.errors.lock();
        state.0 = state
            .0
            .checked_add(1)
            .expect("writeback error sequence exhausted");
        state.1 = Some(error);
    }

    fn consume_error(&self, cursor: &mut WritebackCursor) -> LinuxResult {
        let state = self.errors.lock();
        if state.0 == cursor.seen_sequence {
            return Ok(());
        }
        cursor.seen_sequence = state.0;
        Err(state.1.unwrap_or(LinuxError::EIO))
    }
}
