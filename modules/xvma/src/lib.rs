//! Virtual Memory Area (VMA) management for file-backed memory mappings.
//! Provides abstractions for handling memory-mapped files with on-demand loading.

#![no_std]
extern crate alloc;

use alloc::{collections::BTreeSet, vec, vec::Vec};
use axerrno::{LinuxError, LinuxResult};
use memory_addr::{MemoryAddr, VirtAddr, VirtAddrRange};
use page_table_multiarch::PageSize;
use spin::Mutex;

/// Trait for file operations required by VMA management
/// The implementor is responsible for thread safety and sharing semantics
pub trait VmFile: Send + Sync + Clone {
    /// Read data from the file at the specified offset
    fn read_at(&self, buf: &mut [u8], offset: u64) -> LinuxResult<usize>;

    /// Get the length of the file
    fn len(&self) -> LinuxResult<u64>;

    /// Is the file empty?
    fn is_empty(&self) -> LinuxResult<bool> {
        Ok(self.len()? == 0)
    }
}

/// Represents a memory-mapped region with file backing
pub struct MmapRegion<F: VmFile> {
    /// Virtual address range for this mapping
    pub range: VirtAddrRange,
    /// File backing this memory region
    pub file: F,
    /// Offset into the file for this mapping
    pub offset: isize,
    /// Set of populated (loaded) pages in this region
    pub populated: Mutex<BTreeSet<VirtAddr>>,
    /// Page alignment for this mapping
    pub align: PageSize,
}

impl<F: VmFile> MmapRegion<F> {
    /// Create a new memory-mapped region
    pub fn new(range: VirtAddrRange, file: F, offset: isize, align: PageSize) -> Self {
        Self {
            range,
            file,
            offset,
            populated: Mutex::new(BTreeSet::new()),
            align,
        }
    }

    /// Check if a virtual address is contained within this region
    pub fn contains(&self, vaddr: VirtAddr) -> bool {
        self.range.contains(vaddr)
    }

    /// Check if this region overlaps with the given range
    pub fn overlaps(&self, range: &VirtAddrRange) -> bool {
        self.range.overlaps(*range)
    }

    /// Split this region at the given range, returning up to three segments
    /// Returns (before_segment, overlap_segment, after_segment)
    pub fn split_at_range(
        &self,
        range: &VirtAddrRange,
    ) -> (Option<Self>, Option<Self>, Option<Self>) {
        if !self.overlaps(range) {
            return (None, None, None);
        }

        let self_range = &self.range;
        let split_range = range;
        let populated_pages = self.populated.lock().clone();

        // Helper to create a segment with the given range
        let create_segment = |segment_range: VirtAddrRange| -> Self {
            let populated = populated_pages
                .iter()
                .filter(|&page| segment_range.contains(*page))
                .cloned()
                .collect();

            Self {
                range: segment_range,
                file: self.file.clone(),
                offset: self.offset + (segment_range.start - self_range.start) as isize,
                populated: Mutex::new(populated),
                align: self.align,
            }
        };

        // Create segment before the split range
        let before = (self_range.start < split_range.start).then(|| {
            create_segment(VirtAddrRange::from_start_size(
                self_range.start,
                split_range.start - self_range.start,
            ))
        });

        // Create segment after the split range
        let after = (split_range.end < self_range.end).then(|| {
            create_segment(VirtAddrRange::from_start_size(
                split_range.end,
                self_range.end - split_range.end,
            ))
        });

        // Create overlapping segment
        let overlap_start = self_range.start.max(split_range.start);
        let overlap_end = self_range.end.min(split_range.end);
        let overlap = (overlap_start < overlap_end).then(|| {
            create_segment(VirtAddrRange::from_start_size(
                overlap_start,
                overlap_end - overlap_start,
            ))
        });

        (before, overlap, after)
    }

    /// Load data from file into a buffer for the given virtual address
    /// Returns an error if the page is already populated or if file access fails
    pub fn get_buf(&self, vaddr: VirtAddr) -> LinuxResult<Vec<u8>> {
        let page_addr = vaddr.align_down(self.align);
        if self.populated.lock().contains(&page_addr) {
            return Err(LinuxError::EEXIST);
        }

        let page_offset = page_addr - self.range.start;
        let file_offset = self.offset + page_offset as isize;
        if file_offset < 0 || file_offset >= self.file.len()? as isize {
            return Err(LinuxError::EINVAL);
        }

        let buf_size = core::cmp::min(self.align as usize, self.range.end - page_addr);
        let mut buf = vec![0u8; buf_size];
        self.file.read_at(&mut buf, file_offset as u64)?;
        self.populated.lock().insert(page_addr);

        Ok(buf)
    }
}

impl<F: VmFile> Clone for MmapRegion<F> {
    fn clone(&self) -> Self {
        Self {
            range: self.range,
            file: self.file.clone(),
            offset: self.offset,
            populated: Mutex::new(self.populated.lock().clone()),
            align: self.align,
        }
    }
}

/// Manager for Virtual Memory Areas with file backing
#[derive(Clone)]
pub struct VmaManager<F: VmFile> {
    /// Collection of memory-mapped regions
    regions: Vec<MmapRegion<F>>,
}

impl<F: VmFile> Default for VmaManager<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: VmFile> VmaManager<F> {
    /// Create a new VMA manager
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Clear all managed regions
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Add a new memory-mapped region to the manager
    pub fn add_region(&mut self, region: MmapRegion<F>) -> LinuxResult<()> {
        self.regions.push(region);
        Ok(())
    }

    /// Find the region containing the given virtual address
    pub fn find_region(&self, vaddr: VirtAddr) -> Option<&MmapRegion<F>> {
        self.regions.iter().find(|r| r.contains(vaddr))
    }

    /// Remove all regions that overlap with the given address range
    /// Splits overlapping regions and retains non-overlapping parts
    pub fn remove_overlapped(&mut self, vaddr_range: VirtAddrRange) -> Vec<MmapRegion<F>> {
        let mut removed = Vec::new();
        let mut retained = Vec::new();

        for region in self.regions.drain(..) {
            if region.overlaps(&vaddr_range) {
                let (before, overlap, after) = region.split_at_range(&vaddr_range);
                if let Some(overlap) = overlap {
                    removed.push(overlap);
                }
                if let Some(before) = before {
                    retained.push(before);
                }
                if let Some(after) = after {
                    retained.push(after);
                }
            } else {
                retained.push(region);
            }
        }
        self.regions = retained;
        removed
    }
}
