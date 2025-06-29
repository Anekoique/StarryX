//! Socket address conversion utilities for StarryX.
//!
//! This module provides the [`SocketAddrExt`] trait and associated implementations
//! for safely converting between Rust's [`SocketAddr`] types and the raw C
//! [`sockaddr`] structures used in system calls.
//!
//! The trait handles proper memory management, endianness conversion, and validation
//! when reading from and writing to user space memory. It supports both IPv4
//! ([`SocketAddrV4`]) and IPv6 ([`SocketAddrV6`]) address families.
//!
//! # Safety
//! All user space memory access is performed through safe wrapper types that
//! validate pointer access and prevent buffer overruns.

use crate::ptr::{UserConstPtr, UserPtr};
use axerrno::{LinuxError, LinuxResult};
use core::{
    mem::{MaybeUninit, size_of},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};
use linux_raw_sys::net::{
    __kernel_sa_family_t, AF_INET, AF_INET6, in_addr, in6_addr, sockaddr, sockaddr_in,
    sockaddr_in6, socklen_t,
};

/// Extension trait for socket address types that enables safe user space conversion.
///
/// This trait provides methods to convert between Rust's [`SocketAddr`] types and
/// the raw C [`sockaddr`] structures used in system calls. It handles the complexity
/// of different address families (IPv4/IPv6), endianness conversion, and safe
/// memory access to user space.
///
/// # Safety
/// All implementations ensure that user space memory is accessed safely with
/// proper bounds checking and validation. The trait methods will return appropriate
/// [`LinuxError`] codes when encountering invalid addresses or insufficient buffer space.
///
pub trait SocketAddrExt: Sized {
    /// Reads a socket address from user space memory.
    ///
    /// # Arguments
    /// * `addr` - Pointer to a [`sockaddr`] structure in user space
    /// * `addrlen` - Length of the address structure in bytes
    ///
    /// # Returns
    /// * `Ok(Self)` - Successfully parsed socket address
    /// * `Err(LinuxError)` - Invalid address format, unsupported family, or access error
    ///
    /// # Errors  
    /// * `EINVAL` - Invalid address length or format
    /// * `EAFNOSUPPORT` - Unsupported address family
    /// * `EFAULT` - Invalid user space pointer
    fn read_from_user(addr: UserConstPtr<sockaddr>, addrlen: socklen_t) -> LinuxResult<Self>;

    /// Writes the socket address to user space memory.
    ///
    /// # Arguments
    /// * `addr` - Pointer to user space buffer to write the [`sockaddr`] structure
    ///
    /// # Returns
    /// * `Ok(socklen_t)` - Number of bytes written to user space
    /// * `Err(LinuxError)` - Write error or invalid pointer
    ///
    /// # Errors
    /// * `EINVAL` - Null pointer provided
    /// * `EFAULT` - Invalid user space pointer or insufficient buffer space
    fn write_to_user(&self, addr: UserPtr<sockaddr>) -> LinuxResult<socklen_t>;

    /// Returns the address family identifier (AF_INET, AF_INET6, etc.).
    fn family(&self) -> u16;

    /// Returns the size in bytes of the encoded socket address structure.
    fn addr_len(&self) -> socklen_t;
}

/// Safely copies a socket address structure from user space to kernel memory.
///
/// This utility function performs a validated copy of socket address data from
/// user space into a temporary kernel buffer. It reads exactly `addrlen` bytes
/// from the user-space pointer and stores them in uninitialized kernel memory.
///
/// # Arguments
/// * `addr` - User space pointer to the source [`sockaddr`] structure
/// * `addrlen` - Number of bytes to copy from user space
///
/// # Returns  
/// * `Ok(MaybeUninit<sockaddr>)` - Uninitialized kernel storage containing copied data
/// * `Err(LinuxError)` - Memory access error
///
/// # Safety
/// The returned `MaybeUninit<sockaddr>` contains raw bytes copied from user space
/// and must be validated before use. The caller is responsible for ensuring the
/// data represents a valid socket address structure.
///
#[inline]
fn copy_sockaddr_from_user(
    addr: UserConstPtr<sockaddr>,
    addrlen: socklen_t,
) -> LinuxResult<MaybeUninit<sockaddr>> {
    let mut storage = MaybeUninit::<sockaddr>::uninit();
    
    let sock_bytes = addr.cast::<u8>().get_as_slice(addrlen as usize)?;
    unsafe {
        core::ptr::copy_nonoverlapping(
            sock_bytes.as_ptr(),
            storage.as_mut_ptr() as *mut u8,
            addrlen as usize,
        )
    };
    Ok(storage)
}

