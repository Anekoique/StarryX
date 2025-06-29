//! Socket system call implementations for the StarryX operating system.
//!
//! This module provides implementations for various socket-related system calls,
//! including socket creation, binding, connecting, listening, accepting connections,
//! and data transmission. It supports both TCP and UDP sockets over IPv4,
//! as well as Unix domain sockets.
//!
//! The implementations use the `SocketAddrExt` trait for safe user-space to kernel-space
//! address conversion and provide error handling compatible with Linux system call conventions.

use core::net::SocketAddr;

use axerrno::{LinuxError, LinuxResult};
use axnet::{TcpSocket, UdpSocket, UnixSocket};
use axsync::Mutex;

use crate::{
    backend::net::SocketAddrExt,
    ctypes::{AF_INET, AF_UNIX, IPPROTO_TCP, IPPROTO_UDP, SOCK_DGRAM, SOCK_STREAM, socklen_t},
    fs::FileLike,
    net::Socket,
    ptr::{UserConstPtr, UserPtr},
};
use linux_raw_sys::net::sockaddr;

/// Creates a socket endpoint for communication.
/// 
/// # Arguments
/// * `domain` - Communication domain (e.g., AF_INET for IPv4)
/// * `ty` - Socket type (e.g., SOCK_STREAM for TCP, SOCK_DGRAM for UDP)  
/// * `proto` - Protocol to use (0 for default protocol)
///
/// # Returns
/// * `Ok(fd)` - File descriptor of the created socket on success
/// * `Err(LinuxError)` - Error code on failure
///
/// # Errors
/// * `EAFNOSUPPORT` - Address family not supported
/// * `EPROTONOSUPPORT` - Protocol not supported 
/// * `ESOCKTNOSUPPORT` - Socket type not supported
/// * `EMFILE` - Too many open files
pub fn sys_socket(domain: u32, ty: u32, proto: u32) -> LinuxResult<isize> {
    let ty = ty & 0xFF;

    debug!(
        "sys_socket <= domain: {}, ty: {}, proto: {}",
        domain, ty, proto
    );

    if domain != AF_INET {
        return Err(LinuxError::EAFNOSUPPORT);
    }

    let socket = match ty {
        SOCK_STREAM => {
            if proto != 0 && proto != IPPROTO_TCP as _ {
                return Err(LinuxError::EPROTONOSUPPORT);
            }
            Socket::Tcp(Mutex::new(TcpSocket::new()))
        }
        SOCK_DGRAM => {
            if proto != 0 && proto != IPPROTO_UDP as _ {
                return Err(LinuxError::EPROTONOSUPPORT);
            }
            Socket::Udp(Mutex::new(UdpSocket::new()))
        }
        _ => return Err(LinuxError::ESOCKTNOSUPPORT),
    };

    socket
        .add_to_fd_table()
        .map(|fd| fd as isize)
        .map_err(|_| LinuxError::EMFILE)
}

/// Converts a user-space socket address to a kernel SocketAddr.
///
/// # Arguments
/// * `addr` - Pointer to user-space sockaddr structure
/// * `addrlen` - Length of the address structure
///
/// # Returns
/// * `Ok(SocketAddr)` - Converted socket address on success
/// * `Err(LinuxError)` - Error code on failure
///
/// # Safety
/// This function safely reads from user space using the SocketAddrExt trait.
fn to_socketaddr(addr: UserConstPtr<u8>, addrlen: socklen_t) -> LinuxResult<SocketAddr> {
    let sockaddr_ptr = addr.cast::<sockaddr>();
    SocketAddr::read_from_user(sockaddr_ptr, addrlen)
}

/// Binds a socket to a local address.
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `addr` - Pointer to sockaddr structure containing the address to bind to
/// * `addrlen` - Size of the address structure
///
/// # Returns
/// * `Ok(0)` - Success
/// * `Err(LinuxError)` - Error code on failure
pub fn sys_bind(fd: i32, addr: UserConstPtr<u8>, addrlen: u32) -> LinuxResult<isize> {
    let addr = to_socketaddr(addr, addrlen)?;
    debug!("sys_bind <= fd: {}, addr: {:?}", fd, addr);

    Socket::from_fd(fd)?.bind(addr)?;

    Ok(0)
}

pub fn sys_connect(fd: i32, addr: UserConstPtr<u8>, addrlen: u32) -> LinuxResult<isize> {
    let addr = to_socketaddr(addr, addrlen)?;
    debug!("sys_connect <= fd: {}, addr: {:?}", fd, addr);

    Socket::from_fd(fd)?.connect(addr)?;

    Ok(0)
}

pub fn sys_getsockname(
    fd: i32,
    addr: UserPtr<u8>,
    addrlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    let socket = Socket::from_fd(fd)?;
    let local_addr = socket.local_addr()?;
    debug!("sys_getsockname <= fd: {}, addr: {:?}", fd, local_addr);

    if addr.is_null() {
        return Err(LinuxError::EFAULT);
    }

    let sockaddr_ptr = addr.cast::<sockaddr>();
    let written_len = local_addr.write_to_user(sockaddr_ptr)?;
    *addrlen.get_as_mut()? = written_len;

    Ok(0)
}

