// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use alloc::sync::Arc;
use memory_addr::{VirtAddr, VirtAddrRange};
use xerrno::{XError, XResult};
use xmm::{
    AddressSpace, Frame, MappingFlags, PageIter4K, PageSize, ProtectionTransaction,
    StaticFrameRange,
};

use crate::{FaultResolution, SharedObject, VmObject, area::VmArea, space::VmSpace};

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
            backing: Backing::Static { frames },
            page_size,
            populate: false,
        }
    }

    pub const fn anonymous(populate: bool) -> Self {
        Self {
            backing: Backing::Private {
                source: None,
                offset: 0,
            },
            page_size: PageSize::Size4K,
            populate,
        }
    }

    pub fn file(object: Arc<dyn VmObject>, offset: usize, populate: bool) -> Self {
        Self {
            backing: Backing::Private {
                source: Some(object),
                offset,
            },
            page_size: PageSize::Size4K,
            populate,
        }
    }

    pub fn shared(object: Arc<SharedObject>, offset: usize) -> Self {
        Self {
            backing: Backing::Shared { object, offset },
            page_size: PageSize::Size4K,
            populate: false,
        }
    }

    pub(super) fn prepare(self, size: usize) -> XResult<(PageSize, Backing, bool)> {
        match &self.backing {
            Backing::Static { frames } => {
                if frames.size() != size {
                    return Err(XError::InvalidInput);
                }
            }
            Backing::Private { source, offset } => {
                if !offset.is_multiple_of(usize::from(PageSize::Size4K))
                    || offset.checked_add(size).is_none()
                    || (source.is_none() && *offset != 0)
                {
                    return Err(XError::InvalidInput);
                }
            }
            Backing::Shared { object, offset } => {
                if !offset.is_multiple_of(usize::from(PageSize::Size4K))
                    || offset
                        .checked_add(size)
                        .is_none_or(|end| end > object.byte_len())
                {
                    return Err(XError::InvalidInput);
                }
            }
        }
        Ok((self.page_size, self.backing, self.populate))
    }
}

/// Persistent lifetime and fault policy for one VMA.
#[derive(Clone)]
pub(super) enum Backing {
    Static {
        frames: StaticFrameRange,
    },
    Private {
        source: Option<Arc<dyn VmObject>>,
        offset: usize,
    },
    Shared {
        object: Arc<SharedObject>,
        offset: usize,
    },
}

/// Closed, statically dispatched behavior of a VMA backing.
///
/// This trait is crate-private on purpose: the heterogeneous VMA tree stores
/// the closed [`Backing`] enum, not trait objects.
pub(super) trait AreaBackend: Sized {
    fn shifted(&self, delta: usize, size: usize) -> Option<Self>;
    fn can_merge(&self, next: &Self, left_size: usize) -> bool;
    fn is_private(&self) -> bool;

    fn map(&self, area: &VmArea, address_space: &mut AddressSpace) -> XResult;
    fn unmap(
        &self,
        area: &VmArea,
        range: VirtAddrRange,
        address_space: &mut AddressSpace,
    ) -> XResult;
    fn protect(
        &self,
        area: &VmArea,
        range: VirtAddrRange,
        flags: MappingFlags,
        transaction: &mut ProtectionTransaction<'_>,
    ) -> XResult;
    fn map_child(&self, area: &VmArea, parent: &AddressSpace, child: &mut AddressSpace) -> XResult;
    fn protect_parent_after_fork(
        &self,
        area: &VmArea,
        transaction: &mut ProtectionTransaction<'_>,
    ) -> XResult;
    fn resolve_fault(&self, area: &VmArea, page: VirtAddr, space: &mut VmSpace) -> FaultResolution;
}

impl AreaBackend for Backing {
    fn shifted(&self, delta: usize, size: usize) -> Option<Self> {
        Some(match self {
            Self::Static { frames } => Self::Static {
                frames: frames.subrange(delta, size).ok()?,
            },
            Self::Private { source, offset } => Self::Private {
                source: source.clone(),
                offset: if source.is_some() {
                    offset.checked_add(delta)?
                } else {
                    *offset
                },
            },
            Self::Shared { object, offset } => Self::Shared {
                object: object.clone(),
                offset: offset.checked_add(delta)?,
            },
        })
    }

    fn can_merge(&self, next: &Self, left_size: usize) -> bool {
        match (self, next) {
            (Self::Private { source: None, .. }, Self::Private { source: None, .. }) => true,
            (
                Self::Private {
                    source: Some(left),
                    offset: left_offset,
                },
                Self::Private {
                    source: Some(right),
                    offset: right_offset,
                },
            ) => {
                Arc::ptr_eq(left, right)
                    && left_offset
                        .checked_add(left_size)
                        .is_some_and(|offset| offset == *right_offset)
            }
            (
                Self::Shared {
                    object: left,
                    offset: left_offset,
                },
                Self::Shared {
                    object: right,
                    offset: right_offset,
                },
            ) => {
                Arc::ptr_eq(left, right)
                    && left_offset
                        .checked_add(left_size)
                        .is_some_and(|offset| offset == *right_offset)
            }
            _ => false,
        }
    }

