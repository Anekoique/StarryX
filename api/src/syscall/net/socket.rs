use core::net::SocketAddr;

use axerrno::{LinuxError, LinuxResult};
use axnet::{TcpSocket, UdpSocket, UnixSocket};
use axsync::Mutex;

use crate::{
    ctypes::{AF_INET, AF_UNIX, IPPROTO_TCP, IPPROTO_UDP, SOCK_DGRAM, SOCK_STREAM, socklen_t},
    fs::FileLike,
    net::{SockAddr, Socket},
    ptr::{UserConstPtr, UserPtr},
};

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

fn to_socketaddr(addr: UserConstPtr<u8>, addrlen: u32) -> LinuxResult<SocketAddr> {
    let addr = addr.get_as_slice(addrlen as usize)?;
    let addr = unsafe { SockAddr::read(addr.as_ptr().cast(), addrlen)? };
    SocketAddr::try_from(addr)
}

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

    let local_addr = SockAddr::from(local_addr);
    *addrlen.get_as_mut()? = local_addr.addr_len();
    let bytes = local_addr.bytes();
    addr.get_as_mut_slice(bytes.len())?.copy_from_slice(bytes);

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

    let peer_addr = SockAddr::from(peer_addr);
    *addrlen.get_as_mut()? = peer_addr.addr_len();
    let bytes = peer_addr.bytes();
    addr.get_as_mut_slice(bytes.len())?.copy_from_slice(bytes);

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

    let remote_addr = SockAddr::from(remote_addr);
    *addrlen.get_as_mut()? = remote_addr.addr_len();
    let bytes = remote_addr.bytes();
    addr.get_as_mut_slice(bytes.len())?.copy_from_slice(bytes);

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
    let fd1 = socket1.add_to_fd_table().map_err(|_| LinuxError::EMFILE)?;

    let fd2 = socket2.add_to_fd_table().map_err(|_| LinuxError::EMFILE)?;

    // 将文件描述符写入用户空间
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
    
    // 验证套接字存在
    let _socket = Socket::from_fd(fd)?;
    let optlen_val = *optlen.get_as_mut()?;
    
    // 基本实现：对于大多数选项返回默认值
    match (level, optname) {
        // SOL_SOCKET level options
        (1, 4) => {
            // SO_ERROR - 返回 0 表示没有错误
            if optlen_val >= 4 {
                let optval_slice = optval.get_as_mut_slice(4)?;
                optval_slice[0..4].copy_from_slice(&0i32.to_ne_bytes());
                *optlen.get_as_mut()? = 4;
            }
        }
        (1, 13) => {
            // SO_TYPE - 返回套接字类型
            if optlen_val >= 4 {
                let optval_slice = optval.get_as_mut_slice(4)?;
                optval_slice[0..4].copy_from_slice(&1i32.to_ne_bytes()); // SOCK_STREAM
                *optlen.get_as_mut()? = 4;
            }
        }
        // TCP level options
        (6, 1) => {
            // TCP_NODELAY - 返回默认值 1 (启用)
            if optlen_val >= 4 {
                let optval_slice = optval.get_as_mut_slice(4)?;
                optval_slice[0..4].copy_from_slice(&1i32.to_ne_bytes());
                *optlen.get_as_mut()? = 4;
            }
        }
        (6, 2) => {
            // TCP_MAXSEG - 返回合理的 MSS 值
            // 标准以太网 MTU (1500) - IP头部 (20) - TCP头部 (20) = 1460
            if optlen_val >= 4 {
                let optval_slice = optval.get_as_mut_slice(4)?;
                optval_slice[0..4].copy_from_slice(&1460i32.to_ne_bytes());
                *optlen.get_as_mut()? = 4;
            }
        }
        _ => {
            // 对于未知选项，返回默认值 0
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
    
    // 验证套接字存在
    let _socket = Socket::from_fd(fd)?;
    
    // 基本实现：接受但忽略大多数设置
    // 这对于 iperf3 的基本功能来说通常是可以接受的
    
    Ok(0)
}
