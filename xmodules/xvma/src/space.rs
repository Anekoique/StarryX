use alloc::{collections::BTreeMap, vec::Vec};
use core::ops::Range;
use memory_addr::{MemoryAddr, PhysAddr, VirtAddr, VirtAddrRange};
use xerrno::{XError, XResult};
use xmm::{AddressSpace, MappingFlags, PageSize};

use crate::{VmPageGuard, area::VmArea, backend::Backend};

/// The sole policy/layout owner of one user virtual address space.
pub struct VmSpace {
    pub(super) range: VirtAddrRange,
    pub(super) areas: BTreeMap<VirtAddr, VmArea>,
    pub(super) address_space: AddressSpace,
    /// Owners keeping individual pages writably mapped, ordered by address.
    write_guards: Vec<(VirtAddr, VmPageGuard)>,
}

impl VmSpace {
    pub fn new_empty(base: VirtAddr, size: usize) -> XResult<Self> {
        let range = VirtAddrRange::try_from_start_size(base, size).ok_or(XError::InvalidInput)?;
        Ok(Self {
            range,
            areas: BTreeMap::new(),
            address_space: AddressSpace::new_user(base, size)?,
            write_guards: Vec::new(),
        })
    }

    pub const fn base(&self) -> VirtAddr {
        self.range.start
    }

    pub const fn end(&self) -> VirtAddr {
        self.range.end
    }

    pub const fn page_table_root(&self) -> PhysAddr {
        self.address_space.page_table_root()
    }

    pub fn copy_kernel_mappings(&mut self) -> XResult {
        xmm::copy_kernel_mappings(&mut self.address_space)
    }

    /// Clones VMA policy and installs resident leaves using COW where needed.
    pub fn try_clone(&mut self) -> XResult<Self> {
        let mut child = Self::new_empty(self.range.start, self.range.size())?;
        child.areas = self.areas.clone();
        child
            .write_guards
            .try_reserve(self.write_guards.len())
            .map_err(|_| XError::NoMemory)?;
        child.write_guards.extend(
            self.write_guards
                .iter()
                .map(|(address, guard)| (*address, guard.clone())),
        );

        for area in self.areas.values() {
            area.map_child(&self.address_space, &mut child.address_space)?;
        }

        // Parent permissions change only after the complete child is ready.
        let mut transaction = self.address_space.begin_protection();
        for area in self.areas.values() {
            area.protect_parent_after_fork(&mut transaction)?;
        }
        transaction.commit();
        Ok(child)
    }

    pub fn find_free_area(
        &self,
        hint: VirtAddr,
        size: usize,
        limit: VirtAddrRange,
        page_size: PageSize,
    ) -> Option<VirtAddr> {
        if size == 0 || !size.is_multiple_of(usize::from(page_size)) {
            return None;
        }
        let search_start = limit.start.max(self.range.start);
        let search_end = limit.end.min(self.range.end);
        if search_start >= search_end {
            return None;
        }
        let search = VirtAddrRange::new(search_start, search_end);
        let mut candidate = hint.max(search.start).align_up(page_size);
        for area in self.areas.values() {
            if area.range.end <= candidate {
                continue;
            }
            if area.range.start >= search.end {
                break;
            }
            if candidate
                .checked_add(size)
                .is_some_and(|end| end <= area.range.start && end <= search.end)
            {
                return Some(candidate);
            }
            candidate = area.range.end.align_up(page_size);
        }
        candidate
            .checked_add(size)
            .filter(|end| *end <= search.end)
            .map(|_| candidate)
    }

    pub fn map(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        backend: Backend,
    ) -> XResult {
        let (page_size, backing, populate) = backend.prepare(size)?;
        let range = self.validate_new_area(start, size, page_size)?;
        let area = VmArea {
            range,
            flags,
            page_size,
            backing,
        };
        area.map(&mut self.address_space)?;
        self.insert_area(area);
        if populate && let Err(error) = self.populate_area(start, size, flags) {
            let area = self
                .areas
                .remove(&start)
                .expect("newly inserted VMA must remain present");
            area.unmap(range, &mut self.address_space)
                .expect("partially populated mapping must be removable");
            return Err(error);
        }
        Ok(())
    }

