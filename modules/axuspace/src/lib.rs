//! Utilities for working with user-space pointers.
#![no_std]

use core::{alloc::Layout, ffi::c_char, mem::transmute, ptr, slice, str};

use axerrno::{LinuxError, LinuxResult};
use memory_addr::{MemoryAddr, PAGE_SIZE_4K, VirtAddr, VirtAddrRange};
use page_table_multiarch::MappingFlags;

#[percpu::def_percpu]
static mut ACCESSING_USER_MEM: bool = false;

/// Check if we are currently accessing user memory.
///
/// OS implementation shall allow page faults from kernel when this function
/// returns true.
pub fn is_accessing_user_memory() -> bool {
    ACCESSING_USER_MEM.read_current()
}

fn access_user_memory<R>(f: impl FnOnce() -> R) -> R {
    ACCESSING_USER_MEM.with_current(|v| {
        *v = true;
        let result = f();
        *v = false;
        result
    })
}

/// A trait for accessing user space memory.
///
/// This trait is used to abstract the access to user space memory, so that
/// `axuspace` can be used in different environments.
pub trait UserSpaceAccess {
    /// Check if the given memory region is accessible with the given flags.
    fn check_region_access(
        &self,
        range: VirtAddrRange,
        access_flags: MappingFlags,
    ) -> LinuxResult<()>;

    /// Populate the given memory region, making it accessible.
    fn populate_region(&self, range: VirtAddrRange, access_flags: MappingFlags) -> LinuxResult<()>;
}

pub fn check_region<A: UserSpaceAccess>(
    usa: &A,
    start: VirtAddr,
    layout: Layout,
    access_flags: MappingFlags,
) -> LinuxResult<()> {
    let align = layout.align();
    if start.as_usize() & (align - 1) != 0 {
        return Err(LinuxError::EFAULT);
    }

    let range = VirtAddrRange::from_start_size(start, layout.size());
    usa.check_region_access(range, access_flags)?;
    usa.populate_region(range, access_flags)?;
    Ok(())
}

pub fn check_null_terminated<T: PartialEq + Default, A: UserSpaceAccess>(
    usa: &A,
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
            // SAFETY: This won't overflow the address space since we'll check
            // it below.
            let ptr = unsafe { start_ptr.add(len) };
            while ptr as usize >= page.as_ptr() as usize {
                usa.check_region_access(
                    VirtAddrRange::from_start_size(page, PAGE_SIZE_4K),
                    access_flags,
                )?;
                page += PAGE_SIZE_4K;
            }

            // This might trigger a page fault
            // SAFETY: The pointer is valid and points to a valid memory region.
            if unsafe { ptr.read_volatile() } == zero {
                break;
            }
            len += 1;
        }
        Ok(len)
    })
}

/// A pointer to user space memory.
#[repr(transparent)]
#[derive(PartialEq, Clone, Copy, Debug)]
pub struct UserPtr<T>(*mut T);

impl<T> From<usize> for UserPtr<T> {
    fn from(value: usize) -> Self {
        UserPtr(value as *mut _)
    }
}
impl<T> From<*mut T> for UserPtr<T> {
    fn from(value: *mut T) -> Self {
        UserPtr(value)
    }
}

impl<T> Default for UserPtr<T> {
    fn default() -> Self {
        Self(ptr::null_mut())
    }
}

impl<T> UserPtr<T> {
    const ACCESS_FLAGS: MappingFlags = MappingFlags::READ.union(MappingFlags::WRITE);

    pub fn address(&self) -> VirtAddr {
        VirtAddr::from_ptr_of(self.0)
    }

    pub fn cast<U>(self) -> UserPtr<U> {
        UserPtr(self.0 as *mut U)
    }

    pub fn offset(self, offset: usize) -> UserPtr<T> {
        UserPtr(unsafe { self.0.add(offset) })
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn get_as_mut<A: UserSpaceAccess>(self, usa: &A) -> LinuxResult<&'static mut T> {
        check_region(usa, self.address(), Layout::new::<T>(), Self::ACCESS_FLAGS)?;
        Ok(unsafe { &mut *self.0 })
    }

