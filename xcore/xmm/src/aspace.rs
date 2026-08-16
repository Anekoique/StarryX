use core::fmt;

use alloc::vec::Vec;
use memory_addr::{MemoryAddr, PhysAddr, VirtAddr, VirtAddrRange, is_aligned};
use xerrno::{XError, XResult};
use xhal::{
    mem::phys_to_virt,
    paging::{MappingFlags, PageSize, PageTable, PagingError},
};

use crate::{Frame, PageIter, StaticFrameRange, frame::FrameKind};

fn has_leaf_access(flags: MappingFlags) -> bool {
    flags.intersects(MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE)
}

fn pte_flags(kind: FrameKind, flags: MappingFlags) -> XResult<MappingFlags> {
    let flags = kind.apply(flags - MappingFlags::PROT_NONE);
    if !has_leaf_access(flags) {
        return Ok(flags | MappingFlags::PROT_NONE);
    }
    if cfg!(any(target_arch = "riscv32", target_arch = "riscv64"))
        && !flags.intersects(MappingFlags::READ | MappingFlags::EXECUTE)
    {
        // RISC-V reserves W=1/R=0 and identifies a leaf by R or X.
        return Err(XError::InvalidInput);
    }
    Ok(flags)
}

/// One hardware address space and its page-table lifetime domain.
pub struct AddressSpace {
    range: VirtAddrRange,
    page_table: PageTable,
}

#[derive(Clone, Copy)]
struct ProtectionChange {
    address: VirtAddr,
    old_flags: MappingFlags,
    new_flags: MappingFlags,
}

/// An atomic sequence of PTE permission changes.
///
/// Dropping an uncommitted transaction restores every changed leaf in reverse
/// order. Each journal slot is reserved before its PTE is changed, so rollback
/// performs no allocation.
pub struct ProtectionTransaction<'a> {
    address_space: &'a mut AddressSpace,
    changes: Vec<ProtectionChange>,
    committed: bool,
}

impl AddressSpace {
    pub const fn page_table_root(&self) -> PhysAddr {
        self.page_table.root_paddr()
    }

