// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use alloc::{boxed::Box, sync::Arc, vec::Vec};
use xerrno::{LinuxResult, XError, XResult};
use xmm::{Frame, PageSize};

/// Synchronous input for a private sourced mapping.
///
/// Implementations must not re-enter the same address space while `read_at`
/// or `byte_len` is running.
pub trait VmObject: Send + Sync {
    fn read_at(&self, output: &mut [u8], offset: u64) -> LinuxResult<usize>;
    fn byte_len(&self) -> LinuxResult<u64>;
}

/// Stable identity and page ownership for an anonymous shared mapping.
pub struct SharedObject {
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
        Ok(Arc::new(Self { frames }))
    }

    pub(super) fn byte_len(&self) -> usize {
        self.frames.len() * usize::from(PageSize::Size4K)
    }

    pub(super) fn frame(&self, index: usize) -> Option<Frame> {
        self.frames.get(index).cloned()
    }
}
