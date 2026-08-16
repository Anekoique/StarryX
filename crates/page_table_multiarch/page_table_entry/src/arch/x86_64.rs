//! x86 page table entries on 64-bit paging.

use core::fmt;
use memory_addr::PhysAddr;

pub use x86_64::structures::paging::page_table::PageTableFlags as PTF;

use crate::{GenericPTE, MappingFlags};

const ALLOC_FRAME_BIT: u64 = 1 << 9;
const PROT_NONE_BIT: u64 = 1 << 10;

impl From<PTF> for MappingFlags {
    fn from(f: PTF) -> Self {
        if f.bits() & PROT_NONE_BIT != 0 {
            let mut flags = Self::PROT_NONE;
            if f.bits() & ALLOC_FRAME_BIT != 0 {
                flags |= Self::ALLOC_FRAME;
            }
            return flags;
        }
        if !f.contains(PTF::PRESENT) {
            return Self::empty();
        }
        let mut ret = Self::READ;
        if f.contains(PTF::WRITABLE) {
            ret |= Self::WRITE;
        }
        if !f.contains(PTF::NO_EXECUTE) {
            ret |= Self::EXECUTE;
        }
        if f.contains(PTF::USER_ACCESSIBLE) {
            ret |= Self::USER;
        }
        if f.contains(PTF::NO_CACHE) {
            ret |= Self::UNCACHED;
        }
        if f.bits() & ALLOC_FRAME_BIT != 0 {
            ret |= Self::ALLOC_FRAME;
        }
        ret
    }
}

impl From<MappingFlags> for PTF {
    fn from(f: MappingFlags) -> Self {
        if f.contains(MappingFlags::PROT_NONE) {
            let bits = PROT_NONE_BIT
                | if f.contains(MappingFlags::ALLOC_FRAME) {
                    ALLOC_FRAME_BIT
                } else {
                    0
                };
            return Self::from_bits_retain(bits);
        }
        if f.is_empty() {
            return Self::empty();
        }
        let mut ret = Self::PRESENT;
        if f.contains(MappingFlags::WRITE) {
            ret |= Self::WRITABLE;
        }
        if !f.contains(MappingFlags::EXECUTE) {
            ret |= Self::NO_EXECUTE;
        }
        if f.contains(MappingFlags::USER) {
            ret |= Self::USER_ACCESSIBLE;
        }
        if f.contains(MappingFlags::DEVICE) || f.contains(MappingFlags::UNCACHED) {
            ret |= Self::NO_CACHE | Self::WRITE_THROUGH;
        }
        if f.contains(MappingFlags::ALLOC_FRAME) {
            ret |= Self::from_bits_retain(ALLOC_FRAME_BIT);
        }
        ret
    }
}

/// An x86_64 page table entry.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct X64PTE(u64);

impl X64PTE {
    const PHYS_ADDR_MASK: u64 = 0x000f_ffff_ffff_f000; // bits 12..52

    /// Creates an empty descriptor with all bits set to zero.
    pub const fn empty() -> Self {
        Self(0)
    }

    fn encode_paddr(paddr: PhysAddr, prot_none: bool) -> u64 {
        let bits = paddr.as_usize() as u64 & Self::PHYS_ADDR_MASK;
        if prot_none {
            !bits & Self::PHYS_ADDR_MASK
        } else {
            bits
        }
    }

    fn is_prot_none(&self) -> bool {
        self.0 & PROT_NONE_BIT != 0
    }
}

impl GenericPTE for X64PTE {
    fn new_page(paddr: PhysAddr, flags: MappingFlags, is_huge: bool) -> Self {
        let prot_none = flags.contains(MappingFlags::PROT_NONE);
        let mut flags = PTF::from(flags);
        if is_huge {
            flags |= PTF::HUGE_PAGE;
        }
        Self(flags.bits() | Self::encode_paddr(paddr, prot_none))
    }
    fn new_table(paddr: PhysAddr) -> Self {
        let flags = PTF::PRESENT | PTF::WRITABLE | PTF::USER_ACCESSIBLE;
        Self(flags.bits() | (paddr.as_usize() as u64 & Self::PHYS_ADDR_MASK))
    }
    fn paddr(&self) -> PhysAddr {
        let bits = self.0 & Self::PHYS_ADDR_MASK;
        let bits = if self.is_prot_none() {
            !bits & Self::PHYS_ADDR_MASK
        } else {
            bits
        };
        PhysAddr::from(bits as usize)
    }
    fn flags(&self) -> MappingFlags {
        PTF::from_bits_truncate(self.0).into()
    }
    fn set_paddr(&mut self, paddr: PhysAddr) {
        self.0 = (self.0 & !Self::PHYS_ADDR_MASK) | Self::encode_paddr(paddr, self.is_prot_none())
    }
    fn set_flags(&mut self, flags: MappingFlags, is_huge: bool) {
        let paddr = self.paddr();
        let prot_none = flags.contains(MappingFlags::PROT_NONE);
        let mut flags = PTF::from(flags);
        if is_huge {
            flags |= PTF::HUGE_PAGE;
        }
        self.0 = Self::encode_paddr(paddr, prot_none) | flags.bits()
    }

    fn bits(self) -> usize {
        self.0 as usize
    }
    fn is_unused(&self) -> bool {
        self.0 == 0
    }
    fn is_present(&self) -> bool {
        PTF::from_bits_truncate(self.0).contains(PTF::PRESENT) || self.0 & PROT_NONE_BIT != 0
    }
    fn is_huge(&self) -> bool {
        PTF::from_bits_truncate(self.0).contains(PTF::HUGE_PAGE)
    }
    fn clear(&mut self) {
        self.0 = 0
    }
}

impl fmt::Debug for X64PTE {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_struct("X64PTE");
        f.field("raw", &self.0)
            .field("paddr", &self.paddr())
            .field("flags", &self.flags())
            .finish()
    }
}
