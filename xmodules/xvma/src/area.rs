// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use memory_addr::{MemoryAddr, VirtAddrRange};
use xerrno::XResult;
use xmm::{AddressSpace, MappingFlags, PageSize, ProtectionTransaction};

use crate::backend::{AreaBackend, Backing};

#[derive(Clone)]
pub(super) struct VmArea {
    pub(super) range: VirtAddrRange,
    pub(super) flags: MappingFlags,
    pub(super) page_size: PageSize,
    pub(super) backing: Backing,
}

impl VmArea {
    pub(super) fn checked_slice(&self, range: VirtAddrRange, flags: MappingFlags) -> Option<Self> {
        if !self.range.contains_range(range)
            || !range.start.is_aligned(self.page_size)
            || !range.size().is_multiple_of(usize::from(self.page_size))
        {
            return None;
        }
        Some(Self {
            range,
            flags,
            page_size: self.page_size,
            backing: self
                .backing
                .shifted(range.start - self.range.start, range.size())?,
        })
    }

    pub(super) fn is_private(&self) -> bool {
        self.backing.is_private()
    }

    pub(super) fn can_merge(left: &Self, right: &Self) -> bool {
        if left.range.end != right.range.start
            || left.flags != right.flags
            || left.page_size != right.page_size
        {
            return false;
        }
        left.backing.can_merge(&right.backing, left.range.size())
    }

    pub(super) fn map(&self, address_space: &mut AddressSpace) -> XResult {
        self.backing.map(self, address_space)
    }

    pub(super) fn unmap(&self, range: VirtAddrRange, address_space: &mut AddressSpace) -> XResult {
        self.backing.unmap(self, range, address_space)
    }

    pub(super) fn protect(
        &self,
        range: VirtAddrRange,
        flags: MappingFlags,
        transaction: &mut ProtectionTransaction<'_>,
    ) -> XResult {
        self.backing.protect(self, range, flags, transaction)
    }

    pub(super) fn map_child(&self, parent: &AddressSpace, child: &mut AddressSpace) -> XResult {
        self.backing.map_child(self, parent, child)
    }

    pub(super) fn protect_parent_after_fork(
        &self,
        transaction: &mut ProtectionTransaction<'_>,
    ) -> XResult {
        self.backing.protect_parent_after_fork(self, transaction)
    }
}

#[cfg(test)]
mod tests {
    use alloc::sync::Arc;
    use memory_addr::{VirtAddr, VirtAddrRange};
    use xerrno::LinuxResult;
    use xmm::{MappingFlags, PageSize, StaticFrameRange};

    use super::VmArea;
    use crate::{SharedObject, VmObject, backend::Backing};

    #[repr(align(4096))]
    struct AlignedBytes([u8; 0x3000]);

    static STATIC_BYTES: AlignedBytes = AlignedBytes([0; 0x3000]);

    struct TestObject;

    impl VmObject for TestObject {
        fn read_at(&self, _output: &mut [u8], _offset: u64) -> LinuxResult<usize> {
            Ok(0)
        }

        fn byte_len(&self) -> LinuxResult<u64> {
            Ok(0)
        }
    }

    #[test]
    fn anonymous_private_slices_merge_when_equivalent() {
        let flags = MappingFlags::READ | MappingFlags::USER;
        let left = VmArea {
            range: VirtAddrRange::from_start_size(VirtAddr::from(0x1000), 0x1000),
            flags,
            page_size: PageSize::Size4K,
            backing: Backing::Private {
                source: None,
                offset: 0,
            },
        };
        let right = VmArea {
            range: VirtAddrRange::from_start_size(VirtAddr::from(0x2000), 0x1000),
            flags,
            page_size: PageSize::Size4K,
            backing: Backing::Private {
                source: None,
                offset: 0,
            },
        };
        assert!(VmArea::can_merge(&left, &right));
    }

