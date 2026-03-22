use core::net::SocketAddr;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axnet::{TcpSocket, UdpSocket, UnixSocket};
use axsync::Mutex;

use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::ctypes::{
    AF_INET, AF_INET6, AF_UNIX, IPPROTO_TCP, IPPROTO_UDP, IPPROTO_UDPLITE, SOCK_CLOEXEC,
    SOCK_DGRAM, SOCK_NONBLOCK, SOCK_STREAM, sockaddr, socklen_t,
};

use xcore::{
    fs::file::FileLike,
    net::{Socket, SocketAddrExt},
    task::with_uspace,
};

/// Create a socket.
///
/// # Arguments
/// * `domain` - Communication domain (AF_INET, AF_UNIX)
/// * `ty` - Socket type (SOCK_STREAM, SOCK_DGRAM) and flags
/// * `proto` - Protocol (0 for default)
pub fn sys_socket(domain: u32, ty: u32, proto: u32) -> LinuxResult<isize> {
    debug!(
        "sys_socket <= domain: {}, ty: {}, proto: {}",
        domain, ty, proto
    );
    let sock_type = ty & 0xFF;
    let sock_flags = ty & !0xFF;

    if domain != AF_INET && domain != AF_INET6 && domain != AF_UNIX {
        return Err(LinuxError::EAFNOSUPPORT);
    }

    if domain == AF_INET6 && sock_type == SOCK_STREAM {
        return Err(LinuxError::EAFNOSUPPORT);
    }

    let socket = match (domain, sock_type) {
        (AF_INET, SOCK_STREAM) => {
            if proto != 0 && proto != IPPROTO_TCP as _ {
                return Err(LinuxError::EPROTONOSUPPORT);
            }
            Socket::Tcp(Mutex::new(TcpSocket::new()))
        }
        (AF_INET | AF_INET6, SOCK_DGRAM) => {
            if proto != 0 && proto != IPPROTO_UDP as _ && proto != IPPROTO_UDPLITE as _ {
                return Err(LinuxError::EPROTONOSUPPORT);
            }
            Socket::Udp(Mutex::new(UdpSocket::new()))
        }
        (AF_UNIX, SOCK_STREAM) | (AF_UNIX, SOCK_DGRAM) => {
            if proto != 0 {
                return Err(LinuxError::EPROTONOSUPPORT);
            }
            use axnet::UnixSocket;
            Socket::Unix(Mutex::new(UnixSocket::new()))
        }
        _ => return Err(LinuxError::ESOCKTNOSUPPORT),
    };

    socket.set_nonblocking(sock_flags & SOCK_NONBLOCK != 0);
    socket
        .add_to_fd_table(
            FileFlags::READ | FileFlags::WRITE,
            sock_flags & SOCK_CLOEXEC != 0,
        )
        .map_err(|_| LinuxError::EMFILE)
        .map(|fd| fd as isize)
}

/// Bind a socket to an address.
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `addr` - Address to bind to
/// * `addrlen` - Length of the address structure
pub fn sys_bind(fd: i32, addr: UserConstPtr<sockaddr>, addrlen: u32) -> LinuxResult<isize> {
    let addr = SocketAddr::read_from_user(addr, addrlen)?;
    debug!("sys_bind <= fd: {}, addr: {:?}", fd, addr);

    Socket::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?.bind(addr)?;

    Ok(0)
}

/// Connect a socket to an address.
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `addr` - Address to connect to
/// * `addrlen` - Length of the address structure
pub fn sys_connect(fd: i32, addr: UserConstPtr<sockaddr>, addrlen: u32) -> LinuxResult<isize> {
    let addr = SocketAddr::read_from_user(addr, addrlen)?;
    debug!("sys_connect <= fd: {}, addr: {:?}", fd, addr);

    Socket::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?.connect(addr)?;

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
    addr: UserPtr<sockaddr>,
    addrlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    let socket = Socket::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?;
    let local_addr = socket.local_addr()?;
    debug!("sys_getsockname <= fd: {}, addr: {:?}", fd, local_addr);

    with_uspace(|uspace| {
        uspace.write(addrlen, local_addr.write_to_user(addr)?)?;
        Ok(0)
    })
}

/// Get peer name (remote address).
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `addr` - Buffer to store the address
/// * `addrlen` - Pointer to address length (input/output)
pub fn sys_getpeername(
    fd: i32,
    addr: UserPtr<sockaddr>,
    addrlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    let socket = Socket::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?;
    let peer_addr = socket.peer_addr()?;
    debug!("sys_getpeername <= fd: {}, addr: {:?}", fd, peer_addr);

    with_uspace(|uspace| {
        uspace.write(addrlen, peer_addr.write_to_user(addr)?)?;
        Ok(0)
    })
}

/// Listen for connections on a socket.
///
/// # Arguments
/// * `fd` - Socket file descriptor
/// * `backlog` - Maximum number of pending connections
pub fn sys_listen(fd: i32, backlog: i32) -> LinuxResult<isize> {
    debug!("sys_listen <= fd: {}, backlog: {}", fd, backlog);

    if backlog < 0 {
        return Err(LinuxError::EINVAL);
    }

    Socket::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?.listen()?;

    Ok(0)
}

