// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

use core::{
    fmt, mem,
    sync::atomic::{AtomicU32, Ordering},
};

use kernel_guard::NoPreemptIrqSave;
use lazyinit::LazyInit;
use memory_addr::{MemoryAddr, PhysAddr, VirtAddr};
use xalloc::global_allocator;
use xerrno::{XError, XResult};
use xhal::mem::{phys_to_virt, virt_to_phys};

use crate::{MappingFlags, PAGE_SIZE_4K};

/// The lifetime model behind a leaf page-table entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FrameKind {
    /// An allocator-backed frame whose PTE owns one counted [`Frame`] reference.
    Alloc,
    /// A kernel-long physical range whose lifetime is guaranteed externally.
    Static,
}

impl FrameKind {
    pub(crate) fn from_flags(flags: MappingFlags) -> Self {
        if flags.contains(MappingFlags::ALLOC_FRAME) {
            Self::Alloc
        } else {
            Self::Static
        }
    }

    pub(crate) fn apply(self, flags: MappingFlags) -> MappingFlags {
        match self {
            Self::Alloc => flags | MappingFlags::ALLOC_FRAME,
            Self::Static => flags - MappingFlags::ALLOC_FRAME,
        }
    }
}

/// Intrusive reference-count control block for one allocator-backed frame.
///
/// This is the frame equivalent of an `Arc` control block. It remains private:
/// policy and mapping state belong to `xvma` and the page table respectively.
struct FrameMeta {
    ref_count: AtomicU32,
}

impl FrameMeta {
    const fn new() -> Self {
        Self {
            ref_count: AtomicU32::new(0),
        }
    }

    fn claim(&self) -> bool {
        self.ref_count
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn try_get(&self) -> bool {
        self.ref_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count != 0 && count != u32::MAX).then_some(count + 1)
            })
            .is_ok()
    }

    fn put(&self) -> bool {
        let previous = self
            .ref_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count.checked_sub(1)
            })
            .expect("frame reference count underflow");
        previous == 1
    }

    fn is_unique(&self) -> bool {
        self.ref_count.load(Ordering::Acquire) == 1
    }
}

struct FrameDatabase {
    base: PhysAddr,
    metadata_start: VirtAddr,
    frame_count: usize,
}

impl FrameDatabase {
    fn metadata(&self, paddr: PhysAddr) -> Option<&FrameMeta> {
        let offset = paddr.as_usize().checked_sub(self.base.as_usize())?;
        if !offset.is_multiple_of(PAGE_SIZE_4K) {
            return None;
        }
        let index = offset / PAGE_SIZE_4K;
        if index >= self.frame_count {
            return None;
        }
        // SAFETY: initialization reserved and initialized exactly frame_count
        // contiguous FrameMeta values at metadata_start for the kernel lifetime.
        Some(unsafe { &*(self.metadata_start.as_ptr_of::<FrameMeta>().add(index)) })
    }
}

static FRAME_DATABASE: LazyInit<FrameDatabase> = LazyInit::new();

/// Reserves and initializes the PFN-indexed frame metadata database.
///
/// This runs before the page allocator receives `storage`, so the returned
/// prefix must be excluded from the allocator's free range.
pub fn init_frame_database(storage_start: VirtAddr, storage_size: usize) -> usize {
    assert!(
        FRAME_DATABASE.get().is_none(),
        "frame metadata database initialized twice"
    );
    let base = PhysAddr::from(xconfig::plat::PHYS_MEMORY_BASE);
    let frame_count = xconfig::plat::PHYS_MEMORY_SIZE / PAGE_SIZE_4K;
    assert!(frame_count != 0, "physical memory must contain base frames");
    let metadata_bytes = frame_count
        .checked_mul(mem::size_of::<FrameMeta>())
        .expect("frame metadata size overflow");
    let reserved_bytes = metadata_bytes
        .checked_next_multiple_of(PAGE_SIZE_4K)
        .expect("frame metadata alignment overflow");
    assert!(
        reserved_bytes < storage_size,
        "free memory region is too small for frame metadata"
    );
    assert!(
        storage_start
            .as_usize()
            .is_multiple_of(mem::align_of::<FrameMeta>()),
        "frame metadata storage is misaligned"
    );
    let metadata = storage_start.as_mut_ptr_of::<FrameMeta>();
    for index in 0..frame_count {
        // SAFETY: the caller supplied an exclusive free-memory prefix large
        // enough for every descriptor, and this prefix is reserved below.
        unsafe { metadata.add(index).write(FrameMeta::new()) };
    }
    FRAME_DATABASE.init_once(FrameDatabase {
        base,
        metadata_start: storage_start,
        frame_count,
    });
    info!(
        "frame metadata: {} frames, {} KiB",
        frame_count,
        reserved_bytes / 1024
    );
    reserved_bytes
}

