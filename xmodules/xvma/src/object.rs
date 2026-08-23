// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::{
    any::Any,
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};
use xerrno::{LinuxError, LinuxResult, XError, XResult};
use xmm::{Frame, PageSize};

/// An opaque owner kept alive for as long as a page stays writably mapped.
///
/// xvma never inspects it; dropping it is what tells the page provider that
/// this address space no longer holds the page writable.
pub type VmPageGuard = Arc<dyn Any + Send + Sync>;

/// One page supplied by a [`VmObject`].
pub struct VmPage {
    pub frame: Frame,
    pub guard: Option<VmPageGuard>,
}

/// The single allocator behind every [`VmObject::id`].
///
/// All object kinds draw from one boot-global, never-reused counter, so two
/// objects can never collide regardless of what backs them.
static NEXT_OBJECT_ID: AtomicU64 = AtomicU64::new(1);

/// Allocates a boot-global identity for one [`VmObject`].
pub fn allocate_object_id() -> XResult<u64> {
    NEXT_OBJECT_ID
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
        .map_err(|_| XError::NoMemory)
}

/// The page source of a mapped region.
///
/// One implementation serves every non-anonymous mapping: a cached file, an
/// anonymous shared region, or System V shared memory. Implementations must not
/// re-enter the faulting address space from any of these methods.
pub trait VmObject: Send + Sync {
    /// Globally unique identity, used for VMA merging and invalidation routing.
    fn id(&self) -> u64;

    fn byte_len(&self) -> LinuxResult<u64>;

    /// Supplies the stable page at `index`, writable in the shared sense when
    /// `write`.
    ///
    /// While any returned frame or guard is alive, later calls for the same
    /// object and index must name the same physical frame. A private write never
    /// asks for `write`: it copies the frame instead.
    fn page(&self, index: u64, write: bool) -> LinuxResult<VmPage>;

    fn sync(&self, _range: Range<u64>, _wait: bool) -> LinuxResult {
        Ok(())
    }

    /// Whether a shared mapping may only become writable through a guard.
    ///
    /// A page cache says yes: a writable PTE must first be accounted as dirty.
    /// Objects that own their frames outright say no and stay writable.
    fn requires_write_guard(&self) -> bool {
        false
    }
}

/// Frame ownership for an anonymous shared region.
pub struct SharedObject {
    id: u64,
    frames: Box<[Frame]>,
}

impl SharedObject {
    pub fn new(size: usize) -> XResult<Arc<Self>> {
        let unit = usize::from(PageSize::Size4K);
        if !size.is_multiple_of(unit) {
            return Err(XError::InvalidInput);
        }
        let frames = (0..size / unit)
            .map(|_| Frame::allocate_zeroed())
            .collect::<Option<Vec<_>>>()
            .ok_or(XError::NoMemory)?
            .into_boxed_slice();
        Ok(Arc::new(Self {
            id: allocate_object_id()?,
            frames,
        }))
    }
}

impl VmObject for SharedObject {
    fn id(&self) -> u64 {
        self.id
    }

    fn byte_len(&self) -> LinuxResult<u64> {
        Ok((self.frames.len() * usize::from(PageSize::Size4K)) as u64)
    }

    fn page(&self, index: u64, _write: bool) -> LinuxResult<VmPage> {
        let index = usize::try_from(index).map_err(|_| LinuxError::EINVAL)?;
        let frame = self.frames.get(index).cloned().ok_or(LinuxError::EFAULT)?;
        Ok(VmPage { frame, guard: None })
    }
}