    /// Copies bytes from a logically readable VMA and readable PTEs.
    pub fn read_bytes(&self, start: VirtAddr, output: &mut [u8]) -> XResult {
        if output.is_empty() {
            return Ok(());
        }
        let range =
            VirtAddrRange::try_from_start_size(start, output.len()).ok_or(XError::InvalidInput)?;
        if !self.check_region_access(range, MappingFlags::READ) {
            return Err(XError::PermissionDenied);
        }
        self.address_space.read_bytes(start, output)
    }

    /// Copies bytes into a logically writable VMA backed by writable Alloc PTEs.
    pub fn write_bytes(&mut self, start: VirtAddr, input: &[u8]) -> XResult {
        if input.is_empty() {
            return Ok(());
        }
        let range =
            VirtAddrRange::try_from_start_size(start, input.len()).ok_or(XError::InvalidInput)?;
        if !self.check_region_access(range, MappingFlags::WRITE) {
            return Err(XError::PermissionDenied);
        }
        self.address_space.write_alloc_bytes(start, input)
    }

    pub fn check_region_access(&self, mut range: VirtAddrRange, access: MappingFlags) -> bool {
        while !range.is_empty() {
            let Some(area) = self.area_at(range.start) else {
                return false;
            };
            if !area.flags.contains(access) {
                return false;
            }
            range.start = area.range.end.min(range.end);
        }
        true
    }

    pub fn unmap(&mut self, start: VirtAddr, size: usize) -> XResult {
        let removed = self.validate_existing_range(start, size)?;
        self.validate_split_offsets(removed)?;
        for area in self
            .areas
            .values()
            .filter(|area| area.range.overlaps(removed))
        {
            let overlap = area.overlap_with(removed);
            area.unmap(overlap, &mut self.address_space)?;
        }
        self.retain_outside(removed);
        self.drop_write_guards(removed);
        Ok(())
    }

    pub fn protect(&mut self, start: VirtAddr, size: usize, flags: MappingFlags) -> XResult {
        let protected = self.validate_existing_range(start, size)?;
        if !self.check_region_access(protected, MappingFlags::empty()) {
            return Err(XError::NoMemory);
        }
        self.validate_split_offsets(protected)?;

        // Prepare the complete VMA commit before touching hardware. BTreeMap
        // insertion may allocate, so no map construction or merge is allowed
        // after the first PTE mutation.
        let mut committed_areas = BTreeMap::new();
        for area in self.areas.values() {
            if !area.range.overlaps(protected) {
                committed_areas.insert(area.range.start, area.clone());
                continue;
            }
            if area.range.start < protected.start {
                let left = area
                    .checked_slice(
                        VirtAddrRange::new(area.range.start, protected.start),
                        area.flags,
                    )
                    .expect("split offsets were prevalidated");
                committed_areas.insert(left.range.start, left);
            }
            let middle = area
                .checked_slice(area.overlap_with(protected), flags)
                .expect("split offsets were prevalidated");
            committed_areas.insert(middle.range.start, middle);
            if protected.end < area.range.end {
                let right = area
                    .checked_slice(
                        VirtAddrRange::new(protected.end, area.range.end),
                        area.flags,
                    )
                    .expect("split offsets were prevalidated");
                committed_areas.insert(right.range.start, right);
            }
        }
        committed_areas = Self::merged_area_map(committed_areas);

        let mut transaction = self.address_space.begin_protection();
        for area in self
            .areas
            .values()
            .filter(|area| area.range.overlaps(protected))
        {
            let overlap = area.overlap_with(protected);
            area.protect(overlap, flags, &mut transaction)?;
        }
        transaction.commit();
        self.areas = committed_areas;
        self.drop_write_guards(protected);
        Ok(())
    }

    pub fn unmap_user_areas(&mut self) -> XResult {
        for area in self.areas.values() {
            area.unmap(area.range, &mut self.address_space)?;
        }
        self.areas.clear();
        self.write_guards.clear();
        Ok(())
    }

