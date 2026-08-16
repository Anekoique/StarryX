//! Wrapper for [`sockaddr`]. Using trait to convert between [`SocketAddr`] and [`sockaddr`] types.
use core::{
    mem::{offset_of, size_of},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};

use xerrno::{LinuxError, LinuxResult};

use xuspace::{UserConstPtr, UserPtr};
use xutils::ctypes::{
    __kernel_sa_family_t, AF_INET, AF_INET6, sockaddr, sockaddr_in, sockaddr_in6, socklen_t,
};

use crate::task::with_uspace;

/// Trait to extend [`SocketAddr`] and its variants with methods for reading from and writing to user space.
///
pub trait SocketAddrExt: Sized {
    /// This method attempts to interpret the data pointed to by `addr` with the
    /// given `addrlen` as a valid socket address of the implementing type.
    fn read_from_user(addr: UserConstPtr<sockaddr>, addrlen: socklen_t) -> LinuxResult<Self>;

    /// This method serializes the current socket address instance into the
    /// [`sockaddr`] structure pointed to by `addr` in user space.
    fn write_to_user(&self, addr: UserPtr<sockaddr>) -> LinuxResult<socklen_t>;

    /// Gets the address family of the socket address.
    fn family(&self) -> u16;

    /// Gets the encoded length of the socket address.
    fn addr_len(&self) -> socklen_t;
}

fn read_bytes<const N: usize>(addr: UserConstPtr<sockaddr>) -> LinuxResult<[u8; N]> {
    let mut bytes = [0; N];
    with_uspace(|uspace| uspace.read_slice_to(addr.cast::<u8>(), &mut bytes))?;
    Ok(bytes)
}

impl SocketAddrExt for SocketAddr {
    /// Reads a [`SocketAddr`] from user space.
    ///
    /// This implementation first performs basic length validation. Then, it copies
    /// the raw [`sockaddr`] data from user space into a temporary kernel buffer.
    /// Based on the address family ([`AF_INET`] or [`AF_INET6`]) extracted from the
    /// copied data, it delegates the actual parsing to [`SocketAddrV4::read_from_user`]
    /// or [`SocketAddrV6::read_from_user`].
    fn read_from_user(addr: UserConstPtr<sockaddr>, addrlen: socklen_t) -> LinuxResult<Self> {
        if size_of::<__kernel_sa_family_t>() > addrlen as usize
            || addrlen as usize > size_of::<sockaddr>()
        {
            return Err(LinuxError::EINVAL);
        }
        let family = u16::from_ne_bytes(read_bytes::<2>(addr)?.into()) as u32;
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
        let bytes = read_bytes::<{ size_of::<sockaddr_in>() }>(addr)?;
        let family_offset = offset_of!(sockaddr_in, sin_family);
        let port_offset = offset_of!(sockaddr_in, sin_port);
        let address_offset = offset_of!(sockaddr_in, sin_addr);
        if u16::from_ne_bytes(bytes[family_offset..family_offset + 2].try_into().unwrap()) as u32
            != AF_INET
        {
            return Err(LinuxError::EAFNOSUPPORT);
        }

        Ok(SocketAddrV4::new(
            Ipv4Addr::from(
                <[u8; 4]>::try_from(&bytes[address_offset..address_offset + 4]).unwrap(),
            ),
            u16::from_be_bytes(bytes[port_offset..port_offset + 2].try_into().unwrap()),
        ))
    }

    /// Writes the `SocketAddrV4` to user space.
    fn write_to_user(&self, addr: UserPtr<sockaddr>) -> LinuxResult<socklen_t> {
        if addr.is_null() {
            return Err(LinuxError::EINVAL);
        }
        let len = size_of::<sockaddr_in>() as socklen_t;
        let mut bytes = [0; size_of::<sockaddr_in>()];
        let family_offset = offset_of!(sockaddr_in, sin_family);
        let port_offset = offset_of!(sockaddr_in, sin_port);
        let address_offset = offset_of!(sockaddr_in, sin_addr);
        bytes[family_offset..family_offset + 2].copy_from_slice(&(AF_INET as u16).to_ne_bytes());
        bytes[port_offset..port_offset + 2].copy_from_slice(&self.port().to_be_bytes());
        bytes[address_offset..address_offset + 4].copy_from_slice(&self.ip().octets());
        with_uspace(|uspace| uspace.write_slice(addr.cast::<u8>(), &bytes))?;

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
        let bytes = read_bytes::<{ size_of::<sockaddr_in6>() }>(addr)?;
        let family_offset = offset_of!(sockaddr_in6, sin6_family);
        let port_offset = offset_of!(sockaddr_in6, sin6_port);
        let flow_offset = offset_of!(sockaddr_in6, sin6_flowinfo);
        let address_offset = offset_of!(sockaddr_in6, sin6_addr);
        let scope_offset = offset_of!(sockaddr_in6, sin6_scope_id);
        if u16::from_ne_bytes(bytes[family_offset..family_offset + 2].try_into().unwrap()) as u32
            != AF_INET6
        {
            return Err(LinuxError::EAFNOSUPPORT);
        }

        Ok(SocketAddrV6::new(
            Ipv6Addr::from(
                <[u8; 16]>::try_from(&bytes[address_offset..address_offset + 16]).unwrap(),
            ),
            u16::from_be_bytes(bytes[port_offset..port_offset + 2].try_into().unwrap()),
            u32::from_be_bytes(bytes[flow_offset..flow_offset + 4].try_into().unwrap()),
            u32::from_ne_bytes(bytes[scope_offset..scope_offset + 4].try_into().unwrap()),
        ))
    }
    /// Writes the `SocketAddrV6` to user space.
    fn write_to_user(&self, addr: UserPtr<sockaddr>) -> LinuxResult<socklen_t> {
        if addr.is_null() {
            return Err(LinuxError::EINVAL);
        }
        let len = size_of::<sockaddr_in6>() as socklen_t;
        let mut bytes = [0; size_of::<sockaddr_in6>()];
        let family_offset = offset_of!(sockaddr_in6, sin6_family);
        let port_offset = offset_of!(sockaddr_in6, sin6_port);
        let flow_offset = offset_of!(sockaddr_in6, sin6_flowinfo);
        let address_offset = offset_of!(sockaddr_in6, sin6_addr);
        let scope_offset = offset_of!(sockaddr_in6, sin6_scope_id);
        bytes[family_offset..family_offset + 2].copy_from_slice(&(AF_INET6 as u16).to_ne_bytes());
        bytes[port_offset..port_offset + 2].copy_from_slice(&self.port().to_be_bytes());
        bytes[flow_offset..flow_offset + 4].copy_from_slice(&self.flowinfo().to_be_bytes());
        bytes[address_offset..address_offset + 16].copy_from_slice(&self.ip().octets());
        bytes[scope_offset..scope_offset + 4].copy_from_slice(&self.scope_id().to_ne_bytes());

        with_uspace(|uspace| uspace.write_slice(addr.cast::<u8>(), &bytes))?;

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
