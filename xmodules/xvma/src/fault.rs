use memory_addr::{MemoryAddr, VirtAddr};
use xerrno::{LinuxError, XError, XResult};
use xmm::{Frame, MappingFlags, PageIter4K};

use crate::{VmObject, area::VmArea, backend::AreaBackend, space::VmSpace};

/// Architecture-neutral result of resolving one user page fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaultResolution {
    Resolved,
    Retry,
    Segv,
    Bus,
    NoMemory,
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

        let area = area.clone();
        if let Some(flags) = self.address_space.mapping_flags(page) {
            if flags.contains(access) {
                return FaultResolution::Resolved;
            }
            if access.contains(MappingFlags::WRITE) && area.is_private() {
                return self.resolve_private_write(page, area.flags);
            }
            return FaultResolution::Segv;
        }

        area.backing.resolve_fault(&area, page, self)
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

    pub(super) fn resolve_source_fault(
        &mut self,
        page: VirtAddr,
        area: &VmArea,
        source: &dyn VmObject,
        source_offset: usize,
    ) -> FaultResolution {
        let Some(object_offset) = source_offset.checked_add(page - area.range.start) else {
            return FaultResolution::Bus;
        };
        let object_len = match source.byte_len() {
            Ok(len) => match usize::try_from(len) {
                Ok(len) => len,
                Err(_) => return FaultResolution::Bus,
            },
            Err(LinuxError::EAGAIN) => return FaultResolution::Retry,
            Err(LinuxError::ENOMEM) => return FaultResolution::NoMemory,
            Err(_) => return FaultResolution::Bus,
        };
        if object_offset >= object_len {
            return FaultResolution::Bus;
        }

        let Some(mut frame) = Frame::allocate_zeroed() else {
            return FaultResolution::NoMemory;
        };
        let readable = usize::from(area.page_size).min(object_len - object_offset);
        let mut data = alloc::vec![0_u8; readable];
        let read = match source.read_at(&mut data, object_offset as u64) {
            Ok(read) if read <= readable => read,
            Err(LinuxError::EAGAIN) => return FaultResolution::Retry,
            Err(LinuxError::ENOMEM) => return FaultResolution::NoMemory,
            Ok(_) | Err(_) => return FaultResolution::Bus,
        };
        if !frame.try_write_at(0, &data[..read]) {
            return FaultResolution::Bus;
        }
        match self.address_space.map_frame(page, frame, area.flags) {
            Ok(()) => FaultResolution::Resolved,
            Err(XError::NoMemory) => FaultResolution::NoMemory,
            Err(_) => FaultResolution::Segv,
        }
    }
}