/// Accept a connection on a socket.
///
/// # Arguments
/// * `fd` - Listening socket file descriptor
/// * `addr` - Buffer to store the client address
/// * `addrlen` - Pointer to address length (input/output)
pub fn sys_accept(
    fd: i32,
    addr: UserPtr<sockaddr>,
    addrlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    sys_accept4(fd, addr, addrlen, 0)
}

pub fn sys_accept4(
    fd: i32,
    addr: UserPtr<sockaddr>,
    addrlen: UserPtr<socklen_t>,
    flags: u32,
) -> LinuxResult<isize> {
    debug!("sys_accept4 <= fd: {}, flags: {}", fd, flags);

    let socket = Socket::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?.accept()?;

    let remote_addr = socket.local_addr()?;
    socket.set_nonblocking(flags & SOCK_NONBLOCK != 0);
    let fd = socket
        .add_to_fd_table(
            FileFlags::READ | FileFlags::WRITE,
            flags & SOCK_CLOEXEC != 0,
        )
        .map(|fd| fd as isize)?;
    debug!("sys_accept => fd: {}, addr: {:?}", fd, remote_addr);

    if !addr.is_null() {
        let len = remote_addr.write_to_user(addr)?;
        with_uspace(|uspace| nullable!(uspace.write(addrlen, len)))?;
    }

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
    addr: UserConstPtr<sockaddr>,
    addrlen: u32,
) -> LinuxResult<isize> {
    let addr = if addr.is_null() || addrlen == 0 {
        None
    } else {
        Some(SocketAddr::read_from_user(addr, addrlen)?)
    };

    debug!(
        "sys_sendto <= fd: {}, len: {}, flags: {}, addr: {:?}",
        fd, len, flags, addr
    );

    let bytes = with_uspace(|uspace| uspace.read_slice(buf, len))?;
    let socket = Socket::from_fd(fd, FileFlags::WRITE, FileFlags::empty())?;

    let sent = if let Some(addr) = addr {
        socket.sendto(bytes, addr)?
    } else {
        socket.send(bytes)?
    };

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
    addr: UserPtr<sockaddr>,
    addrlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    debug!("sys_recvfrom <= fd: {}, len: {}, flags: {}", fd, len, flags);

    with_uspace(|uspace| {
        let socket = Socket::from_fd(fd, FileFlags::READ, FileFlags::empty())?;
        let buf = uspace.raw_slice(buf, len)?;
        let (recv, remote_addr) = socket.recvfrom(buf)?;

        if let Some(remote_addr) = remote_addr
            && !addr.is_null()
        {
            let len = remote_addr.write_to_user(addr)?;
            nullable!(uspace.write(addrlen, len))?;
        }

        debug!("sys_recvfrom => fd: {}, recv: {}", fd, recv);
        Ok(recv as isize)
    })
}

pub fn sys_shutdown(fd: i32, how: i32) -> LinuxResult<isize> {
    debug!("sys_shutdown <= fd: {}, how: {}", fd, how);
    Socket::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?.shutdown()?;
    Ok(0)
}

/// Create a pair of connected sockets.
///
/// # Arguments
/// * `domain` - Communication domain (AF_INET, AF_UNIX)
/// * `ty` - Socket type (SOCK_STREAM, SOCK_DGRAM) and flags
/// * `proto` - Protocol (0 for default)
/// * `sv` - Array to store the two socket file descriptors
pub fn sys_socketpair(domain: u32, ty: u32, proto: u32, sv: UserPtr<i32>) -> LinuxResult<isize> {
    let sock_type = ty & 0xFF;
    let sock_flags = ty & !0xFF;
    debug!(
        "sys_socketpair <= domain: {}, ty: {}, proto: {}",
        domain, ty, proto
    );

    // socketpair is primarily for Unix domain sockets
    if domain != AF_UNIX {
        return Err(LinuxError::EAFNOSUPPORT);
    }

    if proto != 0 {
        return Err(LinuxError::EPROTONOSUPPORT);
    }

    match sock_type {
        SOCK_STREAM | SOCK_DGRAM => {}
        _ => return Err(LinuxError::ESOCKTNOSUPPORT),
    }

    let (unix_socket1, unix_socket2) = UnixSocket::pair();

    let socket1 = Socket::Unix(Mutex::new(unix_socket1));
    let socket2 = Socket::Unix(Mutex::new(unix_socket2));

    if sock_flags & SOCK_NONBLOCK != 0 {
        socket1.set_nonblocking(true);
        socket2.set_nonblocking(true);
    }

    let fd1 = socket1
        .add_to_fd_table(
            FileFlags::READ | FileFlags::WRITE,
            sock_flags & SOCK_CLOEXEC != 0,
        )
        .map_err(|_| LinuxError::EMFILE)?;

    let fd2 = socket2
        .add_to_fd_table(
            FileFlags::READ | FileFlags::WRITE,
            sock_flags & SOCK_CLOEXEC != 0,
        )
        .map_err(|_| LinuxError::EMFILE)?;

    with_uspace(|uspace| {
        let sv_slice = uspace.raw_slice(sv, 2)?;
        sv_slice[0] = fd1;
        sv_slice[1] = fd2;
        debug!("sys_socketpair => fds: [{}, {}]", fd1, fd2);
        Ok(0)
    })
}