pub fn sys_getpeername(
    fd: i32,
    addr: UserPtr<u8>,
    addrlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    let socket = Socket::from_fd(fd)?;
    let peer_addr = socket.peer_addr()?;

    debug!("sys_getpeername <= fd: {}, addr: {:?}", fd, peer_addr);

    if addr.is_null() {
        return Err(LinuxError::EFAULT);
    }

    let sockaddr_ptr = addr.cast::<sockaddr>();
    let written_len = peer_addr.write_to_user(sockaddr_ptr)?;
    *addrlen.get_as_mut()? = written_len;

    Ok(0)
}

pub fn sys_listen(fd: i32, backlog: i32) -> LinuxResult<isize> {
    debug!("sys_listen: fd: {}, backlog: {}", fd, backlog);

    if backlog < 0 {
        return Err(LinuxError::EINVAL);
    }

    Socket::from_fd(fd)?.listen()?;

    Ok(0)
}

/// Accepts a connection on a listening socket.
///
/// # Arguments
/// * `fd` - Listening socket file descriptor
/// * `addr` - Optional pointer to buffer to store peer address (can be null)
/// * `addrlen` - Optional pointer to address length (can be null)
///
/// # Returns
/// * `Ok(new_fd)` - File descriptor of the accepted connection
/// * `Err(LinuxError)` - Error code on failure
///
/// # Notes
/// If `addr` and `addrlen` are provided, the peer address information
/// will be written to the user-space buffer.
pub fn sys_accept(fd: i32, addr: UserPtr<u8>, addrlen: UserPtr<socklen_t>) -> LinuxResult<isize> {
    debug!("sys_accept <= fd: {}", fd);

    let socket = Socket::from_fd(fd)?;
    let socket = socket.accept()?;

    let remote_addr = socket.local_addr()?;
    let fd = socket
        .add_to_fd_table()
        .map(|fd| fd as isize)
        .map_err(|_| LinuxError::EMFILE)?;
    debug!("sys_accept => fd: {}, addr: {:?}", fd, remote_addr);

    // If user provided address buffer, write address information
    if !addr.is_null() && !addrlen.is_null() {
        let sockaddr_ptr = addr.cast::<sockaddr>();
        let written_len = remote_addr.write_to_user(sockaddr_ptr)?;
        *addrlen.get_as_mut()? = written_len;
    }

    Ok(fd)
}

pub fn sys_sendto(
    fd: i32,
    buf: UserConstPtr<u8>,
    len: usize,
    flags: u32,
    addr: UserConstPtr<u8>,
    addrlen: u32,
) -> LinuxResult<isize> {
    let addr = to_socketaddr(addr, addrlen)?;
    debug!(
        "sys_sendto <= fd: {}, len: {}, flags: {}, addr: {:?}",
        fd, len, flags, addr
    );

    let bytes = buf.get_as_slice(len)?;
    let socket = Socket::from_fd(fd)?;
    let sent = socket.sendto(bytes, addr)?;

    Ok(sent as isize)
}

pub fn sys_recvfrom(
    fd: i32,
    buf: UserPtr<u8>,
    len: usize,
    flags: u32,
    addr: UserPtr<u8>,
    addrlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    debug!("sys_recvfrom <= fd: {}, len: {}, flags: {}", fd, len, flags);

    let socket = Socket::from_fd(fd)?;
    let buf = buf.get_as_mut_slice(len)?;
    let (recv, remote_addr) = socket.recvfrom(buf)?;

    if let Some(remote_addr) = remote_addr {
        // If user provided address buffer, write address information
        if !addr.is_null() && !addrlen.is_null() {
            let sockaddr_ptr = addr.cast::<sockaddr>();
            let written_len = remote_addr.write_to_user(sockaddr_ptr)?;
            *addrlen.get_as_mut()? = written_len;
        }
    } else {
        // Even if there's no remote address, set addrlen to 0 if user provided it
        if !addrlen.is_null() {
            *addrlen.get_as_mut()? = 0;
        }
    }

    debug!("sys_recvfrom => fd: {}, recv: {}", fd, recv);
    Ok(recv as isize)
}