    /// The identity and offset `address` maps to, if it is write-through.
    ///
    /// A private mapping has no answer: its stores are local, so anything keyed
    /// on the result would wrongly pair unrelated address spaces.
    pub fn shared_object_at(&self, address: VirtAddr) -> Option<(u64, usize)> {
        let area = self.area_at(address)?;
        if area.is_private() {
            return None;
        }
        let (offset, object) = area.backing.object_at(address - area.range.start)?;
        Some((object.id(), offset))
    }

    /// Whether any live VMA still draws from the object identified by `id`.
    pub fn maps_object(&self, id: u64) -> bool {
        self.areas
            .values()
            .filter_map(|area| area.backing.object_at(0))
            .any(|(_, object)| object.id() == id)
    }

    /// Checks that every leaf an invalidation would remove can be removed.
    ///
    /// This is the only failable half of the protocol; the caller may abort
    /// after it and leave the address space untouched.
    pub fn validate_object_range(&self, id: u64, range: &Range<u64>) -> XResult {
        if range.start >= range.end {
            return Ok(());
        }
        for area in self.areas.values() {
            if let Some(invalidated) = Self::invalidated_range(area, id, range) {
                self.address_space
                    .validate_alloc_range(invalidated.start, invalidated.size())?;
            }
        }
        Ok(())
    }

    /// Applies an invalidation accepted by [`Self::validate_object_range`].
    pub fn unmap_object_range(&mut self, id: u64, range: &Range<u64>) {
        for area in self.areas.values() {
            if let Some(invalidated) = Self::invalidated_range(area, id, range) {
                area.unmap(invalidated, &mut self.address_space)
                    .expect("preflighted object-backed leaves must remain removable");
                self.write_guards
                    .retain(|(address, _)| !invalidated.contains(*address));
            }
        }
    }

    fn invalidated_range(
        area: &VmArea,
        id: u64,
        invalidated: &Range<u64>,
    ) -> Option<VirtAddrRange> {
        let (area_offset, object) = area.backing.object_at(0)?;
        if object.id() != id {
            return None;
        }
        let area_start = area_offset as u64;
        let area_end = area_start
            .checked_add(area.range.size() as u64)
            .expect("validated object VMA offset must remain bounded");
        let overlap_start = area_start.max(invalidated.start);
        let overlap_end = area_end.min(invalidated.end);
        if overlap_start >= overlap_end {
            return None;
        }
        let start = area
            .range
            .start
            .checked_add((overlap_start - area_start) as usize)
            .expect("object overlap must lie inside its VMA")
            .align_down_4k();
        let end = area
            .range
            .start
            .checked_add((overlap_end - area_start) as usize)
            .expect("object overlap must lie inside its VMA")
            .align_up_4k()
            .min(area.range.end);
        Some(VirtAddrRange::new(start, end))
    }

    /// Writes back every write-through object range overlapping `range`.
    pub fn sync_object_range(&self, range: VirtAddrRange, wait: bool) -> xerrno::LinuxResult {
        for area in self
            .areas
            .values()
            .filter(|area| area.range.overlaps(range))
        {
            if area.is_private() {
                continue;
            }
            let Some((area_offset, object)) = area.backing.object_at(0) else {
                continue;
            };
            let overlap = area.overlap_with(range);
            let start = area_offset as u64 + (overlap.start - area.range.start) as u64;
            let end = start + overlap.size() as u64;
            object.sync(start..end, wait)?;
        }
        Ok(())
    }

    pub(super) fn reserve_write_guard(&mut self) -> XResult {
        self.write_guards
            .try_reserve(1)
            .map_err(|_| XError::NoMemory)
    }

    pub(super) fn insert_write_guard(&mut self, address: VirtAddr, guard: VmPageGuard) {
        debug_assert!(self.write_guards.len() < self.write_guards.capacity());
        let index = self
            .write_guards
            .binary_search_by_key(&address, |(address, _)| *address)
            .unwrap_or_else(|index| index);
        self.write_guards.insert(index, (address, guard));
    }

