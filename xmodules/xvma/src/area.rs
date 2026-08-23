// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use memory_addr::{MemoryAddr, VirtAddrRange};
use xerrno::XResult;
use xmm::{AddressSpace, MappingFlags, PageSize, ProtectionTransaction};

use crate::backend::Backing;

#[derive(Clone)]
pub(super) struct VmArea {
    pub(super) range: VirtAddrRange,
    pub(super) flags: MappingFlags,
    pub(super) page_size: PageSize,
    pub(super) backing: Backing,
}

impl VmArea {
    /// The part of `range` this area covers.
    pub(super) fn overlap_with(&self, range: VirtAddrRange) -> VirtAddrRange {
        VirtAddrRange::new(
            self.range.start.max(range.start),
            self.range.end.min(range.end),
        )
    }

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
        self.backing.private
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
    use xerrno::{LinuxError, LinuxResult};
    use xmm::{MappingFlags, PageSize, StaticFrameRange};

    use super::VmArea;
    use crate::{
        VmObject, VmPage,
        backend::{Backing, Source},
    };

    #[repr(align(4096))]
    struct AlignedBytes([u8; 0x3000]);

    static STATIC_BYTES: AlignedBytes = AlignedBytes([0; 0x3000]);

    struct TestObject(u64);

    impl VmObject for TestObject {
        fn id(&self) -> u64 {
            self.0
        }

        fn byte_len(&self) -> LinuxResult<u64> {
            Ok(0)
        }

        fn page(&self, _index: u64, _write: bool) -> LinuxResult<VmPage> {
            Err(LinuxError::ENOSYS)
        }
    }

    fn object(id: u64) -> Arc<dyn VmObject> {
        Arc::new(TestObject(id))
    }

    fn area(start: usize, size: usize, page_size: PageSize, backing: Backing) -> VmArea {
        VmArea {
            range: VirtAddrRange::from_start_size(VirtAddr::from(start), size),
            flags: MappingFlags::READ,
            page_size,
            backing,
        }
    }

    fn anonymous() -> Backing {
        Backing {
            source: Source::Zero,
            private: true,
        }
    }

    fn mapped(id: u64, offset: usize, private: bool) -> Backing {
        Backing {
            source: Source::Object {
                object: object(id),
                offset,
            },
            private,
        }
    }

    #[test]
    fn anonymous_slices_merge_when_equivalent() {
        let left = area(0x1000, 0x1000, PageSize::Size4K, anonymous());
        let right = area(0x2000, 0x1000, PageSize::Size4K, anonymous());
        assert!(VmArea::can_merge(&left, &right));
    }

    #[test]
    fn opposite_policies_never_merge() {
        let left = area(0x1000, 0x1000, PageSize::Size4K, mapped(1, 0, true));
        let right = area(0x2000, 0x1000, PageSize::Size4K, mapped(1, 0x1000, false));
        assert!(!VmArea::can_merge(&left, &right));
    }

    #[test]
    fn distinct_objects_never_merge() {
        let left = area(0x1000, 0x1000, PageSize::Size4K, mapped(1, 0, false));
        let right = area(0x2000, 0x1000, PageSize::Size4K, mapped(2, 0x1000, false));
        assert!(!VmArea::can_merge(&left, &right));
    }

    #[test]
    fn contiguous_object_offsets_merge() {
        let left = area(0x1000, 0x1000, PageSize::Size4K, mapped(1, 0x4000, false));
        let right = area(0x2000, 0x1000, PageSize::Size4K, mapped(1, 0x5000, false));
        assert!(VmArea::can_merge(&left, &right));
    }

    #[test]
    fn discontiguous_object_offsets_never_merge() {
        let left = area(0x1000, 0x1000, PageSize::Size4K, mapped(1, 0x4000, false));
        let right = area(0x2000, 0x1000, PageSize::Size4K, mapped(1, 0x9000, false));
        assert!(!VmArea::can_merge(&left, &right));
    }

    #[test]
    fn static_slice_advances_frame_origin() {
        let frames = StaticFrameRange::from_static_readonly(&STATIC_BYTES.0).unwrap();
        let region = area(
            0x1000,
            0x3000,
            PageSize::Size4K,
            Backing {
                source: Source::Static { frames },
                private: false,
            },
        );
        let tail = region
            .checked_slice(
                VirtAddrRange::from_start_size(VirtAddr::from(0x2000), 0x2000),
                region.flags,
            )
            .unwrap();
        let Source::Static {
            frames: tail_frames,
        } = tail.backing.source
        else {
            unreachable!();
        };
        assert_eq!(tail_frames.start(), frames.start() + 0x1000);
        assert_eq!(tail_frames.size(), 0x2000);
    }

    #[test]
    fn static_areas_keep_one_proof_token_each() {
        let frames = StaticFrameRange::from_static_readonly(&STATIC_BYTES.0).unwrap();
        let left = area(
            0x1000,
            0x1000,
            PageSize::Size4K,
            Backing {
                source: Source::Static {
                    frames: frames.subrange(0, 0x1000).unwrap(),
                },
                private: false,
            },
        );
        let right = area(
            0x2000,
            0x2000,
            PageSize::Size4K,
            Backing {
                source: Source::Static {
                    frames: frames.subrange(0x1000, 0x2000).unwrap(),
                },
                private: false,
            },
        );
        assert!(!VmArea::can_merge(&left, &right));
    }

    #[test]
    fn static_slice_must_follow_the_mapping_page_size() {
        let frames = StaticFrameRange::from_static_readonly(&STATIC_BYTES.0).unwrap();
        let region = area(
            0x20_0000,
            0x20_0000,
            PageSize::Size2M,
            Backing {
                source: Source::Static { frames },
                private: false,
            },
        );
        assert!(
            region
                .checked_slice(
                    VirtAddrRange::from_start_size(VirtAddr::from(0x20_0000), 0x1000),
                    region.flags,
                )
                .is_none()
        );
    }

    #[test]
    fn object_slice_advances_offset_and_keeps_identity() {
        let source = object(7);
        let region = area(
            0x1000,
            0x3000,
            PageSize::Size4K,
            Backing {
                source: Source::Object {
                    object: source.clone(),
                    offset: 0x4000,
                },
                private: true,
            },
        );
        let tail = region
            .checked_slice(
                VirtAddrRange::from_start_size(VirtAddr::from(0x2000), 0x2000),
                region.flags,
            )
            .unwrap();
        assert!(tail.backing.private);
        let Source::Object {
            object: tail_object,
            offset,
        } = tail.backing.source
        else {
            unreachable!();
        };
        assert!(Arc::ptr_eq(&source, &tail_object));
        assert_eq!(offset, 0x5000);
    }
}
