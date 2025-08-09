use alloc::{sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axfs_ng_vfs::FileNodeOps;
use axmm::{AddrSpace, PageIter4K};
use axsync::{Mutex, RawMutex};
use memory_addr::{MemoryAddr, PhysAddr, VirtAddr, VirtAddrRange};
use page_table_multiarch::{MappingFlags, PageSize};
use spin::RwLock;

use xcache::{InodeOps, PageOps};
use xuspace::UserSpaceAccess;
use xvma::{MmapRegion, VmFile, VmaManager};

use super::PAGE_CACHE_MANAGER;

pub struct XUserSpace {
    pub aspace: Arc<Mutex<AddrSpace>>,
    pub heap_bottom: AtomicUsize,
    pub heap_top: AtomicUsize,
    pub vma_manager: RwLock<VmaManager<FileWrapper>>,
}

impl XUserSpace {
    pub fn new(
        aspace: Arc<Mutex<AddrSpace>>,
        vma_manager: RwLock<VmaManager<FileWrapper>>,
    ) -> Self {
        Self {
            aspace,
            heap_bottom: AtomicUsize::new(crate::config::USER_HEAP_BASE),
            heap_top: AtomicUsize::new(crate::config::USER_HEAP_BASE),
            vma_manager,
        }
    }

    pub fn get_heap_bottom(&self) -> usize {
        self.heap_bottom.load(Ordering::Acquire)
    }

    pub fn set_heap_bottom(&self, bottom: usize) {
        self.heap_bottom.store(bottom, Ordering::Release);
    }

    pub fn get_heap_top(&self) -> usize {
        self.heap_top.load(Ordering::Acquire)
    }

    pub fn set_heap_top(&self, top: usize) {
        self.heap_top.store(top, Ordering::Release);
    }

    pub fn add_region(&self, region: MmapRegion<FileWrapper>) -> LinuxResult<()> {
        self.vma_manager.write().add_region(region)
    }

    pub fn remove_overlapping_regions(
        &self,
        vaddr_range: VirtAddrRange,
    ) -> Vec<MmapRegion<FileWrapper>> {
        self.vma_manager.write().remove_overlapped(vaddr_range)
    }

    pub fn clear_regions(&self) {
        self.vma_manager.write().clear()
    }

    pub fn populate_file_pages(&self, vaddr: VirtAddr, len: usize) -> LinuxResult<()> {
        let start_addr = vaddr.align_down_4k();
        let end_addr = (vaddr + len).align_up_4k();
        let aspace = self.aspace.lock();

        for page_addr in PageIter4K::new(start_addr, end_addr).unwrap() {
            if let Some(region) = self.vma_manager.read().find_region(page_addr) {
                if region.populated.lock().contains(&page_addr) {
                    continue;
                }

                match region.get_buf(page_addr) {
                    Ok(page_data) => {
                        aspace.write(page_addr, &page_data, region.align)?;
                    }
                    Err(LinuxError::EEXIST) => {
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }
}

impl UserSpaceAccess for &XUserSpace {
    fn check_region_access(
        &self,
        range: VirtAddrRange,
        access_flags: MappingFlags,
    ) -> LinuxResult<()> {
        let aspace = self.aspace.lock();
        if !aspace.check_region_access(range, access_flags) {
            warn!(
                "check_region_access: range={:?}, access_flags={:?}",
                range, access_flags
            );
            return Err(LinuxError::EFAULT);
        }
        Ok(())
    }

    fn populate_region(&self, range: VirtAddrRange, access_flags: MappingFlags) -> LinuxResult<()> {
        let mut aspace = self.aspace.lock();
        let page_start = range.start.align_down_4k();
        let page_end = (range.end).align_up_4k();
        aspace.populate_area(page_start, page_end - page_start, access_flags)?;
        drop(aspace);
        self.populate_file_pages(page_start, page_end - page_start)?;
        Ok(())
    }
}

impl PageOps for XUserSpace {
    fn alloc_page() -> Option<PhysAddr> {
        axmm::alloc_frame(true, PageSize::Size4K)
    }

    fn dealloc_page(addr: PhysAddr) {
        axmm::dealloc_frame(addr, PageSize::Size4K)
    }

    fn read_page(addr: VirtAddr, buf: &mut [u8]) -> LinuxResult {
        unsafe {
            core::ptr::copy_nonoverlapping(addr.as_ptr(), buf.as_mut_ptr(), buf.len());
        }
        Ok(())
    }

    fn write_page(addr: VirtAddr, buf: &[u8]) -> LinuxResult {
        unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr(), addr.as_mut_ptr(), buf.len());
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct FileWrapper(pub Arc<Mutex<axfs_ng::FsFile<RawMutex>>>);
impl VmFile for FileWrapper {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> LinuxResult<usize> {
        let inner = self.0.lock();
        if !inner.get_flags().contains(FileFlags::DIRECT)
            && let Some(cache) = PAGE_CACHE_MANAGER.get_cache(inner.inode()?)
        {
            cache.read_at(buf, offset)
        } else {
            inner.read_at(buf, offset)
        }
    }

    fn len(&self) -> LinuxResult<u64> {
        let inner = self.0.lock();
        Ok(PAGE_CACHE_MANAGER
            .get_cache(inner.inode()?)
            .map(|cache| cache.get_size())
            .unwrap_or(inner.len()?))
    }
}

pub struct InodeWrapper(pub Mutex<Arc<dyn FileNodeOps<RawMutex>>>);
impl InodeOps for InodeWrapper {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> LinuxResult<usize> {
        self.0.lock().read_at(buf, offset)
    }
    fn write_at(&self, buf: &[u8], offset: u64) -> LinuxResult<usize> {
        self.0.lock().write_at(buf, offset)
    }
    fn len(&self) -> LinuxResult<u64> {
        self.0.lock().len()
    }
    fn set_len(&self, len: u64) -> LinuxResult {
        self.0.lock().set_len(len)
    }
}
impl InodeWrapper {
    pub fn inode(&self) -> u64 {
        self.0.lock().inode()
    }

    pub fn is_stale(&self) -> bool {
        Arc::strong_count(&*self.0.lock()) == 1
    }
}