    #[test]
    fn static_slice_advances_frame_origin() {
        let frames = StaticFrameRange::from_static_readonly(&STATIC_BYTES.0).unwrap();
        let area = VmArea {
            range: VirtAddrRange::from_start_size(VirtAddr::from(0x1000), 0x3000),
            flags: MappingFlags::READ,
            page_size: PageSize::Size4K,
            backing: Backing::Static { frames },
        };
        let tail = area
            .checked_slice(
                VirtAddrRange::from_start_size(VirtAddr::from(0x2000), 0x2000),
                area.flags,
            )
            .unwrap();
        let Backing::Static {
            frames: tail_frames,
        } = tail.backing
        else {
            unreachable!();
        };
        assert_eq!(tail_frames.start(), frames.start() + 0x1000);
        assert_eq!(tail_frames.size(), 0x2000);
    }

    #[test]
    fn static_areas_keep_one_proof_token_each() {
        let frames = StaticFrameRange::from_static_readonly(&STATIC_BYTES.0).unwrap();
        let left = VmArea {
            range: VirtAddrRange::from_start_size(VirtAddr::from(0x1000), 0x1000),
            flags: MappingFlags::READ,
            page_size: PageSize::Size4K,
            backing: Backing::Static {
                frames: frames.subrange(0, 0x1000).unwrap(),
            },
        };
        let right = VmArea {
            range: VirtAddrRange::from_start_size(VirtAddr::from(0x2000), 0x2000),
            flags: MappingFlags::READ,
            page_size: PageSize::Size4K,
            backing: Backing::Static {
                frames: frames.subrange(0x1000, 0x2000).unwrap(),
            },
        };
        assert!(!VmArea::can_merge(&left, &right));
    }

    #[test]
    fn static_slice_must_follow_the_mapping_page_size() {
        let frames = StaticFrameRange::from_static_readonly(&STATIC_BYTES.0).unwrap();
        let area = VmArea {
            range: VirtAddrRange::from_start_size(VirtAddr::from(0x20_0000), 0x20_0000),
            flags: MappingFlags::READ,
            page_size: PageSize::Size2M,
            backing: Backing::Static { frames },
        };
        assert!(
            area.checked_slice(
                VirtAddrRange::from_start_size(VirtAddr::from(0x20_0000), 0x1000),
                area.flags,
            )
            .is_none()
        );
    }

    #[test]
    fn private_source_slice_advances_offset_and_keeps_identity() {
        let source: Arc<dyn VmObject> = Arc::new(TestObject);
        let area = VmArea {
            range: VirtAddrRange::from_start_size(VirtAddr::from(0x1000), 0x3000),
            flags: MappingFlags::READ,
            page_size: PageSize::Size4K,
            backing: Backing::Private {
                source: Some(source.clone()),
                offset: 0x4000,
            },
        };
        let tail = area
            .checked_slice(
                VirtAddrRange::from_start_size(VirtAddr::from(0x2000), 0x2000),
                area.flags,
            )
            .unwrap();
        let Backing::Private {
            source: Some(tail_source),
            offset,
        } = tail.backing
        else {
            unreachable!();
        };
        assert!(Arc::ptr_eq(&source, &tail_source));
        assert_eq!(offset, 0x5000);
    }

    #[test]
    fn shared_slice_advances_offset_and_keeps_identity() {
        let object = SharedObject::new(0).unwrap();
        let area = VmArea {
            range: VirtAddrRange::from_start_size(VirtAddr::from(0x1000), 0x3000),
            flags: MappingFlags::READ | MappingFlags::WRITE,
            page_size: PageSize::Size4K,
            backing: Backing::Shared {
                object: object.clone(),
                offset: 0x4000,
            },
        };
        let tail = area
            .checked_slice(
                VirtAddrRange::from_start_size(VirtAddr::from(0x3000), 0x1000),
                area.flags,
            )
            .unwrap();
        let Backing::Shared {
            object: tail_object,
            offset,
        } = tail.backing
        else {
            unreachable!();
        };
        assert!(Arc::ptr_eq(&object, &tail_object));
        assert_eq!(offset, 0x6000);
    }
}