    pub fn begin_protection(&mut self) -> ProtectionTransaction<'_> {
        ProtectionTransaction {
            address_space: self,
            changes: Vec::new(),
            committed: false,
        }
    }

    fn contains_range(&self, start: VirtAddr, size: usize) -> bool {
        VirtAddrRange::try_from_start_size(start, size)
            .is_some_and(|range| self.range.contains_range(range))
    }

    pub(crate) fn new_empty(base: VirtAddr, size: usize) -> XResult<Self> {
        let range = VirtAddrRange::try_from_start_size(base, size).ok_or(XError::InvalidInput)?;
        Ok(Self {
            range,
            page_table: PageTable::try_new().map_err(Self::paging_error)?,
        })
    }

    /// Creates a user page table only when local TLB invalidation is a sound
    /// frame-reuse boundary. Remote shootdown is not implemented yet.
    pub fn new_user(base: VirtAddr, size: usize) -> XResult<Self> {
        if xconfig::SMP != 1 {
            return Err(XError::Unsupported);
        }
        Self::new_empty(base, size)
    }

    /// Imports the immortal kernel's static top-level entries.
    #[cfg(feature = "copy-from")]
    pub(crate) fn copy_static_mappings_from(&mut self, other: &Self) -> XResult {
        if self.range.overlaps(other.range) {
            return Err(XError::InvalidInput);
        }
        let mut contains_alloc_leaf = false;
        other
            .page_table
            .walk_leaf_range(other.range.start, other.range.size(), |_, _, flags, _| {
                contains_alloc_leaf |= FrameKind::from_flags(flags) == FrameKind::Alloc;
            })
            .map_err(Self::paging_error)?;
        if contains_alloc_leaf {
            return Err(XError::BadState);
        }
        // SAFETY: this private entry point is called only with KERNEL_ASPACE,
        // which is initialized once and never dropped. Preflight also rejects
        // Alloc leaves, so the destination borrows no PTE-owned Frame.
        unsafe {
            self.page_table
                .copy_from(&other.page_table, other.range.start, other.range.size());
        }
        Ok(())
    }

    fn paging_error(error: PagingError) -> XError {
        match error {
            PagingError::NoMemory => XError::NoMemory,
            PagingError::AlreadyMapped => XError::AlreadyExists,
            PagingError::NotMapped => XError::BadAddress,
            PagingError::NotAligned | PagingError::MappedToHugePage => XError::InvalidInput,
        }
    }

    fn validate_range(&self, start: VirtAddr, size: usize, page_size: PageSize) -> XResult {
        if size == 0
            || !start.is_aligned(page_size)
            || !is_aligned(size, page_size.into())
            || !self.contains_range(start, size)
        {
            return Err(XError::InvalidInput);
        }
        Ok(())
    }

    pub fn map_static_range(
        &mut self,
        start: VirtAddr,
        frames: StaticFrameRange,
        flags: MappingFlags,
        page_size: PageSize,
    ) -> XResult {
        let physical_start = frames.start();
        let size = frames.size();
        self.validate_range(start, size, page_size)?;
        if !physical_start.is_aligned(page_size) || !frames.allows(flags) {
            return Err(XError::InvalidInput);
        }

        let end = start.checked_add(size).ok_or(XError::InvalidInput)?;
        for address in PageIter::new(start, end, page_size).ok_or(XError::InvalidInput)? {
            if self.page_table.query(address).is_ok() {
                return Err(XError::AlreadyExists);
            }
        }
        let flags = pte_flags(FrameKind::Static, flags)?;

        for address in PageIter::new(start, end, page_size).ok_or(XError::InvalidInput)? {
            let offset = address - start;
            match self
                .page_table
                .map(address, physical_start + offset, page_size, flags)
            {
                Ok(tlb) => {
                    tlb.flush();
                }
                Err(error) => {
                    for installed_address in
                        PageIter::new(start, address, page_size).expect("mapped prefix is aligned")
                    {
                        let (_, _, tlb) = self
                            .page_table
                            .unmap(installed_address)
                            .expect("newly installed static leaf must be removable");
                        tlb.flush();
                    }
                    return Err(Self::paging_error(error));
                }
            }
        }
        Ok(())
    }

    pub fn map_frame(&mut self, address: VirtAddr, frame: Frame, flags: MappingFlags) -> XResult {
        let page_size = PageSize::Size4K;
        self.validate_range(address, page_size.into(), page_size)?;
        if self.page_table.query(address).is_ok() {
            return Err(XError::AlreadyExists);
        }
        let flags = pte_flags(FrameKind::Alloc, flags)?;
        let physical = frame.physical_address();
        let tlb = self
            .page_table
            .map(address, physical, page_size, flags)
            .map_err(Self::paging_error)?;
        tlb.flush();
        let transferred = frame.into_pte();
        debug_assert_eq!(transferred, physical);
        Ok(())
    }

    pub fn replace_frame(
        &mut self,
        address: VirtAddr,
        expected: &Frame,
        replacement: Frame,
        flags: MappingFlags,
    ) -> XResult {
        let address = address.align_down(PageSize::Size4K);
        let (mapped, current_flags, page_size) =
            self.page_table.query(address).map_err(Self::paging_error)?;
        if mapped != expected.physical_address()
            || page_size != PageSize::Size4K
            || !current_flags.contains(MappingFlags::ALLOC_FRAME)
        {
            return Err(XError::BadState);
        }

        let replacement_physical = replacement.physical_address();
        let flags = pte_flags(FrameKind::Alloc, flags)?;
        let (_, tlb) = self
            .page_table
            .remap(address, replacement_physical, flags)
            .map_err(Self::paging_error)?;
        tlb.flush();
        let transferred = replacement.into_pte();
        debug_assert_eq!(transferred, replacement_physical);
        // SAFETY: the validated Alloc PTE owned exactly one reference. The
        // remap removed that PTE and its TLB entry has already been flushed.
        drop(
            unsafe { Frame::take_from_pte(mapped) }
                .expect("validated old PTE must retain allocated-frame metadata"),
        );
        Ok(())
    }

    fn preflight_range(
        &self,
        start: VirtAddr,
        size: usize,
        kind: FrameKind,
        expected_page_size: PageSize,
    ) -> XResult<usize> {
        self.validate_range(start, size, expected_page_size)?;
        let end = start.checked_add(size).ok_or(XError::InvalidInput)?;
        let mut leaf_count = 0;
        let mut validation_error = None;
        self.page_table
            .walk_leaf_range(start, size, |address, physical, flags, page_size| {
                leaf_count += 1;
                let Some(leaf_end) = address.checked_add(page_size.into()) else {
                    validation_error = Some(XError::InvalidInput);
                    return;
                };
                if address < start || leaf_end > end {
                    validation_error = Some(XError::InvalidInput);
                } else if FrameKind::from_flags(flags) != kind || page_size != expected_page_size {
                    validation_error = Some(XError::BadState);
                } else if kind == FrameKind::Alloc
                    && let Err(error) = Frame::validate_pte(physical)
                {
                    validation_error = Some(error);
                }
            })
            .map_err(Self::paging_error)?;
        if let Some(error) = validation_error {
            return Err(error);
        }
        Ok(leaf_count)
    }

    /// Removes every present leaf in the range; holes are intentionally skipped.
    fn unmap_range(
        &mut self,
        start: VirtAddr,
        size: usize,
        kind: FrameKind,
        page_size: PageSize,
    ) -> XResult {
        self.preflight_range(start, size, kind, page_size)?;
        let end = start.checked_add(size).ok_or(XError::InvalidInput)?;
        for address in PageIter::new(start, end, page_size).ok_or(XError::InvalidInput)? {
            let (physical, actual_size, tlb) = match self.page_table.unmap(address) {
                Ok(mapping) => mapping,
                Err(PagingError::NotMapped) => continue,
                Err(error) => panic!("preflighted page-table range changed: {error:?}"),
            };
            assert_eq!(actual_size, page_size, "page-table leaf size changed");
            tlb.flush();
            if kind == FrameKind::Alloc {
                // SAFETY: preflight verified this as a live Alloc PTE. The
                // leaf has been removed and its TLB entry flushed exactly once.
                drop(
                    unsafe { Frame::take_from_pte(physical) }
                        .expect("preflighted PTE must retain allocated-frame metadata"),
                );
            }
        }
        Ok(())
    }

    pub fn unmap_static_range(
        &mut self,
        start: VirtAddr,
        size: usize,
        page_size: PageSize,
    ) -> XResult {
        self.unmap_range(start, size, FrameKind::Static, page_size)
    }

    pub fn unmap_alloc_range(&mut self, start: VirtAddr, size: usize) -> XResult {
        self.unmap_range(start, size, FrameKind::Alloc, PageSize::Size4K)
    }

    fn protect_alloc_range(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
    ) -> XResult {
        self.preflight_range(start, size, FrameKind::Alloc, PageSize::Size4K)?;
        let flags = pte_flags(FrameKind::Alloc, flags)?;
        let end = start.checked_add(size).ok_or(XError::InvalidInput)?;
        for address in PageIter::new(start, end, PageSize::Size4K).ok_or(XError::InvalidInput)? {
            let (actual_size, tlb) = match self.page_table.protect(address, flags) {
                Ok(mapping) => mapping,
                Err(PagingError::NotMapped) => continue,
                Err(error) => panic!("preflighted Alloc range changed: {error:?}"),
            };
            assert_eq!(
                actual_size,
                PageSize::Size4K,
                "page-table leaf size changed"
            );
            tlb.flush();
        }
        Ok(())
    }

    pub fn protect_alloc_page(&mut self, address: VirtAddr, flags: MappingFlags) -> XResult {
        self.protect_alloc_range(
            address.align_down(PageSize::Size4K),
            usize::from(PageSize::Size4K),
            flags,
        )
    }

    /// Clones the counted frame reference only when an Alloc PTE shares it.
    ///
    /// `None` means the PTE is the frame's sole owner, while `Some` keeps a
    /// shared frame alive for a caller that needs to copy or inspect it.
    pub fn frame_if_shared(&self, address: VirtAddr) -> XResult<Option<Frame>> {
        let (physical, flags, page_size) =
            self.page_table.query(address).map_err(Self::paging_error)?;
        if FrameKind::from_flags(flags) != FrameKind::Alloc || page_size != PageSize::Size4K {
            return Err(XError::BadState);
        }
        let physical = physical.align_down(page_size);
        if Frame::pte_is_unique(physical)? {
            Ok(None)
        } else {
            Frame::clone_from_pte(physical).map(Some)
        }
    }

    /// Snapshots the Alloc mappings in `range` as address/frame/permission tuples.
    pub fn mapped_frames(
        &self,
        range: VirtAddrRange,
    ) -> XResult<Vec<(VirtAddr, Frame, MappingFlags)>> {
        let mut frames = Vec::new();
        let mut validation_error = None;
        self.page_table
            .walk_leaf_range(
                range.start,
                range.size(),
                |address, physical, flags, page_size| {
                    if validation_error.is_some() {
                        return;
                    }
                    if FrameKind::from_flags(flags) != FrameKind::Alloc
                        || page_size != PageSize::Size4K
                    {
                        validation_error = Some(XError::BadState);
                        return;
                    }
                    match Frame::clone_from_pte(physical) {
                        Ok(frame) => frames.push((
                            address,
                            frame,
                            flags - MappingFlags::ALLOC_FRAME - MappingFlags::PROT_NONE,
                        )),
                        Err(error) => validation_error = Some(error),
                    }
                },
            )
            .map_err(Self::paging_error)?;
        if let Some(error) = validation_error {
            return Err(error);
        }
        Ok(frames)
    }

    /// Returns the flags of the resident leaf containing `address`.
    pub fn mapping_flags(&self, address: VirtAddr) -> Option<MappingFlags> {
        self.page_table
            .query(address)
            .ok()
            .map(|(_, flags, _)| flags)
    }

    fn process_bytes<F>(
        &self,
        start: VirtAddr,
        size: usize,
        access: MappingFlags,
        kind: Option<FrameKind>,
        mut copy: F,
    ) -> XResult
    where
        F: FnMut(VirtAddr, usize, usize),
    {
        if size == 0 {
            return Ok(());
        }
        if !self.contains_range(start, size) {
            return Err(XError::InvalidInput);
        }
        let mut copied = 0;
        while copied < size {
            let address = start + copied;
            let (physical, flags, page_size) =
                self.page_table.query(address).map_err(Self::paging_error)?;
            if !flags.contains(access)
                || flags.contains(MappingFlags::DEVICE)
                || kind.is_some_and(|kind| FrameKind::from_flags(flags) != kind)
            {
                return Err(XError::PermissionDenied);
            }
            let available = usize::from(page_size) - address.align_offset(page_size);
            let count = available.min(size - copied);
            copy(phys_to_virt(physical), copied, count);
            copied += count;
        }
        Ok(())
    }

    /// Copies bytes from readable, ordinary-memory leaves.
    ///
    /// Static and Alloc leaves are both readable, but device mappings require a
    /// dedicated volatile/device API rather than ordinary byte copies.
    pub fn read_bytes(&self, start: VirtAddr, output: &mut [u8]) -> XResult {
        self.process_bytes(
            start,
            output.len(),
            MappingFlags::READ,
            None,
            |source, offset, count| {
                // SAFETY: page-table query proved the source leaf present; output
                // is exclusive, this is ordinary memory with READ permission, and
                // each chunk is in bounds.
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        source.as_ptr(),
                        output.as_mut_ptr().add(offset),
                        count,
                    );
                }
            },
        )
    }

    /// Copies bytes only into writable Alloc leaves.
    ///
    /// Requiring `FrameKind::Alloc` prevents a safe caller from bypassing the
    /// lifetime and alias contract carried by a read-only `StaticFrameRange`.
    pub fn write_alloc_bytes(&mut self, start: VirtAddr, input: &[u8]) -> XResult {
        self.process_bytes(
            start,
            input.len(),
            MappingFlags::WRITE,
            Some(FrameKind::Alloc),
            |destination, offset, count| {
                // SAFETY: page-table query proved the destination leaf present;
                // it is a writable Alloc frame, input is valid, and each chunk is
                // in bounds. User address spaces are restricted to SMP=1.
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        input.as_ptr().add(offset),
                        destination.as_mut_ptr(),
                        count,
                    );
                }
            },
        )
    }
}

