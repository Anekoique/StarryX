// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

//! Adapters binding the page cache to xvma.
//!
//! Two independent concerns live here: [`FileVmObject`] supplies pages to any
//! address space that maps the file, and [`MappedFiles`] tells the cache which
//! address spaces must drop leaves when a file shrinks.

use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::ops::Range;

use xcache::{FileMapping, InvalidationObserver, ObserverRegistration};
use xerrno::{LinuxError, LinuxResult};
use xsync::Mutex;
use xvma::{VmObject, VmPage, VmSpace};

use crate::fs::cache::CachedMapping;

/// Supplies cached pages of one file to every address space mapping it.
pub(crate) struct FileVmObject(CachedMapping);

impl FileVmObject {
    pub(crate) fn new(mapping: Arc<FileMapping>) -> Arc<Self> {
        Arc::new(Self(CachedMapping::new(mapping)))
    }

    fn mapping(&self) -> &Arc<FileMapping> {
        self.0.mapping()
    }
}

impl VmObject for FileVmObject {
    fn id(&self) -> u64 {
        self.mapping().id()
    }

    fn byte_len(&self) -> LinuxResult<u64> {
        Ok(self.mapping().size())
    }

    fn page(&self, index: u64, write: bool) -> LinuxResult<VmPage> {
        let lease = self.mapping().acquire_page(index)?;
        let guard = if write {
            Some(lease.shared_write_guard()?)
        } else {
            None
        };
        Ok(VmPage {
            frame: lease.frame(),
            guard,
        })
    }

    fn sync(&self, range: Range<u64>, wait: bool) -> LinuxResult {
        self.0.sync_range(range, false, wait)
    }

    /// A writable PTE must be preceded by dirty accounting, which only a
    /// shared write guard performs.
    fn requires_write_guard(&self) -> bool {
        true
    }
}

/// The cached files one address space maps, holding its invalidation
/// registrations for exactly as long as the mappings live.
///
/// Address spaces sharing a `VmSpace` share one instance; a forked space gets
/// its own so a truncation reaches both.
pub(crate) struct MappedFiles {
    space: Weak<Mutex<VmSpace>>,
    /// Keyed by cache-mapping id. The value keeps the mapping alive so the
    /// observer registration cannot outlive what it observes.
    registrations: Mutex<BTreeMap<u64, (Arc<FileMapping>, ObserverRegistration)>>,
}

impl MappedFiles {
    pub(crate) fn new(space: &Arc<Mutex<VmSpace>>) -> Arc<Self> {
        Arc::new(Self {
            space: Arc::downgrade(space),
            registrations: Mutex::new(BTreeMap::new()),
        })
    }

    /// Reproduces these registrations for a forked address space.
    pub(crate) fn fork(&self, space: &Arc<Mutex<VmSpace>>) -> LinuxResult<Arc<Self>> {
        let files = Self::new(space);
        let inherited: Vec<_> = self
            .registrations
            .lock()
            .values()
            .map(|(mapping, _)| mapping.clone())
            .collect();
        for mapping in inherited {
            files.attach(&mapping)?;
        }
        Ok(files)
    }

    /// Subscribes this address space to `mapping`'s invalidations.
    ///
    /// Idempotent: a file mapped many times registers once.
    pub(crate) fn attach(&self, mapping: &Arc<FileMapping>) -> LinuxResult {
        let id = mapping.id();
        if self.registrations.lock().contains_key(&id) {
            return Ok(());
        }
        let observer: Arc<dyn InvalidationObserver> = Arc::new(SpaceInvalidator {
            id,
            space: self.space.clone(),
        });
        // Registration goes through the cache's admission gate, so it cannot
        // interleave with a truncation of the same file.
        let registration = mapping.register_observer(observer)?;
        let mut registrations = self.registrations.lock();
        registrations.insert(id, (mapping.clone(), registration));
        Ok(())
    }

    /// Drops registrations for files `space` no longer maps.
    ///
    /// Takes the caller's live borrow rather than the lock, because every
    /// unmap path already holds it.
    pub(crate) fn prune(&self, space: &VmSpace) {
        self.registrations
            .lock()
            .retain(|id, _| space.maps_object(*id));
    }
}

/// One address space's view of one file's invalidations.
struct SpaceInvalidator {
    id: u64,
    space: Weak<Mutex<VmSpace>>,
}

impl InvalidationObserver for SpaceInvalidator {
    fn validate(&self, range: &Range<u64>) -> LinuxResult {
        let Some(space) = self.space.upgrade() else {
            return Ok(());
        };
        space
            .lock()
            .validate_object_range(self.id, range)
            .map_err(LinuxError::from)
    }

    fn invalidate(&self, range: &Range<u64>) {
        let Some(space) = self.space.upgrade() else {
            return;
        };
        space.lock().unmap_object_range(self.id, range);
    }
}
