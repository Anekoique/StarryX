use alloc::{sync::Arc, vec::Vec};

use allocator::AllocError;
use axerrno::{AxError, AxResult};
use memory_addr::{PhysAddr, VirtAddr};
use spin::Mutex;

use crate::{PAGE_SIZE, global_allocator};

/// A RAII wrapper of contiguous 4K-sized pages.
///
/// It will automatically deallocate the pages when dropped.
#[derive(Debug)]
pub struct GlobalPage {
    start_vaddr: VirtAddr,
    num_pages: usize,
}

impl GlobalPage {
    /// Allocate one 4K-sized page.
    pub fn alloc() -> AxResult<Self> {
        global_allocator()
            .alloc_pages(1, PAGE_SIZE)
            .map(|vaddr| Self {
                start_vaddr: vaddr.into(),
                num_pages: 1,
            })
            .map_err(alloc_err_to_ax_err)
    }

    /// Allocate one 4K-sized page and fill with zero.
    pub fn alloc_zero() -> AxResult<Self> {
        let mut p = Self::alloc()?;
        p.zero();
        Ok(p)
    }

    /// Allocate contiguous 4K-sized pages.
    pub fn alloc_contiguous(num_pages: usize, align_pow2: usize) -> AxResult<Self> {
        global_allocator()
            .alloc_pages(num_pages, align_pow2)
            .map(|vaddr| Self {
                start_vaddr: vaddr.into(),
                num_pages,
            })
            .map_err(alloc_err_to_ax_err)
    }

    /// Get the start virtual address of this page.
    pub fn start_vaddr(&self) -> VirtAddr {
        self.start_vaddr
    }

    /// Get the start physical address of this page.
    pub fn start_paddr<F>(&self, virt_to_phys: F) -> PhysAddr
    where
        F: FnOnce(VirtAddr) -> PhysAddr,
    {
        virt_to_phys(self.start_vaddr)
    }

    /// Get the total size (in bytes) of these page(s).
    pub fn size(&self) -> usize {
        self.num_pages * PAGE_SIZE
    }

    /// Convert to a raw pointer.
    pub fn as_ptr(&self) -> *const u8 {
        self.start_vaddr.as_ptr()
    }

    /// Convert to a mutable raw pointer.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.start_vaddr.as_mut_ptr()
    }

    /// Fill `self` with `byte`.
    pub fn fill(&mut self, byte: u8) {
        unsafe { core::ptr::write_bytes(self.as_mut_ptr(), byte, self.size()) }
    }

    /// Fill `self` with zero.
    pub fn zero(&mut self) {
        self.fill(0)
    }

    /// Forms a slice that can read data.
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.size()) }
    }

    /// Forms a mutable slice that can write data.
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.size()) }
    }
}

impl Drop for GlobalPage {
    fn drop(&mut self) {
        global_allocator().dealloc_pages(self.start_vaddr.into(), self.num_pages);
    }
}

const fn alloc_err_to_ax_err(e: AllocError) -> AxError {
    match e {
        AllocError::InvalidParam | AllocError::MemoryOverlap | AllocError::NotAllocated => {
            AxError::InvalidInput
        }
        AllocError::NoMemory => AxError::NoMemory,
    }
}

/// A safe wrapper of a single 4K page.
/// It holds the page's VirtAddr (PhysAddr + offset)
#[derive(Debug)]
pub struct PhysPage {
    /// The start virtual address of this page.
    pub start_vaddr: VirtAddr,
}

/// A container for managing multiple PhysPage instances with thread-safe shared ownership.
///
/// This structure provides a way to manage collections of physical pages using
/// Arc<Mutex<PhysPage>> for thread-safe access. It's useful for scenarios where
/// you need to share page ownership between multiple threads or when implementing
/// memory management systems that require reference counting.
///
/// # Example
/// ```no_run
/// use axalloc::{PhysPageSet, PhysPage};
/// use memory_addr::VirtAddr;
///
/// // Create a new page set
/// let mut page_set = PhysPageSet::new();
///
/// // Allocate contiguous pages
/// let pages = PhysPage::alloc_contiguous(4, 4096, None).unwrap();
///
/// // Access individual pages
/// for page_arc in pages.iter() {
///     if let Some(page) = page_arc.try_lock() {
///         println!("Page at: {:?}", page.start_vaddr());
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PhysPageSet {
    pages: Vec<Arc<Mutex<PhysPage>>>,
    num_pages: usize,
}

