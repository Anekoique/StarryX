use core::net::{IpAddr, SocketAddr};
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address, Ipv6Address};

use alloc::vec::Vec;

use super::unix_socket::UnixAddr;

extern crate alloc;

pub const fn from_core_ipaddr(ip: IpAddr) -> IpAddress {
    match ip {
        IpAddr::V4(ipv4) => IpAddress::Ipv4(Ipv4Address(ipv4.octets())),
        IpAddr::V6(ipv6) => IpAddress::Ipv6(Ipv6Address(ipv6.octets())),
    }
}

pub const fn into_core_ipaddr(ip: IpAddress) -> IpAddr {
    match ip {
        IpAddress::Ipv4(ipv4) => IpAddr::V4(unsafe { core::mem::transmute(ipv4.0) }),
        IpAddress::Ipv6(ipv6) => IpAddr::V6(unsafe { core::mem::transmute(ipv6.0) }),
    }
}

/// Convert from `std::net::SocketAddr` to `smoltcp::wire::IpEndpoint`.
pub const fn from_core_sockaddr(addr: SocketAddr) -> IpEndpoint {
    IpEndpoint {
        addr: from_core_ipaddr(addr.ip()),
        port: addr.port(),
    }
}

/// Convert from `smoltcp::wire::IpEndpoint` to `std::net::SocketAddr`.
pub const fn into_core_sockaddr(addr: IpEndpoint) -> SocketAddr {
    SocketAddr::new(into_core_ipaddr(addr.addr), addr.port)
}

pub fn is_unspecified(ip: IpAddress) -> bool {
    ip.as_bytes() == [0, 0, 0, 0]
}

pub const UNSPECIFIED_IP: IpAddress = IpAddress::v4(0, 0, 0, 0);
pub const UNSPECIFIED_ENDPOINT: IpEndpoint = IpEndpoint::new(UNSPECIFIED_IP, 0);

/// Convert from path string to `UnixAddr::Pathname`.
pub fn from_path_str(path: &str) -> UnixAddr {
    UnixAddr::from_path(path)
}

/// Convert from abstract name bytes to `UnixAddr::Abstract`.
pub fn from_abstract_name(name: Vec<u8>) -> UnixAddr {
    UnixAddr::from_abstract(name)
}

/// Create an unnamed Unix socket address.
pub const fn unnamed_unix_addr() -> UnixAddr {
    UnixAddr::Unnamed
}

/// Check if a Unix socket address is unnamed.
pub fn is_unix_addr_unnamed(addr: &UnixAddr) -> bool {
    addr.is_unnamed()
}

/// Convert Unix socket address to a string representation for debugging.
pub fn unix_addr_to_string(addr: &UnixAddr) -> alloc::string::String {
    match addr {
        UnixAddr::Unnamed => alloc::string::String::from("(unnamed)"),
        UnixAddr::Pathname(path) => alloc::format!("path:{}", path),
        UnixAddr::Abstract(name) => {
            let name_str = core::str::from_utf8(name).unwrap_or("<invalid utf8>");
            alloc::format!("abstract:{}", name_str)
        }
    }
}

/// Extract the pathname from a Unix socket address if it's a pathname type.
pub fn extract_unix_pathname(addr: &UnixAddr) -> Option<&str> {
    match addr {
        UnixAddr::Pathname(path) => Some(path.as_str()),
        _ => None,
    }
}

/// Extract the abstract name from a Unix socket address if it's an abstract type.
pub fn extract_unix_abstract_name(addr: &UnixAddr) -> Option<&[u8]> {
    match addr {
        UnixAddr::Abstract(name) => Some(name.as_slice()),
        _ => None,
    }
}

/// Check if two Unix socket addresses are equal.
pub fn unix_addr_eq(addr1: &UnixAddr, addr2: &UnixAddr) -> bool {
    addr1 == addr2
}

/// Unix socket constants
pub const UNNAMED_UNIX_ADDR: UnixAddr = UnixAddr::Unnamed;