impl SocketAddrExt for SocketAddr {
    /// Reads a [`SocketAddr`] from user space.
    ///
    /// This implementation first performs basic length validation. Then, it reads
    /// the address family from the first 2 bytes to determine the socket address type.
    fn read_from_user(addr: UserConstPtr<sockaddr>, addrlen: socklen_t) -> LinuxResult<Self> {
        if (size_of::<__kernel_sa_family_t>() as socklen_t) > addrlen 
            || addrlen > (size_of::<sockaddr>() as socklen_t)
        {
            return Err(LinuxError::EINVAL);
        }
        
        // Read the address family field (sa_family) from the beginning of the structure
        let family_bytes = addr.cast::<u8>().get_as_slice(size_of::<__kernel_sa_family_t>())?;
        let family = u16::from_ne_bytes([family_bytes[0], family_bytes[1]]) as u32;
        
        match family {
            AF_INET => SocketAddrV4::read_from_user(addr, addrlen).map(SocketAddr::V4),
            AF_INET6 => SocketAddrV6::read_from_user(addr, addrlen).map(SocketAddr::V6),
            _ => Err(LinuxError::EAFNOSUPPORT),
        }
    }

    /// Writes the [`SocketAddr`] to user space.
    ///
    /// This implementation checks for a null user-space pointer. Then, it delegates
    /// the actual writing to the specific [`SocketAddrV4`] or [`SocketAddrV6`]
    /// `write_to_user` implementation based on the variant of `self`.
    fn write_to_user(&self, addr: UserPtr<sockaddr>) -> LinuxResult<socklen_t> {
        if addr.is_null() {
            return Err(LinuxError::EINVAL);
        }

        match self {
            SocketAddr::V4(v4) => v4.write_to_user(addr),
            SocketAddr::V6(v6) => v6.write_to_user(addr),
        }
    }

    /// Gets the address family of the [`SocketAddr`].
    ///
    /// Returns `AF_INET` for IPv4 addresses or `AF_INET6` for IPv6 addresses.
    fn family(&self) -> u16 {
        match self {
            SocketAddr::V4(v4) => v4.family(),
            SocketAddr::V6(v6) => v6.family(),
        }
    }

    /// Gets the encoded length of the [`SocketAddr`] instance.
    ///
    /// Returns the size in bytes that this [`SocketAddr`] would occupy when
    /// encoded as a [`sockaddr_in`] (for IPv4) or [`sockaddr_in6`] (for IPv6) structure.
    fn addr_len(&self) -> socklen_t {
        match self {
            SocketAddr::V4(v4) => v4.addr_len(),
            SocketAddr::V6(v6) => v6.addr_len(),
        }
    }
}

impl SocketAddrExt for SocketAddrV4 {
    /// Reads an [`SocketAddrV4`] from user space.
    fn read_from_user(addr: UserConstPtr<sockaddr>, addrlen: socklen_t) -> LinuxResult<Self> {
        if addrlen < size_of::<sockaddr_in>() as socklen_t {
            return Err(LinuxError::EINVAL);
        }
        let storage = copy_sockaddr_from_user(addr, addrlen)?;
        let addr_in = unsafe { &*(storage.as_ptr() as *const sockaddr_in) };
        if addr_in.sin_family as u32 != AF_INET {
            return Err(LinuxError::EAFNOSUPPORT);
        }

        Ok(SocketAddrV4::new(
            Ipv4Addr::from_bits(u32::from_be(addr_in.sin_addr.s_addr)),
            u16::from_be(addr_in.sin_port),
        ))
    }

