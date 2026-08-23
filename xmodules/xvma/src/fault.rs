use memory_addr::{MemoryAddr, VirtAddr};
use xerrno::{LinuxError, XError, XResult};
use xmm::{Frame, MappingFlags, PageIter4K};

use crate::{VmObject, VmPageGuard, area::VmArea, space::VmSpace};

/// Architecture-neutral result of resolving one user page fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaultResolution {
    Resolved,
    Retry,
    Segv,
    Bus,
    NoMemory,
}

impl FaultResolution {
    /// Classifies a page-provider failure. Anything other than a transient
    /// retry or memory shortage is reported to userspace as `SIGBUS`.
    fn from_page_error(error: LinuxError) -> Self {
        match error {
            LinuxError::EAGAIN => Self::Retry,
            LinuxError::ENOMEM => Self::NoMemory,
            _ => Self::Bus,
        }
    }
}

impl VmSpace {
    pub fn populate_area(&mut self, start: VirtAddr, size: usize, access: MappingFlags) -> XResult {
        let range = self.validate_existing_range(start, size)?;
        if !self.check_region_access(range, access) {
            return Err(XError::NoMemory);
        }
        for page in PageIter4K::new(range.start, range.end).ok_or(XError::InvalidInput)? {
            match self.handle_page_fault(page, access) {
                FaultResolution::Resolved => {}
                FaultResolution::Retry => return Err(XError::WouldBlock),
                FaultResolution::NoMemory => return Err(XError::NoMemory),
                FaultResolution::Segv => return Err(XError::BadAddress),
                FaultResolution::Bus => return Err(XError::Io),
            }
        }
        Ok(())
    }

    pub fn handle_page_fault(
        &mut self,
        address: VirtAddr,
        access: MappingFlags,
    ) -> FaultResolution {
        let Some(area) = self.area_at(address) else {
            return FaultResolution::Segv;
        };
        if !area.flags.contains(access) {
            return FaultResolution::Segv;
        }
        let page = address.align_down(area.page_size);

        // The area is cloned only on the paths that mutate `self`; the common
        // already-resident case stays borrow-only.
        if let Some(flags) = self.address_space.mapping_flags(page) {
            if flags.contains(access) {
                return FaultResolution::Resolved;
            }
            if !access.contains(MappingFlags::WRITE) {
                return FaultResolution::Segv;
            }
            if area.is_private() {
                let flags = area.flags;
                return self.resolve_private_write(page, flags);
            }
            // A shared page already resident but not yet writable: the object
            // must account the store before the PTE may carry WRITE.
            let area = area.clone();
            let Some((offset, object)) = area.backing.object_at(page - area.range.start) else {
                return FaultResolution::Segv;
            };
            return self.upgrade_shared_write(page, &area, object, offset);
        }

        let area = area.clone();
        area.backing.resolve_fault(&area, page, access, self)
    }

    fn resolve_private_write(&mut self, page: VirtAddr, flags: MappingFlags) -> FaultResolution {
        match self.address_space.frame_if_shared(page) {
            Ok(None) => match self.address_space.protect_alloc_page(page, flags) {
                Ok(()) => FaultResolution::Resolved,
                Err(_) => FaultResolution::Segv,
            },
            Ok(Some(current)) => {
                let Some(replacement) = current.deep_copy() else {
                    return FaultResolution::NoMemory;
                };
                match self
                    .address_space
                    .replace_frame(page, &current, replacement, flags)
                {
                    Ok(()) => FaultResolution::Resolved,
                    Err(XError::NoMemory) => FaultResolution::NoMemory,
                    Err(_) => FaultResolution::Segv,
                }
            }
            Err(_) => FaultResolution::Segv,
        }
    }

    pub(super) fn resolve_object_fault(
        &mut self,
        page: VirtAddr,
        area: &VmArea,
        object: &dyn VmObject,
        source_offset: usize,
        private: bool,
        access: MappingFlags,
    ) -> FaultResolution {
        let Some(object_offset) = source_offset.checked_add(page - area.range.start) else {
            return FaultResolution::Bus;
        };
        let object_len = match object.byte_len() {
            Ok(length) => length,
            Err(error) => return FaultResolution::from_page_error(error),
        };
        if object_offset as u64 >= object_len {
            return FaultResolution::Bus;
        }
        let writing = access.contains(MappingFlags::WRITE);
        let index = (object_offset / xmm::PAGE_SIZE_4K) as u64;
        let supplied = match object.page(index, !private && writing) {
            Ok(page) => page,
            Err(error) => return FaultResolution::from_page_error(error),
        };

        if private {
            // A private write takes a copy so the object cannot observe this
            // address space's stores; a private read maps read-only and copies
            // on the first store instead.
            if !writing {
                return self.install_page(
                    page,
                    supplied.frame,
                    area.flags - MappingFlags::WRITE,
                    None,
                );
            }
            let Some(frame) = supplied.frame.deep_copy() else {
                return FaultResolution::NoMemory;
            };
            return self.install_page(page, frame, area.flags, None);
        }

        let flags = if object.requires_write_guard() && supplied.guard.is_none() {
            area.flags - MappingFlags::WRITE
        } else {
            area.flags
        };
        self.install_page(page, supplied.frame, flags, supplied.guard)
    }

    /// Grants WRITE to an already-resident shared page.
    fn upgrade_shared_write(
        &mut self,
        page: VirtAddr,
        area: &VmArea,
        object: &dyn VmObject,
        object_offset: usize,
    ) -> FaultResolution {
        let index = (object_offset / xmm::PAGE_SIZE_4K) as u64;
        let supplied = match object.page(index, true) {
            Ok(page) => page,
            Err(error) => return FaultResolution::from_page_error(error),
        };
        if object.requires_write_guard() && supplied.guard.is_none() {
            return FaultResolution::Bus;
        }
        let flags = area.flags;
        self.commit_page(page, supplied.guard, |space| {
            space.protect_alloc_page(page, flags)
        })
    }

    /// Maps one frame, reserving guard storage before any PTE becomes writable
    /// so a late allocation failure cannot leave an untracked writable page.
    fn install_page(
        &mut self,
        page: VirtAddr,
        frame: Frame,
        flags: MappingFlags,
        guard: Option<VmPageGuard>,
    ) -> FaultResolution {
        self.commit_page(page, guard, |space| space.map_frame(page, frame, flags))
    }

    /// Reserves guard storage, applies one PTE change, then records the guard,
    /// so the guard bookkeeping cannot fail after the PTE is live.
    fn commit_page(
        &mut self,
        page: VirtAddr,
        guard: Option<VmPageGuard>,
        pte_change: impl FnOnce(&mut xmm::AddressSpace) -> XResult,
    ) -> FaultResolution {
        if guard.is_some() && self.reserve_write_guard().is_err() {
            return FaultResolution::NoMemory;
        }
        match pte_change(&mut self.address_space) {
            Ok(()) => {
                if let Some(guard) = guard {
                    self.insert_write_guard(page, guard);
                }
                FaultResolution::Resolved
            }
            Err(XError::NoMemory) => FaultResolution::NoMemory,
            Err(_) => FaultResolution::Segv,
        }
    }
}