fn database() -> &'static FrameDatabase {
    FRAME_DATABASE
        .get()
        .expect("frame metadata database is not initialized")
}

fn metadata(paddr: PhysAddr) -> XResult<&'static FrameMeta> {
    database().metadata(paddr).ok_or(XError::BadAddress)
}

fn release_reference(paddr: PhysAddr) {
    let meta = metadata(paddr).expect("allocated frame lies outside physical memory");
    if meta.put() {
        global_allocator().dealloc_pages(phys_to_virt(paddr).as_usize(), 1);
    }
}

/// A counted handle to one allocator-backed 4-KiB physical frame.
///
/// Every `Frame` value and every `FrameKind::Alloc` PTE owns exactly one
/// reference in the frame's private [`FrameMeta`].
pub struct Frame {
    paddr: PhysAddr,
}

impl Frame {
    /// Allocates and zeroes one frame with an initial reference count of one.
    pub fn allocate_zeroed() -> Option<Self> {
        let vaddr = VirtAddr::from(global_allocator().alloc_pages(1, PAGE_SIZE_4K).ok()?);
        let paddr = virt_to_phys(vaddr);
        let Ok(meta) = metadata(paddr) else {
            global_allocator().dealloc_pages(vaddr.as_usize(), 1);
            return None;
        };
        assert!(
            meta.claim(),
            "allocator returned an already referenced frame"
        );
        // SAFETY: the allocator returned a unique live base frame.
        unsafe { core::ptr::write_bytes(vaddr.as_mut_ptr(), 0, PAGE_SIZE_4K) };
        Some(Self { paddr })
    }

    pub(crate) const fn physical_address(&self) -> PhysAddr {
        self.paddr
    }

    /// Writes only while this handle is the frame's sole owner.
    ///
    /// `&mut Frame` alone does not imply exclusive physical memory because the
    /// handle is cloneable. A count of one proves that no clone or Alloc PTE can
    /// concurrently expose this frame.
    pub fn try_write_at(&mut self, offset: usize, source: &[u8]) -> bool {
        let Some(end) = offset.checked_add(source.len()) else {
            return false;
        };
        if end > PAGE_SIZE_4K
            || !metadata(self.paddr)
                .expect("allocated frame lies outside physical memory")
                .is_unique()
        {
            return false;
        }
        let destination = phys_to_virt(self.paddr) + offset;
        // SAFETY: the checked reference count proves exclusive access to this
        // live frame, and the destination range lies within it.
        unsafe {
            core::ptr::copy_nonoverlapping(source.as_ptr(), destination.as_mut_ptr(), source.len());
        };
        true
    }

    /// Returns whether this is the only counted handle to the frame.
    pub fn is_unique(&self) -> bool {
        metadata(self.paddr)
            .expect("allocated frame lies outside physical memory")
            .is_unique()
    }

    fn access_at(&self, offset: usize, len: usize) -> XResult<(VirtAddr, NoPreemptIrqSave)> {
        if xconfig::SMP != 1 {
            return Err(XError::Unsupported);
        }
        let end = offset.checked_add(len).ok_or(XError::InvalidInput)?;
        if end > PAGE_SIZE_4K {
            return Err(XError::InvalidInput);
        }
        Ok((phys_to_virt(self.paddr) + offset, NoPreemptIrqSave::new()))
    }

