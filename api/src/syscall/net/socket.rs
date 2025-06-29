use core::net::SocketAddr;

use axerrno::{LinuxError, LinuxResult};
use axnet::{TcpSocket, UdpSocket, UnixSocket};
use axsync::Mutex;

use crate::{
    ctypes::{
        AF_INET, AF_UNIX, IPPROTO_TCP, IPPROTO_UDP, SOCK_CLOEXEC, SOCK_DGRAM, SOCK_STREAM,
        socklen_t,
    },
    fs::FileLike,
    net::{SockAddr, Socket},
    ptr::{UserConstPtr, UserPtr},
};

/// Create a socket.
///
/// # Arguments
/// * `domain` - Communication domain (AF_INET, AF_UNIX)
/// * `ty` - Socket type (SOCK_STREAM, SOCK_DGRAM) and flags
/// * `proto` - Protocol (0 for default)
pub fn sys_socket(domain: u32, ty: u32, proto: u32) -> LinuxResult<isize> {
    // Extract socket type from low bits and flags from high bits
    let sock_type = ty & 0xFF;
    let sock_flags = ty & !0xFF;

    debug!(
        "sys_socket <= domain: {}, ty: {}, proto: {}",
        domain, ty, proto
    );

    if domain != AF_INET {
        return Err(LinuxError::EAFNOSUPPORT);
    }

    let socket = match sock_type {
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

    let fd = socket
        .add_to_fd_table(sock_flags & SOCK_CLOEXEC != 0)
        .map_err(|_| LinuxError::EMFILE)?;

    Ok(fd as isize)
}

fn to_socketaddr(addr: UserConstPtr<u8>, addrlen: u32) -> LinuxResult<SocketAddr> {
    let addr = addr.get_as_slice(addrlen as usize)?;
    let addr = unsafe { SockAddr::read(addr.as_ptr().cast(), addrlen)? };
    SocketAddr::try_from(addr)
}

/// Bind a socket to an address.
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `addr` - Address to bind to
/// * `addrlen` - Length of the address structure
pub fn sys_bind(fd: i32, addr: UserConstPtr<u8>, addrlen: u32) -> LinuxResult<isize> {
    let addr = to_socketaddr(addr, addrlen)?;
    debug!("sys_bind <= fd: {}, addr: {:?}", fd, addr);

    Socket::from_fd(fd)?.bind(addr)?;

    Ok(0)
}

/// Connect a socket to an address.
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `addr` - Address to connect to
/// * `addrlen` - Length of the address structure
pub fn sys_connect(fd: i32, addr: UserConstPtr<u8>, addrlen: u32) -> LinuxResult<isize> {
    let addr = to_socketaddr(addr, addrlen)?;
    debug!("sys_connect <= fd: {}, addr: {:?}", fd, addr);

    Socket::from_fd(fd)?.connect(addr)?;

    Ok(0)
}

/// Get socket name (local address).
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `addr` - Buffer to store the address
/// * `addrlen` - Pointer to address length (input/output)
pub fn sys_getsockname(
    fd: i32,
    addr: UserPtr<u8>,
    addrlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    let socket = Socket::from_fd(fd)?;
    let local_addr = socket.local_addr()?;
    debug!("sys_getsockname <= fd: {}, addr: {:?}", fd, local_addr);

    let local_addr = SockAddr::from(local_addr);
    *addrlen.get_as_mut()? = local_addr.addr_len();
    let bytes = local_addr.bytes();
    addr.get_as_mut_slice(bytes.len())?.copy_from_slice(bytes);

    Ok(0)
}

/// Get peer name (remote address).
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `addr` - Buffer to store the address
/// * `addrlen` - Pointer to address length (input/output)
pub fn sys_getpeername(
    fd: i32,
    addr: UserPtr<u8>,
    addrlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    let socket = Socket::from_fd(fd)?;
    let peer_addr = socket.peer_addr()?;

    debug!("sys_getpeername <= fd: {}, addr: {:?}", fd, peer_addr);

    let peer_addr = SockAddr::from(peer_addr);
    *addrlen.get_as_mut()? = peer_addr.addr_len();
    let bytes = peer_addr.bytes();
    addr.get_as_mut_slice(bytes.len())?.copy_from_slice(bytes);

    Ok(0)
}

/// Listen for connections on a socket.
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `backlog` - Maximum number of pending connections
pub fn sys_listen(fd: i32, backlog: i32) -> LinuxResult<isize> {
    debug!("sys_listen: fd: {}, backlog: {}", fd, backlog);

    if backlog < 0 {
        return Err(LinuxError::EINVAL);
    }

    Socket::from_fd(fd)?.listen()?;

    Ok(0)
}

/// Accept a connection on a socket.
///
/// # Arguments
/// * `fd` - Listening socket file descriptor
/// * `addr` - Buffer to store the client address
/// * `addrlen` - Pointer to address length (input/output)
pub fn sys_accept(fd: i32, addr: UserPtr<u8>, addrlen: UserPtr<socklen_t>) -> LinuxResult<isize> {
    debug!("sys_accept <= fd: {}", fd);

    let socket = Socket::from_fd(fd)?;
    let socket = socket.accept()?;

    let remote_addr = socket.local_addr()?;
    let fd = socket
        .add_to_fd_table(false)
        .map(|fd| fd as isize)
        .map_err(|_| LinuxError::EMFILE)?;
    debug!("sys_accept => fd: {}, addr: {:?}", fd, remote_addr);

    let remote_addr = SockAddr::from(remote_addr);
    *addrlen.get_as_mut()? = remote_addr.addr_len();
    let bytes = remote_addr.bytes();
    addr.get_as_mut_slice(bytes.len())?.copy_from_slice(bytes);

    Ok(fd)
}

/// Send data to a specific address.
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `buf` - Buffer containing data to send
/// * `len` - Length of data to send
/// * `flags` - Send flags
/// * `addr` - Destination address
/// * `addrlen` - Length of the address structure
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

/// Receive data from a socket.
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `buf` - Buffer to store received data
/// * `len` - Maximum length of data to receive
/// * `flags` - Receive flags
/// * `addr` - Buffer to store sender address
/// * `addrlen` - Pointer to address length (input/output)
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
        let remote_addr = SockAddr::from(remote_addr);
        *addrlen.get_as_mut()? = remote_addr.addr_len();
        let bytes = remote_addr.bytes();
        addr.get_as_mut_slice(bytes.len())?.copy_from_slice(bytes);
    } else {
        *addrlen.get_as_mut()? = 0;
    }

    debug!("sys_recvfrom => fd: {}, recv: {}", fd, recv);
    Ok(recv as isize)
}