    fn is_private(&self) -> bool {
        matches!(self, Self::Private { .. })
    }

    fn map(&self, area: &VmArea, address_space: &mut AddressSpace) -> XResult {
        match self {
            Self::Static { frames } => address_space.map_static_range(
                area.range.start,
                *frames,
                area.flags,
                area.page_size,
            ),
            Self::Private { .. } => Ok(()),
            Self::Shared { object, offset } => {
                map_shared_frames(object, *offset, area, address_space)
            }
        }
    }

    fn unmap(
        &self,
        area: &VmArea,
        range: VirtAddrRange,
        address_space: &mut AddressSpace,
    ) -> XResult {
        match self {
            Self::Static { .. } => {
                address_space.unmap_static_range(range.start, range.size(), area.page_size)
            }
            Self::Private { .. } | Self::Shared { .. } => {
                address_space.unmap_alloc_range(range.start, range.size())
            }
        }
    }

    fn protect(
        &self,
        area: &VmArea,
        range: VirtAddrRange,
        flags: MappingFlags,
        transaction: &mut ProtectionTransaction<'_>,
    ) -> XResult {
        match self {
            Self::Static { frames } => {
                let offset = range.start - area.range.start;
                transaction.protect_static_range(
                    range.start,
                    frames.subrange(offset, range.size())?,
                    flags,
                    area.page_size,
                )
            }
            Self::Private { .. } if flags.contains(MappingFlags::WRITE) => transaction
                .protect_alloc_range_with(range.start, range.size(), |exclusive| {
                    if exclusive {
                        flags
                    } else {
                        flags - MappingFlags::WRITE
                    }
                }),
            Self::Private { .. } | Self::Shared { .. } => {
                transaction.protect_alloc_range(range.start, range.size(), flags)
            }
        }
    }

    fn map_child(&self, area: &VmArea, parent: &AddressSpace, child: &mut AddressSpace) -> XResult {
        match self {
            Self::Static { frames } => {
                child.map_static_range(area.range.start, *frames, area.flags, area.page_size)
            }
            Self::Shared { object, offset } => map_shared_frames(object, *offset, area, child),
            Self::Private { .. } => {
                for (address, frame, flags) in parent.mapped_frames(area.range)? {
                    child.map_frame(address, frame, flags - MappingFlags::WRITE)?;
                }
                Ok(())
            }
        }
    }

    fn protect_parent_after_fork(
        &self,
        area: &VmArea,
        transaction: &mut ProtectionTransaction<'_>,
    ) -> XResult {
        if self.is_private() && area.flags.contains(MappingFlags::WRITE) {
            transaction.protect_alloc_range(
                area.range.start,
                area.range.size(),
                area.flags - MappingFlags::WRITE,
            )?;
        }
        Ok(())
    }

    fn resolve_fault(&self, area: &VmArea, page: VirtAddr, space: &mut VmSpace) -> FaultResolution {
        match self {
            Self::Private { source: None, .. } => {
                let Some(frame) = Frame::allocate_zeroed() else {
                    return FaultResolution::NoMemory;
                };
                match space.address_space.map_frame(page, frame, area.flags) {
                    Ok(()) => FaultResolution::Resolved,
                    Err(XError::NoMemory) => FaultResolution::NoMemory,
                    Err(_) => FaultResolution::Segv,
                }
            }
            Self::Private {
                source: Some(source),
                offset,
            } => space.resolve_source_fault(page, area, source.as_ref(), *offset),
            Self::Static { .. } | Self::Shared { .. } => FaultResolution::Segv,
        }
    }
}

fn map_shared_frames(
    object: &SharedObject,
    offset: usize,
    area: &VmArea,
    address_space: &mut AddressSpace,
) -> XResult {
    debug_assert_eq!(area.page_size, PageSize::Size4K);
    let page_size = usize::from(PageSize::Size4K);
    let first_frame = offset / page_size;
    let pages = PageIter4K::new(area.range.start, area.range.end).ok_or(XError::InvalidInput)?;

    for (mapped, address) in pages.enumerate() {
        let result = object
            .frame(first_frame + mapped)
            .ok_or(XError::BadState)
            .and_then(|frame| address_space.map_frame(address, frame, area.flags));
        if let Err(error) = result {
            if mapped != 0 {
                address_space
                    .unmap_alloc_range(area.range.start, mapped * page_size)
                    .expect("shared-map rollback range must be removable");
            }
            return Err(error);
        }
    }
    Ok(())
}

impl Backing {
    pub(super) fn shared_at(&self, delta: usize) -> Option<(usize, Arc<SharedObject>)> {
        let Self::Shared { object, offset } = self else {
            return None;
        };
        Some((offset.checked_add(delta)?, object.clone()))
    }
}
