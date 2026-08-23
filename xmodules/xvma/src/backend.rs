// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use alloc::sync::Arc;
use memory_addr::{VirtAddr, VirtAddrRange};
use xerrno::{XError, XResult};
use xmm::{AddressSpace, Frame, MappingFlags, PageSize, ProtectionTransaction, StaticFrameRange};

use crate::{FaultResolution, VmObject, area::VmArea, space::VmSpace};

/// A construction request for one virtual-memory area.
///
/// The type is opaque so one-shot policy such as eager population cannot leak
/// into the persistent VMA backing.
pub struct Backend {
    backing: Backing,
    page_size: PageSize,
    populate: bool,
}

impl Backend {
    pub const fn static_frames(frames: StaticFrameRange, page_size: PageSize) -> Self {
        Self {
            backing: Backing {
                source: Source::Static { frames },
                private: false,
            },
            page_size,
            populate: false,
        }
    }

    pub const fn anonymous(populate: bool) -> Self {
        Self {
            backing: Backing {
                source: Source::Zero,
                private: true,
            },
            page_size: PageSize::Size4K,
            populate,
        }
    }

    /// Maps `object` copy-on-write: stores stay local to this address space.
    pub fn private(object: Arc<dyn VmObject>, offset: usize, populate: bool) -> Self {
        Self::object(object, offset, true, populate)
    }

    /// Maps `object` write-through: stores reach the object itself.
    pub fn shared(object: Arc<dyn VmObject>, offset: usize, populate: bool) -> Self {
        Self::object(object, offset, false, populate)
    }

    fn object(object: Arc<dyn VmObject>, offset: usize, private: bool, populate: bool) -> Self {
        Self {
            backing: Backing {
                source: Source::Object { object, offset },
                private,
            },
            page_size: PageSize::Size4K,
            populate,
        }
    }

    pub(super) fn prepare(self, size: usize) -> XResult<(PageSize, Backing, bool)> {
        match &self.backing.source {
            Source::Zero => {}
            Source::Static { frames } => {
                if frames.size() != size {
                    return Err(XError::InvalidInput);
                }
            }
            Source::Object { offset, .. } => {
                if !offset.is_multiple_of(usize::from(PageSize::Size4K))
                    || offset.checked_add(size).is_none()
                {
                    return Err(XError::InvalidInput);
                }
            }
        }
        Ok((self.page_size, self.backing, self.populate))
    }
}

/// Where the pages of a VMA come from.
#[derive(Clone)]
pub(super) enum Source {
    /// Demand-zero anonymous memory.
    Zero,
    /// A physical range whose lifetime is guaranteed for the kernel lifetime.
    Static { frames: StaticFrameRange },
    /// Pages supplied by a shared object at a byte offset into it.
    Object {
        object: Arc<dyn VmObject>,
        offset: usize,
    },
}

/// Persistent lifetime and fault policy for one VMA.
///
/// The two fields are independent: `source` decides where a page comes from,
/// `private` decides whether a store copies it or writes through to the source.
#[derive(Clone)]
pub(super) struct Backing {
    pub(super) source: Source,
    pub(super) private: bool,
}

impl Backing {
    pub(super) fn shifted(&self, delta: usize, size: usize) -> Option<Self> {
        let source = match &self.source {
            Source::Zero => Source::Zero,
            Source::Static { frames } => Source::Static {
                frames: frames.subrange(delta, size).ok()?,
            },
            Source::Object { object, offset } => Source::Object {
                object: object.clone(),
                offset: offset.checked_add(delta)?,
            },
        };
        Some(Self {
            source,
            private: self.private,
        })
    }

    pub(super) fn can_merge(&self, next: &Self, left_size: usize) -> bool {
        if self.private != next.private {
            return false;
        }
        match (&self.source, &next.source) {
            (Source::Zero, Source::Zero) => true,
            (
                Source::Object {
                    object: left,
                    offset: left_offset,
                },
                Source::Object {
                    object: right,
                    offset: right_offset,
                },
            ) => {
                left.id() == right.id()
                    && left_offset
                        .checked_add(left_size)
                        .is_some_and(|offset| offset == *right_offset)
            }
            _ => false,
        }
    }

    /// The object this VMA draws from, and the object offset `delta` bytes in.
    pub(super) fn object_at(&self, delta: usize) -> Option<(usize, &dyn VmObject)> {
        let Source::Object { object, offset } = &self.source else {
            return None;
        };
        Some((offset.checked_add(delta)?, object.as_ref()))
    }

