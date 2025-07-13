use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{sync::Arc, vec::Vec};
use axerrno::{LinuxError, LinuxResult};
use axmm::{AddrSpace, PageIter4K};
use axsync::{Mutex, RawMutex};

use axuspace::UserSpaceAccess;
use axvma::{MmapRegion, VmFile, VmaManager};
use memory_addr::{MemoryAddr, VirtAddr, VirtAddrRange};
use page_table_multiarch::MappingFlags;
use spin::RwLock;

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
            heap_bottom: AtomicUsize::new(axconfig::plat::USER_HEAP_BASE),
            heap_top: AtomicUsize::new(axconfig::plat::USER_HEAP_BASE),
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
                        debug!("Populating page: {:#x}", page_addr);
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

#[derive(Clone)]
pub struct FileWrapper(pub Arc<Mutex<axfs_ng::FsFile<RawMutex>>>);
impl VmFile for FileWrapper {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> LinuxResult<usize> {
        self.0.lock().read_at(buf, offset)
    }

    fn len(&self) -> LinuxResult<u64> {
        self.0.lock().len()
    }
}