    pub fn get_as_mut_slice<A: UserSpaceAccess>(
        self,
        usa: &A,
        len: usize,
    ) -> LinuxResult<&'static mut [T]> {
        check_region(
            usa,
            self.address(),
            Layout::array::<T>(len).unwrap(),
            Self::ACCESS_FLAGS,
        )?;
        Ok(unsafe { slice::from_raw_parts_mut(self.0, len) })
    }

    pub fn get_as_mut_null_terminated<A: UserSpaceAccess>(
        self,
        usa: &A,
    ) -> LinuxResult<&'static mut [T]>
    where
        T: PartialEq + Default,
    {
        let len = check_null_terminated::<T, A>(usa, self.address(), Self::ACCESS_FLAGS)?;
        Ok(unsafe { slice::from_raw_parts_mut(self.0, len) })
    }
}

/// An immutable pointer to user space memory.
#[repr(transparent)]
#[derive(PartialEq, Clone, Copy)]
pub struct UserConstPtr<T>(*const T);

impl<T> From<usize> for UserConstPtr<T> {
    fn from(value: usize) -> Self {
        UserConstPtr(value as *const _)
    }
}
impl<T> From<*const T> for UserConstPtr<T> {
    fn from(value: *const T) -> Self {
        UserConstPtr(value)
    }
}

impl<T> Default for UserConstPtr<T> {
    fn default() -> Self {
        Self(ptr::null())
    }
}

impl<T> UserConstPtr<T> {
    const ACCESS_FLAGS: MappingFlags = MappingFlags::READ;

    pub fn address(&self) -> VirtAddr {
        VirtAddr::from_ptr_of(self.0)
    }

    pub fn cast<U>(self) -> UserConstPtr<U> {
        UserConstPtr(self.0 as *const U)
    }

    pub fn offset(self, offset: usize) -> UserConstPtr<T> {
        UserConstPtr(unsafe { self.0.add(offset) })
    }

    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    pub fn get_as_ref<A: UserSpaceAccess>(self, usa: &A) -> LinuxResult<&'static T> {
        check_region(usa, self.address(), Layout::new::<T>(), Self::ACCESS_FLAGS)?;
        Ok(unsafe { &*self.0 })
    }

    pub fn get_as_slice<A: UserSpaceAccess>(
        self,
        usa: &A,
        len: usize,
    ) -> LinuxResult<&'static [T]> {
        check_region(
            usa,
            self.address(),
            Layout::array::<T>(len).unwrap(),
            Self::ACCESS_FLAGS,
        )?;
        Ok(unsafe { slice::from_raw_parts(self.0, len) })
    }

    pub fn get_as_null_terminated<A: UserSpaceAccess>(self, usa: &A) -> LinuxResult<&'static [T]>
    where
        T: PartialEq + Default,
    {
        let len = check_null_terminated::<T, A>(usa, self.address(), Self::ACCESS_FLAGS)?;
        Ok(unsafe { slice::from_raw_parts(self.0, len) })
    }
}

impl UserConstPtr<c_char> {
    /// Get the pointer as `&str`, validating the memory region.
    pub fn get_as_str<A: UserSpaceAccess>(self, usa: &A) -> LinuxResult<&'static str> {
        let slice = self.get_as_null_terminated(usa)?;
        // SAFETY: c_char is u8
        let slice = unsafe { transmute::<&[c_char], &[u8]>(slice) };

        str::from_utf8(slice).map_err(|_| LinuxError::EILSEQ)
    }
}

#[macro_export]
macro_rules! nullable {
    ($ptr:ident.$func:ident($usa:expr, $($arg:expr),*)) => {
        if $ptr.is_null() {
            Ok(None)
        } else {
            Some($ptr.$func($usa, $($arg),*)).transpose()
        }
    };
}
