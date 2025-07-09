use core::{alloc::Layout, ffi::c_char, mem::transmute, ptr, slice, str};

use axerrno::{LinuxError, LinuxResult};
use memory_addr::VirtAddr;
use page_table_multiarch::MappingFlags;

use crate::{UserSpaceAccess, check_null_terminated, check_region};

macro_rules! impl_user_pointer {
    ($ptr_type:ident, $raw_ptr:ty) => {
        impl<T> $ptr_type<T> {
            pub fn address(&self) -> VirtAddr {
                VirtAddr::from_ptr_of(self.0)
            }

            pub fn is_null(&self) -> bool {
                self.0.is_null()
            }

            pub fn cast<U>(self) -> $ptr_type<U> {
                $ptr_type(self.0 as *const U as $raw_ptr)
            }

            pub fn offset(self, offset: usize) -> Self {
                $ptr_type(unsafe { self.0.add(offset) })
            }
        }

        impl<T> UserReadable<T> for $ptr_type<T> {
            fn get_as_ref<A: UserSpaceAccess>(self, uspace: &A) -> LinuxResult<&'static T> {
                check_region(
                    uspace,
                    self.address(),
                    Layout::new::<T>(),
                    MappingFlags::READ,
                )?;
                Ok(unsafe { &*self.0 })
            }

            fn get_as_slice<A: UserSpaceAccess>(
                self,
                uspace: &A,
                len: usize,
            ) -> LinuxResult<&'static [T]> {
                check_region(
                    uspace,
                    self.address(),
                    Layout::array::<T>(len).map_err(|_| LinuxError::EINVAL)?,
                    MappingFlags::READ,
                )?;
                Ok(unsafe { slice::from_raw_parts(self.0, len) })
            }

            fn get_as_null_terminated<A: UserSpaceAccess>(
                self,
                uspace: &A,
            ) -> LinuxResult<&'static [T]>
            where
                T: PartialEq + Default,
            {
                let len =
                    check_null_terminated::<T, A>(uspace, self.address(), MappingFlags::READ)?;
                Ok(unsafe { slice::from_raw_parts(self.0, len) })
            }
        }

        impl $ptr_type<c_char> {
            pub fn get_as_str<A: UserSpaceAccess>(self, uspace: &A) -> LinuxResult<&'static str> {
                let slice = self.get_as_null_terminated(uspace)?;
                let slice = unsafe { transmute::<&[c_char], &[u8]>(slice) };
                str::from_utf8(slice).map_err(|_| LinuxError::EILSEQ)
            }
        }
    };
}

pub trait UserReadable<T> {
    fn get_as_ref<A: UserSpaceAccess>(self, uspace: &A) -> LinuxResult<&'static T>;
    fn get_as_slice<A: UserSpaceAccess>(self, uspace: &A, len: usize) -> LinuxResult<&'static [T]>;
    fn get_as_null_terminated<A: UserSpaceAccess>(self, uspace: &A) -> LinuxResult<&'static [T]>
    where
        T: PartialEq + Default;
}

#[repr(transparent)]
#[derive(PartialEq, Debug, Clone, Copy)]
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

impl_user_pointer!(UserPtr, *mut U);

impl<T> UserPtr<T> {
    pub fn get_as_mut<A: UserSpaceAccess>(self, uspace: &A) -> LinuxResult<&'static mut T> {
        check_region(
            uspace,
            self.address(),
            Layout::new::<T>(),
            MappingFlags::READ.union(MappingFlags::WRITE),
        )?;
        Ok(unsafe { &mut *self.0 })
    }

    pub fn get_as_mut_slice<A: UserSpaceAccess>(
        self,
        uspace: &A,
        len: usize,
    ) -> LinuxResult<&'static mut [T]> {
        check_region(
            uspace,
            self.address(),
            Layout::array::<T>(len).map_err(|_| LinuxError::EINVAL)?,
            MappingFlags::READ.union(MappingFlags::WRITE),
        )?;
        Ok(unsafe { slice::from_raw_parts_mut(self.0, len) })
    }

    pub fn get_as_mut_null_terminated<A: UserSpaceAccess>(
        self,
        uspace: &A,
    ) -> LinuxResult<&'static mut [T]>
    where
        T: PartialEq + Default,
    {
        let len = check_null_terminated::<T, A>(
            uspace,
            self.address(),
            MappingFlags::READ.union(MappingFlags::WRITE),
        )?;
        Ok(unsafe { slice::from_raw_parts_mut(self.0, len) })
    }
}

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

impl_user_pointer!(UserConstPtr, *const U);
