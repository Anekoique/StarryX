use core::{
    alloc::Layout,
    ffi::c_char,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{UserConstPtr, UserPtr};
use alloc::{string::String, vec::Vec};
use memory_addr::{VirtAddr, VirtAddrRange};
use page_table_multiarch::MappingFlags;
use xerrno::{LinuxError, LinuxResult};

#[percpu::def_percpu]
static ACCESSING_USER_MEM: AtomicBool = AtomicBool::new(false);

/// Check if the current thread is accessing user memory
pub fn is_accessing_user_memory() -> bool {
    ACCESSING_USER_MEM.with_current(|v| v.load(Ordering::SeqCst))
}

/// Enable safe access to user memory within the closure
pub fn access_user_memory<R>(f: impl FnOnce() -> R) -> R {
    ACCESSING_USER_MEM.with_current(|v| {
        v.store(true, Ordering::SeqCst);
        let result = f();
        v.store(false, Ordering::SeqCst);
        result
    })
}

/// Trait for validating and populating user space memory access
pub trait UserSpaceAccess: Sized {
    /// Check if a memory region is accessible with given flags
    fn check_region_access(
        &self,
        range: VirtAddrRange,
        access_flags: MappingFlags,
    ) -> LinuxResult<()>;

    /// Populate a memory region making it accessible
    fn populate_region(&self, range: VirtAddrRange, access_flags: MappingFlags) -> LinuxResult<()>;

    /// Copies user bytes into a kernel-owned buffer.
    fn copy_from_user(&self, address: VirtAddr, output: &mut [u8]) -> LinuxResult<()>;

    /// Copies kernel-owned bytes into user memory.
    fn copy_to_user(&self, address: VirtAddr, input: &[u8]) -> LinuxResult<()>;

    /// Validates and populates a typed user output span without reading it.
    fn check_write<T>(&self, ptr: UserPtr<T>) -> LinuxResult<()> {
        check_region(self, ptr.address(), Layout::new::<T>(), MappingFlags::WRITE)
    }

    /// Read a null-terminated string from user space
    fn read_str(&self, ptr: UserConstPtr<c_char>) -> LinuxResult<String> {
        const MAX_STRING_BYTES: usize = 4096;
        let mut bytes = Vec::new();
        for index in 0..MAX_STRING_BYTES {
            let mut byte = [0];
            self.copy_from_user(ptr.cast::<u8>().offset(index).address(), &mut byte)?;
            let byte = byte[0];
            if byte == 0 {
                return String::from_utf8(bytes).map_err(|_| LinuxError::EILSEQ);
            }
            bytes.push(byte);
        }
        Err(LinuxError::ENAMETOOLONG)
    }

    /// Read multiple strings from a null-terminated array of string pointers
    fn read_str_array(&self, ptr: UserConstPtr<UserConstPtr<c_char>>) -> LinuxResult<Vec<String>> {
        let mut strings = Vec::new();
        let mut offset = 0;
        if ptr.is_null() {
            return Ok(strings);
        }

        loop {
            let mut raw = [0; core::mem::size_of::<usize>()];
            self.copy_from_user(ptr.offset(offset).address(), &mut raw)?;
            let str_ptr = UserConstPtr::from(usize::from_ne_bytes(raw));
            if str_ptr.is_null() {
                break;
            }
            strings.push(self.read_str(str_ptr)?);
            offset += 1;
        }

        Ok(strings)
    }
}

/// Validate memory region alignment and accessibility
pub fn check_region<A: UserSpaceAccess>(
    uspace: &A,
    start: VirtAddr,
    layout: Layout,
    access_flags: MappingFlags,
) -> LinuxResult<()> {
    let align = layout.align();
    if start.as_usize() & (align - 1) != 0 {
        return Err(LinuxError::EFAULT);
    }

    let range =
        VirtAddrRange::try_from_start_size(start, layout.size()).ok_or(LinuxError::EFAULT)?;
    uspace.check_region_access(range, access_flags)?;
    uspace.populate_region(range, access_flags)?;
    Ok(())
}

#[macro_export]
macro_rules! nullable {
    (@impl ($($base:tt)*) . $method:ident ( $ptr:expr $(, $args:expr)* )) => {
        {
            if $ptr.is_null() { Ok(None) }
            else { ($($base)*) . $method ($ptr $(, $args)*).map(Some) }
        }
    };

    (@impl ($($base:tt)*) . $next:ident ( $($args:tt)* ) $($rest:tt)*) => {
        nullable!(@impl ($($base)* . $next ( $($args)* )) $($rest)*)
    };

    (@impl ($($base:tt)*) . $field:ident $($rest:tt)*) => {
        nullable!(@impl ($($base)* . $field) $($rest)*)
    };

    (@impl () $first:ident $($rest:tt)*) => {
        nullable!(@impl ($first) $($rest)*)
    };

    ($($chain:tt)*) => {
        nullable!(@impl () $($chain)*)
    };
}