/// Create a pair of connected sockets.
///
/// # Arguments
/// * `domain` - Communication domain (AF_INET, AF_UNIX)
/// * `ty` - Socket type (SOCK_STREAM, SOCK_DGRAM)
/// * `proto` - Protocol (0 for default)
/// * `sv` - Array to store the two socket file descriptors
pub fn sys_socketpair(domain: u32, ty: u32, proto: u32, sv: UserPtr<i32>) -> LinuxResult<isize> {
    let ty = ty & 0xFF;

    if domain == AF_UNIX {
        // 只支持 SOCK_STREAM/ SOCK_DGRAM
        if ty != SOCK_STREAM && ty != SOCK_DGRAM {
            return Err(LinuxError::ESOCKTNOSUPPORT);
        }
        let (sock1, sock2) = UnixSocket::pair();
        let socket1 = Socket::Unix(Mutex::new(sock1));
        let socket2 = Socket::Unix(Mutex::new(sock2));
        let fd1 = socket1
            .add_to_fd_table(false)
            .map_err(|_| LinuxError::EMFILE)?;
        let fd2 = socket2
            .add_to_fd_table(false)
            .map_err(|_| LinuxError::EMFILE)?;
        let sv_slice = sv.get_as_mut_slice(2)?;
        sv_slice[0] = fd1;
        sv_slice[1] = fd2;
        return Ok(0);
    }

    debug!(
        "sys_socketpair <= domain: {}, ty: {}, proto: {}",
        domain, ty, proto
    );

    // 检查地址族
    if domain != AF_INET {
        return Err(LinuxError::EAFNOSUPPORT);
    }

    // 创建两个相同类型的 socket
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

    // 分配文件描述符
    let fd1 = socket1
        .add_to_fd_table(false)
        .map_err(|_| LinuxError::EMFILE)?;

    let fd2 = socket2
        .add_to_fd_table(false)
        .map_err(|_| LinuxError::EMFILE)?;

    // 将文件描述符写入用户空间
    let sv_slice = sv.get_as_mut_slice(2)?;
    sv_slice[0] = fd1;
    sv_slice[1] = fd2;

    debug!("sys_socketpair => fds: [{}, {}]", fd1, fd2);
    Ok(0)
}