pub fn sys_socketpair(domain: u32, ty: u32, proto: u32, sv: UserPtr<i32>) -> LinuxResult<isize> {
    let ty = ty & 0xFF;

    if domain == AF_UNIX {
        // Only support SOCK_STREAM/SOCK_DGRAM for Unix domain sockets
        if ty != SOCK_STREAM && ty != SOCK_DGRAM {
            return Err(LinuxError::ESOCKTNOSUPPORT);
        }
        let (sock1, sock2) = UnixSocket::pair();
        let socket1 = Socket::Unix(Mutex::new(sock1));
        let socket2 = Socket::Unix(Mutex::new(sock2));
        let fd1 = socket1.add_to_fd_table().map_err(|_| LinuxError::EMFILE)?;
        let fd2 = socket2.add_to_fd_table().map_err(|_| LinuxError::EMFILE)?;
        let sv_slice = sv.get_as_mut_slice(2)?;
        sv_slice[0] = fd1;
        sv_slice[1] = fd2;
        return Ok(0);
    }

    debug!(
        "sys_socketpair <= domain: {}, ty: {}, proto: {}",
        domain, ty, proto
    );

    // Check address family
    if domain != AF_INET {
        return Err(LinuxError::EAFNOSUPPORT);
    }

    // Create two sockets of the same type
    let socket1 = match ty {
        SOCK_STREAM => {
            if proto != 0 && proto != IPPROTO_TCP as _ {
                return Err(LinuxError::EPROTONOSUPPORT);
            }
            Socket::Tcp(Mutex::new(TcpSocket::new()))
        }
        SOCK_DGRAM => {
            if proto != 0 && proto != IPPROTO_UDP as _ {
                return Err(LinuxError::EPROTONOSUPPORT);
            }
            Socket::Udp(Mutex::new(UdpSocket::new()))
        }
        _ => return Err(LinuxError::ESOCKTNOSUPPORT),
    };

    let socket2 = match ty {
        SOCK_STREAM => Socket::Tcp(Mutex::new(TcpSocket::new())),
        SOCK_DGRAM => Socket::Udp(Mutex::new(UdpSocket::new())),
        _ => return Err(LinuxError::ESOCKTNOSUPPORT),
    };

    // Allocate file descriptors
    let fd1 = socket1.add_to_fd_table().map_err(|_| LinuxError::EMFILE)?;

    let fd2 = socket2.add_to_fd_table().map_err(|_| LinuxError::EMFILE)?;

    // Write file descriptors to user space
    let sv_slice = sv.get_as_mut_slice(2)?;
    sv_slice[0] = fd1;
    sv_slice[1] = fd2;

    debug!("sys_socketpair => fds: [{}, {}]", fd1, fd2);
    Ok(0)
}

pub fn sys_getsockopt(
    fd: i32,
    level: i32,
    optname: i32,
    optval: UserPtr<u8>,
    optlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    debug!("sys_getsockopt <= fd: {}, level: {}, optname: {}", fd, level, optname);
    
    // Verify socket exists
    let _socket = Socket::from_fd(fd)?;
    let optlen_val = *optlen.get_as_mut()?;
    
    // Basic implementation: return default values for most options
    match (level, optname) {
        // SOL_SOCKET level options
        (1, 4) => {
            // SO_ERROR - return 0 indicating no error
            if optlen_val >= 4 {
                let optval_slice = optval.get_as_mut_slice(4)?;
                optval_slice[0..4].copy_from_slice(&0i32.to_ne_bytes());
                *optlen.get_as_mut()? = 4;
            }
        }
        (1, 13) => {
            // SO_TYPE - return socket type
            if optlen_val >= 4 {
                let optval_slice = optval.get_as_mut_slice(4)?;
                optval_slice[0..4].copy_from_slice(&1i32.to_ne_bytes()); // SOCK_STREAM
                *optlen.get_as_mut()? = 4;
            }
        }
        // TCP level options
        (6, 1) => {
            // TCP_NODELAY - return default value 1 (enabled)
            if optlen_val >= 4 {
                let optval_slice = optval.get_as_mut_slice(4)?;
                optval_slice[0..4].copy_from_slice(&1i32.to_ne_bytes());
                *optlen.get_as_mut()? = 4;
            }
        }
        (6, 2) => {
            // TCP_MAXSEG - return reasonable MSS value
            // Standard Ethernet MTU (1500) - IP header (20) - TCP header (20) = 1460
            if optlen_val >= 4 {
                let optval_slice = optval.get_as_mut_slice(4)?;
                optval_slice[0..4].copy_from_slice(&1460i32.to_ne_bytes());
                *optlen.get_as_mut()? = 4;
            }
        }
        _ => {
            // For unknown options, return default value 0
            if optlen_val >= 4 {
                let optval_slice = optval.get_as_mut_slice(4)?;
                optval_slice[0..4].copy_from_slice(&0i32.to_ne_bytes());
                *optlen.get_as_mut()? = 4;
            }
        }
    }
    
    Ok(0)
}

pub fn sys_setsockopt(
    fd: i32,
    level: i32,
    optname: i32,
    _optval: UserConstPtr<u8>,
    _optlen: socklen_t,
) -> LinuxResult<isize> {
    debug!("sys_setsockopt <= fd: {}, level: {}, optname: {}, optlen: {}", fd, level, optname, _optlen);
    
    // Verify socket exists
    let _socket = Socket::from_fd(fd)?;
    
    // Basic implementation: accept but ignore most settings
    // This is usually acceptable for basic functionality like iperf3
    
    Ok(0)
}
