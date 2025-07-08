/// Virtual Memory Area (VMA) management.

pub struct MmapRegion {
    pub range: VirtAddrRange,
    pub file: Arc<Mutex<FsFile<RawMutex>>>,
    pub offset: isize,
    pub populated: Mutex<BTreeSet<VirtAddr>>,
    pub align: PageSize,
}

impl MmapRegion {
    pub fn new(
        range: VirtAddrRange,
        file: Arc<Mutex<FsFile<RawMutex>>>,
        offset: isize,
        align: PageSize,
    ) -> Self {
        Self {
            range,
            file,
            offset,
            populated: Mutex::new(BTreeSet::new()),
            align,
        }
    }

    pub fn contains(&self, vaddr: VirtAddr) -> bool {
        self.range.contains(vaddr)
    }

    pub fn overlaps(&self, range: &VirtAddrRange) -> bool {
        self.range.overlaps(*range)
    }

    pub fn split_at_range(&self, range: &VirtAddrRange) -> (Option<Self>, Option<Self>, Option<Self>) {
        if !self.overlaps(range) {
            return (None, None, None);
        }

        let self_start = self.range.start;
        let self_end = self.range.end;
        let split_start = range.start;
        let split_end = range.end;

        let before = if self_start < split_start {
            Some(Self {
                range: VirtAddrRange::from_start_size(self_start, split_start - self_start),
                file: self.file.clone(),
                offset: self.offset,
                populated: Mutex::new(BTreeSet::new()),
                align: self.align,
            })
        } else {
            None
        };

        let after = if split_end < self_end {
            Some(Self {
                range: VirtAddrRange::from_start_size(split_end, self_end - split_end),
                file: self.file.clone(),
                offset: self.offset + (split_end - self_start) as isize,
                populated: Mutex::new(BTreeSet::new()),
                align: self.align,
            })
        } else {
            None
        };

        (before, after)
    }

    pub fn get_buf(&self, vaddr: VirtAddr) -> LinuxResult<Vec<u8>> {
        let page_addr = vaddr.align_down(self.align);
        if self.populated.lock().contains(&page_addr) {
            return Err(LinuxError::EFAULT);
        }

        let page_offset = page_addr - self.range.start;
        let file_offset = self.offset + page_offset as isize;
        if file_offset < 0 || file_offset >= self.file.lock().len()? as isize {
            return Err(LinuxError::EINVAL);
        }

        let buf_size = core::cmp::min(self.align as usize, self.range.end - page_addr);
        let mut buf = vec![0u8; buf_size];
        self.file.lock().read_at(&mut buf, file_offset as u64)?;
        self.populated.lock().insert(page_addr);

        Ok(buf)
    }
}

impl Clone for MmapRegion {
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

#[derive(Default, Clone)]
pub struct VmaManager {
    regions: Vec<MmapRegion>,
}

impl VmaManager {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn add_region(&mut self, region: MmapRegion) -> LinuxResult<()> {
        self.regions.push(region);
        Ok(())
    }

    pub fn find_region(&self, vaddr: VirtAddr) -> Option<&MmapRegion> {
        self.regions.iter().find(|r| r.contains(vaddr))
    }

    pub fn find_overlapped(&self, vaddr: VirtAddr) -> Option<&MmapRegion> {}

    pub fn remove_region(&mut self, vaddr: VirtAddr) -> LinuxResult<()> {}

    pub fn populate_pages(&self, vaddr: VirtAddr, len: usize) -> LinuxResult<()> {}
}
