use core::mem::size_of;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;

use xuspace::{UserPtr, UserSpaceAccess};
use xutils::ctypes::{
    IP_RECVERR, L_IP, L_MAX, L_SOCKET, L_TCP, L_UDP, MCAST_JOIN_GROUP, MCAST_LEAVE_GROUP,
    SO_DONTROUTE, SO_KEEPALIVE, SO_RCVBUF, SO_RCVTIMEO, SO_REUSEADDR, SO_SNDBUF, SO_SNDBUFFORCE,
    TCP_CONGESTION, TCP_INFO, TCP_KEEPIDLE, TCP_MAXSEG, TCP_NODELAY, socklen_t,
};

use xcore::{fs::file::FileLike, net::Socket, task::with_uspace};

const TCP_MAXSEG_DEFAULT: u32 = 1460;
const CONGESTION: &str = "reno";
const CONGESTION_BYTES: &[u8] = CONGESTION.as_bytes();

pub fn sys_getsockopt(
    fd: i32,
    level: i32,
    optname: i32,
    optval: UserPtr<u8>,
    optlen: UserPtr<socklen_t>,
) -> LinuxResult<isize> {
    debug!(
        "sys_getsockopt <= fd: {}, level: {}, optname: {}, optval: {:?}, optlen: {:?}",
        fd,
        level,
        optname,
        optval.address(),
        optlen,
    );

    let optname = optname as u32;
    let socket = Socket::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?;
    if level > L_MAX {
        return Err(LinuxError::EOPNOTSUPP);
    }

    with_uspace(|uspace| match level {
        L_SOCKET => match optname {
            SO_RCVBUF => {
                uspace.write(optval.cast::<u32>(), socket.get_recv_buffer_size()?)?;
                uspace.write(optlen, size_of::<u32>() as socklen_t)
            }
            SO_SNDBUF => {
                uspace.write(optval.cast::<u32>(), socket.get_send_buffer_size()?)?;
                uspace.write(optlen, size_of::<u32>() as socklen_t)
            }
            _ => {
                let _ = uspace.read(optval.cast::<u32>())?;
                let _ = uspace.read(optlen)?;
                Err(LinuxError::ENOPROTOOPT)
            }
        },
        L_TCP => match optname {
            TCP_MAXSEG => {
                uspace.write(optval.cast::<u32>(), TCP_MAXSEG_DEFAULT)?;
                uspace.write(optlen, size_of::<u32>() as socklen_t)
            }
            TCP_CONGESTION => {
                uspace.write_slice(optval.cast::<u8>(), CONGESTION_BYTES)?;
                uspace.write(optlen, CONGESTION_BYTES.len() as socklen_t)
            }
            TCP_INFO => Ok(()),
            _ => {
                let _ = uspace.read(optval.cast::<u32>())?;
                let _ = uspace.read(optlen)?;
                Err(LinuxError::ENOPROTOOPT)
            }
        },
        L_UDP => {
            if optname == 10 {
                return Err(LinuxError::EOPNOTSUPP);
            }
            let _ = uspace.read(optval.cast::<u32>())?;
            let _ = uspace.read(optlen)?;
            Err(LinuxError::ENOPROTOOPT)
        }
        L_IP => {
            let _ = uspace.read(optval.cast::<u32>())?;
            let _ = uspace.read(optlen)?;
            Err(LinuxError::ENOPROTOOPT)
        }
        _ => {
            let _ = uspace.read(optval.cast::<u32>())?;
            let _ = uspace.read(optlen)?;
            Err(LinuxError::ENOPROTOOPT)
        }
    })?;

    Ok(0)
}

pub fn sys_setsockopt(
    fd: i32,
    level: i32,
    optname: i32,
    optval: UserPtr<u8>,
    _optlen: socklen_t,
) -> LinuxResult<isize> {
    debug!(
        "sys_setsockopt <= fd: {}, level: {}, optname: {}, optval: {:?}, optlen: {}",
        fd,
        level,
        optname,
        optval.address(),
        _optlen
    );

    let optname = optname as u32;
    let socket = Socket::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?;
    match level {
        L_SOCKET => match optname {
            SO_REUSEADDR => {
                socket.set_reuse_addr(with_uspace(|uspace| uspace.read(optval.cast::<bool>()))?)?;
            }
            SO_RCVTIMEO => {
                return Ok(0);
            }
            SO_KEEPALIVE => {
                socket.set_keep_alive(with_uspace(|uspace| uspace.read(optval.cast::<bool>()))?)?;
            }
            SO_RCVBUF => return Ok(0),
            SO_SNDBUF => return Ok(0),
            SO_DONTROUTE => return Ok(0),
            SO_SNDBUFFORCE => return Ok(0),
            _ => return Err(LinuxError::ENOPROTOOPT),
        },
        L_TCP => match optname {
            TCP_NODELAY => {
                socket.set_nagle_enabled(!with_uspace(|uspace| {
                    uspace.read(optval.cast::<bool>())
                })?)?;
            }
            TCP_KEEPIDLE => return Ok(0),
            _ => return Err(LinuxError::ENOPROTOOPT),
        },
        L_UDP => return Err(LinuxError::ENOPROTOOPT),
        L_IP => match optname {
            IP_RECVERR => return Ok(0),
            MCAST_JOIN_GROUP => return Ok(0),
            MCAST_LEAVE_GROUP => return Ok(0),
            _ => return Err(LinuxError::ENOPROTOOPT),
        },
        _ => return Err(LinuxError::ENOPROTOOPT),
    }

    Ok(0)
}
