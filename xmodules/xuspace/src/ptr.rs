use core::{marker::PhantomData, ptr};

use memory_addr::VirtAddr;

/// Common address operations for typed user pointers.
pub trait UserReadable<T> {
    fn address(&self) -> VirtAddr;
    fn offset(&self, offset: usize) -> Self;
}

macro_rules! impl_user_pointer {
    ($ptr_type:ident) => {
        impl<T> $ptr_type<T> {
            pub fn address(&self) -> VirtAddr {
                VirtAddr::from(self.0)
            }

            pub fn is_null(&self) -> bool {
                self.0 == 0
            }

            pub fn cast<U>(self) -> $ptr_type<U> {
                $ptr_type(self.0, PhantomData)
            }

            pub fn offset(self, offset: usize) -> Self {
                $ptr_type(
                    self.0
                        .wrapping_add(offset.wrapping_mul(core::mem::size_of::<T>())),
                    PhantomData,
                )
            }
        }

        impl<T> UserReadable<T> for $ptr_type<T> {
            fn address(&self) -> VirtAddr {
                $ptr_type::<T>::address(self)
            }

            fn offset(&self, offset: usize) -> Self {
                (*self).offset(offset)
            }
        }
    };
}

/// Mutable user-space address token.
#[repr(transparent)]
#[derive(PartialEq, Debug)]
pub struct UserPtr<T>(usize, PhantomData<*mut T>);

impl<T> Copy for UserPtr<T> {}

impl<T> Clone for UserPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> From<usize> for UserPtr<T> {
    fn from(value: usize) -> Self {
        Self(value, PhantomData)
    }
}

impl<T> From<*mut T> for UserPtr<T> {
    fn from(value: *mut T) -> Self {
        Self(value as usize, PhantomData)
    }
}

impl<T> From<Option<*mut T>> for UserPtr<T> {
    fn from(value: Option<*mut T>) -> Self {
        Self(value.unwrap_or(ptr::null_mut()) as usize, PhantomData)
    }
}

impl<T> Default for UserPtr<T> {
    fn default() -> Self {
        Self(0, PhantomData)
    }
}

impl_user_pointer!(UserPtr);

/// Immutable user-space address token.
#[repr(transparent)]
#[derive(Debug, PartialEq)]
pub struct UserConstPtr<T>(usize, PhantomData<*const T>);

impl<T> Copy for UserConstPtr<T> {}

impl<T> Clone for UserConstPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> From<usize> for UserConstPtr<T> {
    fn from(value: usize) -> Self {
        Self(value, PhantomData)
    }
}

impl<T> From<*const T> for UserConstPtr<T> {
    fn from(value: *const T) -> Self {
        Self(value as usize, PhantomData)
    }
}

impl<T> Default for UserConstPtr<T> {
    fn default() -> Self {
        Self(0, PhantomData)
    }
}

impl_user_pointer!(UserConstPtr);
