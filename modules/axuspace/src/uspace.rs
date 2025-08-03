use core::{
    alloc::Layout,
    ffi::c_char,
    sync::atomic::{AtomicBool, Ordering},
};

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use axerrno::{LinuxError, LinuxResult};
use memory_addr::{MemoryAddr, PAGE_SIZE_4K, VirtAddr, VirtAddrRange};
use page_table_multiarch::MappingFlags;

use crate::{UserConstPtr, UserPtr, UserReadable};

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

    /// Read a value from user space
    fn read<P, T>(&self, ptr: P) -> LinuxResult<T>
    where
        P: UserReadable<T>,
        T: Copy + 'static,
    {
        ptr.get_as_ref(self).copied()
    }

    /// Read a null-terminated string from user space
    fn read_str(&self, ptr: UserConstPtr<c_char>) -> LinuxResult<&'static str> {
        ptr.get_as_str(self)
    }

    /// Read a slice from user space
    fn read_slice<P, T>(&self, ptr: P, len: usize) -> LinuxResult<&'static [T]>
    where
        P: UserReadable<T>,
    {
        ptr.get_as_slice(self, len)
    }

    /// Read from user space into a kernel buffer using direct memory copy
    fn read_slice_to<P, T>(&self, ptr: P, buf: &mut [T]) -> LinuxResult<()>
    where
        P: UserReadable<T>,
        T: 'static,
    {
        let user_slice = ptr.get_as_slice(self, buf.len())?;
        unsafe {
            core::ptr::copy_nonoverlapping(user_slice.as_ptr(), buf.as_mut_ptr(), buf.len());
        }
        Ok(())
    }

    /// Get a mutable reference to user space data
    fn raw_ptr<T>(&self, ptr: UserPtr<T>) -> LinuxResult<&'static mut T> {
        ptr.get_as_mut(self)
    }

    /// Get a mutable slice to user space data
    fn raw_slice<T>(&self, ptr: UserPtr<T>, len: usize) -> LinuxResult<&'static mut [T]> {
        ptr.get_as_mut_slice(self, len)
    }

    /// Write a value to user space
    fn write<T>(&self, ptr: UserPtr<T>, val: T) -> LinuxResult<()>
    where
        T: 'static,
    {
        ptr.get_as_mut(self).map(|v| *v = val)
    }

    /// Write a slice to user space using direct memory copy
    fn write_slice<T>(&self, ptr: UserPtr<T>, slice: &[T]) -> LinuxResult<()>
    where
        T: 'static,
    {
        let user_slice = ptr.get_as_mut_slice(self, slice.len())?;
        unsafe {
            core::ptr::copy_nonoverlapping(slice.as_ptr(), user_slice.as_mut_ptr(), slice.len());
        }
        Ok(())
    }

    /// Read multiple strings from a null-terminated array of string pointers
    fn read_str_array(&self, ptr: UserConstPtr<UserConstPtr<c_char>>) -> LinuxResult<Vec<String>> {
        let mut strings = Vec::new();
        let mut offset = 0;
        if ptr.is_null() {
            return Ok(strings);
        }

        loop {
            let str_ptr = self.read(ptr.offset(offset))?;
            if str_ptr.is_null() {
                break;
            }
            strings.push(self.read_str(str_ptr)?.to_string());
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

/// Find the length of a null-terminated array in user space
pub fn check_null_terminated<T: PartialEq + Default, A: UserSpaceAccess>(
    uspace: &A,
    start: VirtAddr,
    access_flags: MappingFlags,
) -> LinuxResult<usize> {
    let align = Layout::new::<T>().align();
    if start.as_usize() & (align - 1) != 0 {
        return Err(LinuxError::EFAULT);
    }

    let zero = T::default();

    let start_ptr = start.as_ptr_of::<T>();

    access_user_memory(|| {
        let mut len = 0;
        let mut page = start.align_down_4k();
        loop {
            let ptr = unsafe { start_ptr.add(len) };
            while ptr as usize >= page.as_ptr() as usize {
                uspace.check_region_access(
                    VirtAddrRange::from_start_size(page, PAGE_SIZE_4K),
                    access_flags,
                )?;
                page += PAGE_SIZE_4K;
            }

            if unsafe { ptr.read_volatile() } == zero {
                break;
            }
            len += 1;
        }
        Ok(len)
    })
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
