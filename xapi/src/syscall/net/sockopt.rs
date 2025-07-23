use core::mem::size_of;

use axerrno::{LinuxError, LinuxResult};
use axuspace::{UserPtr, UserSpaceAccess};
use xcore::task::with_uspace;

use crate::{
    ctypes::{
        IP_RECVERR, L_IP, L_SOCKET, L_TCP, L_UDP, SO_DONTROUTE, SO_KEEPALIVE, SO_RCVBUF,
        SO_RCVTIMEO, SO_REUSEADDR, SO_SNDBUF, TCP_CONGESTION, TCP_INFO, TCP_MAXSEG, TCP_NODELAY,
        socklen_t,
    },
    fs::FileLike,
    net::Socket,
};

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
    let socket = Socket::from_fd(fd)?;
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
            _ => Err(LinuxError::ENOPROTOOPT),
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
            _ => Err(LinuxError::ENOPROTOOPT),
        },
        L_UDP => Err(LinuxError::ENOPROTOOPT),
        L_IP => Err(LinuxError::ENOPROTOOPT),
        _ => Err(LinuxError::ENOPROTOOPT),
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
    let socket = Socket::from_fd(fd)?;
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
            _ => return Err(LinuxError::ENOPROTOOPT),
        },
        L_TCP => match optname {
            TCP_NODELAY => {
                socket.set_nagle_enabled(!with_uspace(|uspace| {
                    uspace.read(optval.cast::<bool>())
                })?)?;
            }
            _ => return Err(LinuxError::ENOPROTOOPT),
        },
        L_UDP => return Err(LinuxError::ENOPROTOOPT),
        L_IP => match optname {
            IP_RECVERR => return Ok(0),
            _ => return Err(LinuxError::ENOPROTOOPT),
        },
        _ => return Err(LinuxError::ENOPROTOOPT),
    }

    Ok(0)
}