    /// Copies bytes from a live frame without exposing an aliased Rust slice.
    pub fn read_bytes(&self, offset: usize, destination: &mut [u8]) -> XResult {
        let (source, _guard) = self.access_at(offset, destination.len())?;
        // SAFETY: Frame keeps the page live, access_at bounds the copy and
        // serializes local execution, and destination is an exclusive slice.
        unsafe {
            core::ptr::copy_nonoverlapping(
                source.as_ptr(),
                destination.as_mut_ptr(),
                destination.len(),
            );
        }
        Ok(())
    }

    /// Copies bytes into a live ordinary frame without exposing a Rust slice.
    pub fn write_bytes(&self, offset: usize, source: &[u8]) -> XResult {
        let (destination, _guard) = self.access_at(offset, source.len())?;
        // SAFETY: access_at keeps the page live and confines the copy while
        // interrupts and preemption are disabled in the supported one-hart
        // execution model.
        unsafe {
            core::ptr::copy_nonoverlapping(source.as_ptr(), destination.as_mut_ptr(), source.len());
        }
        Ok(())
    }

    pub fn deep_copy(&self) -> Option<Self> {
        let copy = Self::allocate_zeroed()?;
        let _guard = NoPreemptIrqSave::new();
        // SAFETY: both counted handles keep distinct 4-KiB frames live, and
        // the newly allocated destination has no aliases. Local execution is
        // serialized for the complete snapshot in the supported one-hart model.
        unsafe {
            core::ptr::copy_nonoverlapping(
                phys_to_virt(self.paddr).as_ptr(),
                phys_to_virt(copy.paddr).as_mut_ptr(),
                PAGE_SIZE_4K,
            );
        }
        Some(copy)
    }

    /// Transfers this reference into a newly installed Alloc PTE.
    pub(crate) fn into_pte(self) -> PhysAddr {
        let paddr = self.paddr;
        mem::forget(self);
        paddr
    }

    /// Clones the reference owned by a live Alloc PTE.
    pub(crate) fn clone_from_pte(paddr: PhysAddr) -> XResult<Self> {
        let meta = metadata(paddr)?;
        if !meta.try_get() {
            return Err(XError::BadState);
        }
        Ok(Self { paddr })
    }

    /// The metadata of a frame that a live PTE must still reference.
    fn live_metadata(paddr: PhysAddr) -> XResult<&'static FrameMeta> {
        let meta = metadata(paddr)?;
        if meta.ref_count.load(Ordering::Acquire) == 0 {
            return Err(XError::BadState);
        }
        Ok(meta)
    }

    pub(crate) fn pte_is_unique(paddr: PhysAddr) -> XResult<bool> {
        Ok(Self::live_metadata(paddr)?.is_unique())
    }

    pub(crate) fn validate_pte(paddr: PhysAddr) -> XResult {
        Self::live_metadata(paddr).map(|_| ())
    }

    /// Reconstructs the reference owned by a removed Alloc PTE.
    ///
    /// # Safety
    ///
    /// The caller must own one reference previously transferred with
    /// [`Frame::into_pte`] and must reconstruct it exactly once.
    pub(crate) unsafe fn take_from_pte(paddr: PhysAddr) -> XResult<Self> {
        Self::validate_pte(paddr)?;
        Ok(Self { paddr })
    }
}

impl Clone for Frame {
    fn clone(&self) -> Self {
        assert!(
            metadata(self.paddr)
                .expect("allocated frame lies outside physical memory")
                .try_get(),
            "cannot clone a released frame"
        );
        Self { paddr: self.paddr }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        release_reference(self.paddr);
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let meta = metadata(self.paddr).map_err(|_| fmt::Error)?;
        formatter
            .debug_struct("Frame")
            .field("paddr", &self.paddr)
            .field("ref_count", &meta.ref_count.load(Ordering::Relaxed))
            .finish()
    }
}