impl PhysPageSet {
    /// Create a new empty PhysPageSet
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            num_pages: 0,
        }
    }

    /// Create a PhysPageSet from a vector of pages
    pub fn from_pages(pages: Vec<PhysPage>) -> Self {
        let num_pages = pages.len();
        let pages = pages
            .into_iter()
            .map(|page| Arc::new(Mutex::new(page)))
            .collect();
        Self { pages, num_pages }
    }

    /// Get the number of pages in this set
    pub fn len(&self) -> usize {
        self.num_pages
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.num_pages == 0
    }

    /// Get a reference to the page at the given index
    pub fn get(&self, index: usize) -> Option<&Arc<Mutex<PhysPage>>> {
        self.pages.get(index)
    }

    /// Get an iterator over all pages
    pub fn iter(&self) -> impl Iterator<Item = &Arc<Mutex<PhysPage>>> {
        self.pages.iter()
    }

    /// Get the total size in bytes of all pages
    pub fn total_size(&self) -> usize {
        self.num_pages * PAGE_SIZE
    }

    /// Add a page to the set
    pub fn push(&mut self, page: Arc<Mutex<PhysPage>>) {
        self.pages.push(page);
        self.num_pages += 1;
    }

    /// Access the underlying Vec<Arc<Mutex<PhysPage>>>
    pub fn as_vec(&self) -> &Vec<Arc<Mutex<PhysPage>>> {
        &self.pages
    }

    /// Convert into the underlying Vec<Arc<Mutex<PhysPage>>>
    pub fn into_vec(mut self) -> Vec<Arc<Mutex<PhysPage>>> {
        let pages = core::mem::take(&mut self.pages);
        self.num_pages = 0;
        pages
    }

    /// Get the first page in the set, if any
    pub fn first(&self) -> Option<&Arc<Mutex<PhysPage>>> {
        self.pages.first()
    }

    /// Get the last page in the set, if any
    pub fn last(&self) -> Option<&Arc<Mutex<PhysPage>>> {
        self.pages.last()
    }

    /// Clear all pages from the set
    pub fn clear(&mut self) {
        self.pages.clear();
        self.num_pages = 0;
    }

    /// Remove a page with the specified virtual address from the set
    /// Returns true if a page was found and removed, false otherwise
    pub fn remove_by_vaddr(&mut self, vaddr: VirtAddr) -> bool {
        if let Some(pos) = self.pages.iter().position(|page| {
            // Lock the page to access its start_vaddr
            if let Some(locked_page) = page.try_lock() {
                locked_page.start_vaddr == vaddr
            } else {
                false
            }
        }) {
            self.pages.remove(pos);
            self.num_pages -= 1;
            true
        } else {
            false
        }
    }

    /// Remove a page with the specified physical address from the set
    /// Returns true if a page was found and removed, false otherwise
    pub fn remove_by_paddr<F>(&mut self, paddr: PhysAddr, phys_to_virt: F) -> bool
    where
        F: Fn(PhysAddr) -> VirtAddr,
    {
        self.remove_by_vaddr(phys_to_virt(paddr))
    }

    /// Push a PhysPage directly into the set (without Arc<Mutex>)
    pub fn push_page(&mut self, page: PhysPage) {
        self.pages.push(Arc::new(Mutex::new(page)));
        self.num_pages += 1;
    }
}

impl Drop for PhysPageSet {
    fn drop(&mut self) {
        // Pages will be automatically deallocated when the Arc<Mutex<PhysPage>>
        // instances are dropped and their reference count reaches zero
        self.clear();
    }
}

impl PhysPage {
    pub fn new(vaddr: VirtAddr) -> Self {
        Self { start_vaddr: vaddr }
    }

    /// Allocate one 4K-sized page.
    pub fn alloc() -> AxResult<Self> {
        global_allocator()
            .alloc_pages(1, PAGE_SIZE)
            .map(|vaddr| Self {
                start_vaddr: vaddr.into(),
            })
            .map_err(alloc_err_to_ax_err)
    }

    /// Allocate some 4K-sized pages and fill with zero.
    /// Returns a PhysPageSet containing Arc<Mutex<PhysPage>> for thread-safe access.
    pub fn alloc_contiguous(
        num_pages: usize,
        align_pow2: usize,
        data: Option<&[u8]>,
    ) -> AxResult<PhysPageSet> {
        global_allocator()
            .alloc_pages(num_pages, align_pow2)
            .map(|vaddr| {
                let pages = unsafe {
                    core::slice::from_raw_parts_mut(vaddr as *mut u8, num_pages * PAGE_SIZE)
                };
                pages.fill(0);
                if let Some(data) = data {
                    pages[..data.len()].copy_from_slice(data);
                }

                let mut page_set = PhysPageSet::new();
                for page_idx in 0..num_pages {
                    let phys_page = PhysPage {
                        start_vaddr: (vaddr + page_idx * PAGE_SIZE).into(),
                    };
                    page_set.push(Arc::new(Mutex::new(phys_page)));
                }
                page_set
            })
            .map_err(alloc_err_to_ax_err)
    }

    /// Convert to a raw pointer.
    pub fn as_ptr(&self) -> *const u8 {
        self.start_vaddr.as_ptr()
    }

    /// Convert to a mutable raw pointer.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.start_vaddr.as_mut_ptr()
    }

    /// Forms a slice that can read data.
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.as_ptr(), PAGE_SIZE) }
    }

    /// Forms a mutable slice that can write data.
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), PAGE_SIZE) }
    }

    /// Fill `self` with `byte`.
    pub fn fill(&mut self, byte: u8) {
        unsafe { core::ptr::write_bytes(self.as_mut_ptr(), byte, PAGE_SIZE) }
    }
}

impl Drop for PhysPage {
    fn drop(&mut self) {
        global_allocator().dealloc_pages(self.start_vaddr.into(), 1);
    }
}
