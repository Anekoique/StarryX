//! Runtime-sized virtual-page iteration.

use memory_addr::{PageIter as FixedPageIter, VirtAddr};

pub use memory_addr::{PAGE_SIZE_4K, PageIter4K};
pub use xhal::paging::{MappingFlags, PageSize};

const PAGE_SIZE_2M: usize = 0x20_0000;
const PAGE_SIZE_1G: usize = 0x4000_0000;

type PageIter2M<A> = FixedPageIter<PAGE_SIZE_2M, A>;
type PageIter1G<A> = FixedPageIter<PAGE_SIZE_1G, A>;

/// Iterates an aligned virtual range using a runtime-selected page size.
pub enum PageIter {
    Size4K(PageIter4K<VirtAddr>),
    Size2M(PageIter2M<VirtAddr>),
    Size1G(PageIter1G<VirtAddr>),
}

impl PageIter {
    pub fn new(start: VirtAddr, end: VirtAddr, page_size: PageSize) -> Option<Self> {
        match page_size {
            PageSize::Size4K => PageIter4K::<VirtAddr>::new(start, end).map(Self::Size4K),
            PageSize::Size2M => PageIter2M::<VirtAddr>::new(start, end).map(Self::Size2M),
            PageSize::Size1G => PageIter1G::<VirtAddr>::new(start, end).map(Self::Size1G),
        }
    }
}

impl Iterator for PageIter {
    type Item = VirtAddr;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Size4K(iter) => iter.next(),
            Self::Size2M(iter) => iter.next(),
            Self::Size1G(iter) => iter.next(),
        }
    }
}