    fn drop_write_guards(&mut self, range: VirtAddrRange) {
        self.write_guards
            .retain(|(address, _)| !range.contains(*address));
    }

    fn validate_new_area(
        &self,
        start: VirtAddr,
        size: usize,
        page_size: PageSize,
    ) -> XResult<VirtAddrRange> {
        let range = self.validate_range(start, size, page_size)?;
        if self.areas.values().any(|area| area.range.overlaps(range)) {
            return Err(XError::AlreadyExists);
        }
        Ok(range)
    }

    pub(super) fn validate_existing_range(
        &self,
        start: VirtAddr,
        size: usize,
    ) -> XResult<VirtAddrRange> {
        self.validate_range(start, size, PageSize::Size4K)
    }

    fn validate_range(
        &self,
        start: VirtAddr,
        size: usize,
        page_size: PageSize,
    ) -> XResult<VirtAddrRange> {
        if size == 0 || !start.is_aligned(page_size) || !size.is_multiple_of(usize::from(page_size))
        {
            return Err(XError::InvalidInput);
        }
        let range = VirtAddrRange::try_from_start_size(start, size).ok_or(XError::InvalidInput)?;
        self.range
            .contains_range(range)
            .then_some(range)
            .ok_or(XError::InvalidInput)
    }

    fn validate_split_offsets(&self, changed: VirtAddrRange) -> XResult {
        for area in self
            .areas
            .values()
            .filter(|area| area.range.overlaps(changed))
        {
            if area.range.start < changed.start {
                area.checked_slice(
                    VirtAddrRange::new(area.range.start, changed.start),
                    area.flags,
                )
                .ok_or(XError::InvalidInput)?;
            }
            if changed.end < area.range.end {
                area.checked_slice(VirtAddrRange::new(changed.end, area.range.end), area.flags)
                    .ok_or(XError::InvalidInput)?;
            }
        }
        Ok(())
    }

    fn insert_area(&mut self, area: VmArea) {
        let previous = self.areas.insert(area.range.start, area);
        debug_assert!(previous.is_none());
    }

    pub(super) fn area_at(&self, address: VirtAddr) -> Option<&VmArea> {
        self.areas
            .range(..=address)
            .next_back()
            .map(|(_, area)| area)
            .filter(|area| area.range.contains(address))
    }

    fn retain_outside(&mut self, removed: VirtAddrRange) {
        let old = core::mem::take(&mut self.areas);
        for area in old.into_values() {
            if !area.range.overlaps(removed) {
                self.insert_area(area);
                continue;
            }
            if area.range.start < removed.start {
                self.insert_area(
                    area.checked_slice(
                        VirtAddrRange::new(area.range.start, removed.start),
                        area.flags,
                    )
                    .expect("split offsets were prevalidated"),
                );
            }
            if removed.end < area.range.end {
                self.insert_area(
                    area.checked_slice(VirtAddrRange::new(removed.end, area.range.end), area.flags)
                        .expect("split offsets were prevalidated"),
                );
            }
        }
        self.merge_adjacent();
    }

    fn merge_adjacent(&mut self) {
        self.areas = Self::merged_area_map(core::mem::take(&mut self.areas));
    }

    fn merged_area_map(old: BTreeMap<VirtAddr, VmArea>) -> BTreeMap<VirtAddr, VmArea> {
        let mut merged = BTreeMap::new();
        let mut current: Option<VmArea> = None;
        for area in old.into_values() {
            if let Some(mut previous) = current.take() {
                if VmArea::can_merge(&previous, &area) {
                    previous.range = VirtAddrRange::new(previous.range.start, area.range.end);
                    current = Some(previous);
                } else {
                    merged.insert(previous.range.start, previous);
                    current = Some(area);
                }
            } else {
                current = Some(area);
            }
        }
        if let Some(area) = current {
            merged.insert(area.range.start, area);
        }
        merged
    }
}

impl Drop for VmSpace {
    fn drop(&mut self) {
        self.unmap_user_areas()
            .expect("valid VM areas must be removable during drop");
    }
}