    /// Writes the `SocketAddrV4` to user space.
    fn write_to_user(&self, addr: UserPtr<sockaddr>) -> LinuxResult<socklen_t> {
        if addr.is_null() {
            return Err(LinuxError::EINVAL);
        }
        
        let len = size_of::<sockaddr_in>() as socklen_t;
        let sockin_addr = sockaddr_in {
            sin_family: AF_INET as _,
            sin_port: self.port().to_be(),
            sin_addr: in_addr {
                s_addr: u32::from_ne_bytes(self.ip().octets()),
            },
            __pad: [0_u8; 8],
        };
        
        // Write directly to user space buffer, only validating the size we need
        let dst_bytes = addr.cast::<u8>().get_as_mut_slice(len as usize)?;
        unsafe {
            core::ptr::copy_nonoverlapping(
                &sockin_addr as *const sockaddr_in as *const u8,
                dst_bytes.as_mut_ptr(),
                len as usize,
            )
        };

        Ok(len)
    }

    /// Gets the address family for [`SocketAddrV4`].
    fn family(&self) -> u16 {
        AF_INET as u16
    }

    /// Gets the encoded length of [`SocketAddrV4`].
    fn addr_len(&self) -> socklen_t {
        size_of::<sockaddr_in>() as socklen_t
    }
}

impl SocketAddrExt for SocketAddrV6 {
    /// Reads an [`SocketAddrV6`] from user space.
    fn read_from_user(addr: UserConstPtr<sockaddr>, addrlen: socklen_t) -> LinuxResult<Self> {
        if addrlen < size_of::<sockaddr_in6>() as socklen_t {
            return Err(LinuxError::EINVAL);
        }
        let storage = copy_sockaddr_from_user(addr, addrlen)?;
        let addr_in6 = unsafe { &*(storage.as_ptr() as *const sockaddr_in6) };
        if addr_in6.sin6_family as u32 != AF_INET6 {
            return Err(LinuxError::EAFNOSUPPORT);
        }

        Ok(SocketAddrV6::new(
            Ipv6Addr::from(unsafe { addr_in6.sin6_addr.in6_u.u6_addr8 }),
            u16::from_be(addr_in6.sin6_port),
            u32::from_be(addr_in6.sin6_flowinfo),
            addr_in6.sin6_scope_id,
        ))
    }
    /// Writes the `SocketAddrV6` to user space.
    fn write_to_user(&self, addr: UserPtr<sockaddr>) -> LinuxResult<socklen_t> {
        if addr.is_null() {
            return Err(LinuxError::EINVAL);
        }
        
        let len = size_of::<sockaddr_in6>() as socklen_t;
        let sockin_addr = sockaddr_in6 {
            sin6_family: AF_INET6 as _,
            sin6_port: self.port().to_be(),
            sin6_flowinfo: self.flowinfo().to_be(),
            sin6_addr: in6_addr {
                in6_u: linux_raw_sys::net::in6_addr__bindgen_ty_1 {
                    u6_addr8: self.ip().octets(),
                },
            },
            sin6_scope_id: self.scope_id(),
        };

        // Write directly to user space buffer, only validating the size we need
        let dst_bytes = addr.cast::<u8>().get_as_mut_slice(len as usize)?;
        unsafe {
            core::ptr::copy_nonoverlapping(
                &sockin_addr as *const sockaddr_in6 as *const u8,
                dst_bytes.as_mut_ptr(),
                len as usize,
            )
        };

        Ok(len)
    }

    /// Gets the address family for [`SocketAddrV6`].
    fn family(&self) -> u16 {
        AF_INET6 as u16
    }

    /// Gets the encoded length of [`SocketAddrV6`].
    fn addr_len(&self) -> socklen_t {
        size_of::<sockaddr_in6>() as socklen_t
    }
}