    pub(super) fn map(&self, area: &VmArea, address_space: &mut AddressSpace) -> XResult {
        match &self.source {
            Source::Static { frames } => address_space.map_static_range(
                area.range.start,
                *frames,
                area.flags,
                area.page_size,
            ),
            // Object and zero pages arrive through faults.
            Source::Zero | Source::Object { .. } => Ok(()),
        }
    }

    pub(super) fn unmap(
        &self,
        area: &VmArea,
        range: VirtAddrRange,
        address_space: &mut AddressSpace,
    ) -> XResult {
        match &self.source {
            Source::Static { .. } => {
                address_space.unmap_static_range(range.start, range.size(), area.page_size)
            }
            Source::Zero | Source::Object { .. } => {
                address_space.unmap_alloc_range(range.start, range.size())
            }
        }
    }

    pub(super) fn protect(
        &self,
        area: &VmArea,
        range: VirtAddrRange,
        flags: MappingFlags,
        transaction: &mut ProtectionTransaction<'_>,
    ) -> XResult {
        if let Source::Static { frames } = &self.source {
            let offset = range.start - area.range.start;
            return transaction.protect_static_range(
                range.start,
                frames.subrange(offset, range.size())?,
                flags,
                area.page_size,
            );
        }
        if !flags.contains(MappingFlags::WRITE) {
            return transaction.protect_alloc_range(range.start, range.size(), flags);
        }
        if self.private {
            // A COW page may only regain WRITE while this space owns it
            // exclusively; any shared frame must fault to be copied first.
            return transaction.protect_alloc_range_with(range.start, range.size(), |exclusive| {
                if exclusive {
                    flags
                } else {
                    flags - MappingFlags::WRITE
                }
            });
        }
        if self.requires_write_guard() {
            // A guarded page regains WRITE through a write fault that first
            // accounts it dirty, never through mprotect.
            return transaction.protect_alloc_range(
                range.start,
                range.size(),
                flags - MappingFlags::WRITE,
            );
        }
        transaction.protect_alloc_range(range.start, range.size(), flags)
    }

    pub(super) fn map_child(
        &self,
        area: &VmArea,
        parent: &AddressSpace,
        child: &mut AddressSpace,
    ) -> XResult {
        if let Source::Static { frames } = &self.source {
            return child.map_static_range(area.range.start, *frames, area.flags, area.page_size);
        }
        // A shared child keeps WRITE because parent and child address the same
        // frame; every private frame becomes copy-on-write in both spaces.
        for (address, frame, flags) in parent.mapped_frames(area.range)? {
            let flags = if self.private {
                flags - MappingFlags::WRITE
            } else {
                flags
            };
            child.map_frame(address, frame, flags)?;
        }
        Ok(())
    }

    pub(super) fn protect_parent_after_fork(
        &self,
        area: &VmArea,
        transaction: &mut ProtectionTransaction<'_>,
    ) -> XResult {
        if self.private && area.flags.contains(MappingFlags::WRITE) {
            transaction.protect_alloc_range(
                area.range.start,
                area.range.size(),
                area.flags - MappingFlags::WRITE,
            )?;
        }
        Ok(())
    }

    pub(super) fn resolve_fault(
        &self,
        area: &VmArea,
        page: VirtAddr,
        access: MappingFlags,
        space: &mut VmSpace,
    ) -> FaultResolution {
        match &self.source {
            Source::Zero => {
                let Some(frame) = Frame::allocate_zeroed() else {
                    return FaultResolution::NoMemory;
                };
                match space.address_space.map_frame(page, frame, area.flags) {
                    Ok(()) => FaultResolution::Resolved,
                    Err(XError::NoMemory) => FaultResolution::NoMemory,
                    Err(_) => FaultResolution::Segv,
                }
            }
            Source::Object { object, offset } => space.resolve_object_fault(
                page,
                area,
                object.as_ref(),
                *offset,
                self.private,
                access,
            ),
            // A static range is fully mapped at creation; a fault means the
            // range was torn down under us.
            Source::Static { .. } => FaultResolution::Segv,
        }
    }

    /// Whether writable mappings of this backing require a page guard.
    pub(super) fn requires_write_guard(&self) -> bool {
        !self.private
            && matches!(&self.source, Source::Object { object, .. } if object.requires_write_guard())
    }
}
