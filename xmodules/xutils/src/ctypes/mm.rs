use axhal::paging::MappingFlags;
use strum::FromRepr;

use super::{
    MAP_ANONYMOUS, MAP_FIXED, MAP_FIXED_NOREPLACE, MAP_HUGE_1GB, MAP_HUGETLB, MAP_NORESERVE,
    MAP_POPULATE, MAP_PRIVATE, MAP_SHARED, MAP_SHARED_VALIDATE, MAP_STACK, MAP_TYPE, PROT_EXEC,
    PROT_GROWSDOWN, PROT_GROWSUP, PROT_READ, PROT_WRITE,
};

bitflags::bitflags! {
    /// flags for sys_mmap
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub struct MmapFlags: u32 {
        /// Share changes
        const SHARED = MAP_SHARED;
        /// Share changes with other processes
        const SHARED_VALIDATE = MAP_SHARED_VALIDATE;
        /// Changes private; copy pages on write.
        const PRIVATE = MAP_PRIVATE;
        /// Map address must be exactly as requested, no matter whether it is available.
        const FIXED = MAP_FIXED;
        /// Map address must be exactly as requested, no matter whether it is available.
        const FIXED_NOREPLACE = MAP_FIXED_NOREPLACE;
        /// Don't use a file.
        const ANONYMOUS = MAP_ANONYMOUS;
        /// Don't check for reservations.
        const NORESERVE = MAP_NORESERVE;
        /// Allocation is for a stack.
        const STACK = MAP_STACK;
        /// Populate (prefault) memory pages
        const POPULATE = MAP_POPULATE;
        /// Huge page
        const HUGE = MAP_HUGETLB;
        /// Huge page 1G
        const HUGE_1G = MAP_HUGETLB | MAP_HUGE_1GB;
        /// Mask for type of mapping
        const TYPE = MAP_TYPE;
    }

    #[derive(Debug)]
    pub struct MmapProt: u32 {
        /// Page can be read.
        const READ = PROT_READ;
        /// Page can be written.
        const WRITE = PROT_WRITE;
        /// Page can be executed.
        const EXEC = PROT_EXEC;
        /// Extend change to start of growsdown vma (mprotect only).
        const GROWDOWN = PROT_GROWSDOWN;
        /// Extend change to start of growsup vma (mprotect only).
        const GROWSUP = PROT_GROWSUP;
    }
}

impl From<MmapProt> for MappingFlags {
    fn from(value: MmapProt) -> Self {
        let mut flags = MappingFlags::USER;
        if value.contains(MmapProt::READ) {
            flags |= MappingFlags::READ;
        }
        if value.contains(MmapProt::WRITE) {
            flags |= MappingFlags::WRITE;
        }
        if value.contains(MmapProt::EXEC) {
            flags |= MappingFlags::EXECUTE;
        }
        flags
    }
}

#[derive(FromRepr, Debug, Eq, PartialEq)]
#[allow(non_camel_case_types)]
#[repr(i32)]
pub enum Madv {
    MADV_NORMAL = 0,
    MADV_RANDOM = 1,
    MADV_SEQUENTIAL = 2,
    MADV_WILLNEED = 3,
    MADV_DONTNEED = 4,
    MADV_FREE = 8,
    MADV_REMOVE = 9,
    MADV_DONTFORK = 10,
    MADV_DOFORK = 11,
    MADV_MERGEABLE = 12,
    MADV_UNMERGEABLE = 13,
    MADV_HUGEPAGE = 14,
    MADV_NOHUGEPAGE = 15,
    MADV_DONTDUMP = 16,
    MADV_DODUMP = 17,
    MADV_WIPEONFORK = 18,
    MADV_KEEPONFORK = 19,
    MADV_COLD = 20,
    MADV_PAGEOUT = 21,
    MADV_POPULATE_READ = 22,
    MADV_POPULATE_WRITE = 23,
    MADV_HWPOISON = 100,
}
