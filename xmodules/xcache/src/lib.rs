// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

//! Coherent file page-cache mechanisms.
//!
//! Filesystems provide raw [`Backing`] I/O, `xmm` provides owned frames, and
//! the kernel integration layer owns workers and VMA adapters. This crate does
//! not depend on a concrete filesystem or process implementation.

#![no_std]
#![forbid(unsafe_code)]
#![feature(allocator_api)]

extern crate alloc;

mod backing;
mod manager;
mod mapping;
mod page;

pub use backing::Backing;
pub use manager::{CacheManager, CachePolicy};
pub use mapping::{FileMapping, InvalidationObserver, ObserverRegistration, WritebackCursor};
pub use page::PageLease;

/// The cache page size is the frame size — a [`xmm::Frame`] backs every page.
pub const PAGE_SIZE: usize = xmm::PAGE_SIZE_4K;
const PAGE_SHIFT: u32 = PAGE_SIZE.trailing_zeros();

#[inline]
pub(crate) const fn page_index(offset: u64) -> u64 {
    offset >> PAGE_SHIFT
}

#[inline]
pub(crate) const fn page_offset(offset: u64) -> usize {
    (offset as usize) & (PAGE_SIZE - 1)
}