impl ProtectionTransaction<'_> {
    fn protect_range(
        &mut self,
        start: VirtAddr,
        size: usize,
        kind: FrameKind,
        page_size: PageSize,
        expected_leaf_count: Option<usize>,
        mut new_flags: impl FnMut(VirtAddr, PhysAddr) -> XResult<MappingFlags>,
    ) -> XResult {
        let leaf_count = self
            .address_space
            .preflight_range(start, size, kind, page_size)?;
        if expected_leaf_count.is_some_and(|expected| expected != leaf_count) {
            return Err(XError::BadState);
        }

        self.changes
            .try_reserve(leaf_count)
            .map_err(|_| XError::NoMemory)?;

        let first_change = self.changes.len();
        let mut prepare_error = None;
        self.address_space
            .page_table
            .walk_leaf_range(start, size, |address, physical, old_flags, actual_size| {
                if prepare_error.is_some() {
                    return;
                }
                debug_assert_eq!(FrameKind::from_flags(old_flags), kind);
                debug_assert_eq!(actual_size, page_size);
                match new_flags(address, physical).and_then(|flags| pte_flags(kind, flags)) {
                    Ok(new_flags) => self.changes.push(ProtectionChange {
                        address,
                        old_flags,
                        new_flags,
                    }),
                    Err(error) => prepare_error = Some(error),
                }
            })
            .map_err(AddressSpace::paging_error)?;
        if let Some(error) = prepare_error {
            self.changes.truncate(first_change);
            return Err(error);
        }

        for index in first_change..self.changes.len() {
            let change = self.changes[index];
            let (_, tlb) = self
                .address_space
                .page_table
                .protect(change.address, change.new_flags)
                .expect("preflighted mapping must remain protectable");
            tlb.flush();
        }
        Ok(())
    }

    pub fn protect_alloc_range(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
    ) -> XResult {
        self.protect_range(
            start,
            size,
            FrameKind::Alloc,
            PageSize::Size4K,
            None,
            |_, _| Ok(flags),
        )
    }

    /// Protects Alloc leaves using their current ownership exclusivity.
    pub fn protect_alloc_range_with(
        &mut self,
        start: VirtAddr,
        size: usize,
        mut flags: impl FnMut(bool) -> MappingFlags,
    ) -> XResult {
        self.protect_range(
            start,
            size,
            FrameKind::Alloc,
            PageSize::Size4K,
            None,
            |_, physical| Ok(flags(Frame::pte_is_unique(physical)?)),
        )
    }

    pub fn protect_static_range(
        &mut self,
        start: VirtAddr,
        frames: StaticFrameRange,
        flags: MappingFlags,
        page_size: PageSize,
    ) -> XResult {
        self.address_space
            .validate_range(start, frames.size(), page_size)?;
        if !frames.allows(flags) {
            return Err(XError::PermissionDenied);
        }
        self.protect_range(
            start,
            frames.size(),
            FrameKind::Static,
            page_size,
            Some(frames.size() / usize::from(page_size)),
            |address, physical| {
                (physical == frames.start() + (address - start))
                    .then_some(flags)
                    .ok_or(XError::BadState)
            },
        )
    }

    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for ProtectionTransaction<'_> {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for change in self.changes.iter().rev() {
            let (_, tlb) = self
                .address_space
                .page_table
                .protect(change.address, change.old_flags)
                .expect("protected mapping must remain restorable");
            tlb.flush();
        }
    }
}

impl fmt::Debug for AddressSpace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AddressSpace")
            .field("range", &self.range)
            .field("page_table_root", &self.page_table.root_paddr())
            .finish()
    }
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        let mut contains_alloc_leaf = false;
        self.page_table
            .walk_leaf_range(self.range.start, self.range.size(), |_, _, flags, _| {
                contains_alloc_leaf |= FrameKind::from_flags(flags) == FrameKind::Alloc;
            })
            .expect("address-space range must remain walkable");
        assert!(
            !contains_alloc_leaf,
            "Alloc mappings must be removed before dropping an address space"
        );
    }
}