/// Proof that a physical frame range remains valid for the kernel lifetime.
///
/// This token owns no counted frame reference. It is intended for
/// firmware-defined memory, MMIO, and statically linked kernel data that
/// cannot be returned to the allocator while a mapping exists.
#[derive(Clone, Copy, Debug)]
pub struct StaticFrameRange {
    start: PhysAddr,
    size: usize,
    allowed_flags: MappingFlags,
}

impl StaticFrameRange {
    fn from_static_value<T: ?Sized + Sync>(
        value: &'static T,
        allowed_flags: MappingFlags,
    ) -> XResult<Self> {
        let address = core::ptr::from_ref(value).cast::<u8>() as usize;
        let size = core::mem::size_of_val(value);
        if size == 0 || !address.is_multiple_of(PAGE_SIZE_4K) || !size.is_multiple_of(PAGE_SIZE_4K)
        {
            return Err(XError::InvalidInput);
        }
        // SAFETY: the shared static reference proves lifetime and shared
        // access. The public constructors expose only read or execute access.
        unsafe { Self::new(virt_to_phys(address.into()), size, allowed_flags) }
    }

    /// Creates a read-only proof for a page-aligned static value.
    pub fn from_static_readonly<T: ?Sized + Sync>(value: &'static T) -> XResult<Self> {
        Self::from_static_value(value, MappingFlags::READ)
    }

    /// Creates a read/execute proof for page-aligned immutable static code.
    pub fn from_static_code(code: &'static [u8]) -> XResult<Self> {
        Self::from_static_value(code, MappingFlags::READ | MappingFlags::EXECUTE)
    }

    /// Creates a static physical-frame-range proof.
    ///
    /// # Safety
    ///
    /// `start..start + size` must remain physically valid for the kernel
    /// lifetime. Every access described by `allowed_flags` must be compatible
    /// with the underlying memory's Rust aliasing and device contracts.
    pub unsafe fn new(start: PhysAddr, size: usize, allowed_flags: MappingFlags) -> XResult<Self> {
        if size == 0 || start.checked_add(size).is_none() {
            return Err(XError::InvalidInput);
        }
        let allowed_flags = allowed_flags
            - MappingFlags::USER
            - MappingFlags::ALLOC_FRAME
            - MappingFlags::PROT_NONE;
        Ok(Self {
            start,
            size,
            allowed_flags,
        })
    }

    pub const fn start(self) -> PhysAddr {
        self.start
    }

    pub const fn size(self) -> usize {
        self.size
    }

    pub fn allows(self, flags: MappingFlags) -> bool {
        let requested =
            flags - MappingFlags::USER - MappingFlags::ALLOC_FRAME - MappingFlags::PROT_NONE;
        self.allowed_flags.contains(requested)
    }

    pub fn subrange(self, offset: usize, size: usize) -> XResult<Self> {
        let end = offset.checked_add(size).ok_or(XError::InvalidInput)?;
        if size == 0 || end > self.size {
            return Err(XError::InvalidInput);
        }
        Ok(Self {
            start: self.start.checked_add(offset).ok_or(XError::InvalidInput)?,
            size,
            allowed_flags: self.allowed_flags,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameKind, FrameMeta, MappingFlags};

    #[test]
    fn frame_meta_is_a_checked_intrusive_reference_count() {
        let meta = FrameMeta::new();
        assert!(meta.claim());
        assert!(!meta.claim());
        assert!(meta.try_get());
        assert!(!meta.put());
        assert!(meta.put());
        assert!(!meta.try_get());
    }

    #[test]
    fn frame_kind_owns_the_pte_software_bit() {
        let flags = MappingFlags::READ;
        assert_eq!(FrameKind::from_flags(flags), FrameKind::Static);
        assert_eq!(
            FrameKind::from_flags(FrameKind::Alloc.apply(flags)),
            FrameKind::Alloc
        );
        assert_eq!(
            FrameKind::Static.apply(FrameKind::Alloc.apply(flags)),
            flags
        );
    }
}
