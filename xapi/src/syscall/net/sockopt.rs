use core::mem::size_of;

use axerrno::{LinuxError, LinuxResult};
use axuspace::{UserPtr, UserSpaceAccess};
use xcore::task::with_uspace;

use crate::{
    ctypes::{
        IPPROTO_IP, IPPROTO_TCP, IPPROTO_UDP, SO_RCVBUF, SO_REUSEADDR, SO_SNDBUF, SOL_SOCKET,
        TCP_CONGESTION, TCP_INFO, TCP_MAXSEG, TCP_NODELAY, socklen_t, SO_RCVTIMEO,
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

    if level == SOL_SOCKET as _ {
        if optname == SO_RCVBUF as _ {
            let socket = Socket::from_fd(fd)?;
            let len = socket.get_recv_buffer_size()?;
            with_uspace(|uspace| -> LinuxResult<()> {
                uspace.write(optval.cast::<u32>(), len)?;
                uspace.write(optlen, size_of::<u32>() as socklen_t)?;
                Ok(())
            })?;
        } else if optname == SO_SNDBUF as _ {
            let socket = Socket::from_fd(fd)?;
            let len = socket.get_send_buffer_size()?;
            with_uspace(|uspace| -> LinuxResult<()> {
                uspace.write(optval.cast::<u32>(), len)?;
                uspace.write(optlen, size_of::<u32>() as socklen_t)?;
                Ok(())
            })?;
        } else {
        }
    } else if level == IPPROTO_TCP as _ {
        if optname == TCP_MAXSEG as _ {
            let len = TCP_MAXSEG_DEFAULT;
            with_uspace(|uspace| -> LinuxResult<()> {
                uspace.write(optval.cast::<u32>(), len)?;
                uspace.write(optlen, size_of::<u32>() as socklen_t)?;
                Ok(())
            })?;
        } else if optname == TCP_CONGESTION as _ {
            // FIXME: implement this
            with_uspace(|uspace| -> LinuxResult<()> {
                uspace.write_slice(optval.cast::<u8>(), CONGESTION_BYTES)?;
                uspace.write(optlen, CONGESTION_BYTES.len() as socklen_t)?;
                Ok(())
            })?;
            return Ok(0);
        } else if optname == TCP_INFO as _ {
            return Ok(0);
        } else {
            return Err(LinuxError::ENOPROTOOPT);
        }
    } else if level == IPPROTO_IP as _ {
        return Err(LinuxError::ENOPROTOOPT);
    } else {
        return Err(LinuxError::ENOPROTOOPT);
    }
    Ok(0)
}

pub fn sys_setsockopt(
    fd: i32,
    level: i32,
    optname: i32,
    optval: UserPtr<u8>,
    optlen: socklen_t,
) -> LinuxResult<isize> {
    debug!(
        "sys_setsockopt <= fd: {}, level: {}, optname: {}, optval: {:?}, optlen: {}",
        fd,
        level,
        optname,
        optval.address(),
        optlen
    );

    let socket = Socket::from_fd(fd)?;
    if level == SOL_SOCKET as _ {
        if optname == SO_REUSEADDR as _ {
            socket.set_reuse_addr(true)?;
        } else if optname == SO_RCVTIMEO as _ {
            return Ok(0);
        }
    } else if level == IPPROTO_IP as _ {
        match optname {
            _ => return Err(LinuxError::ENOPROTOOPT),
        }
    } else if level == IPPROTO_TCP as _ {
        if optname == TCP_NODELAY as _ {
            socket.set_nagle_enabled(false)?;
        } else {
            return Err(LinuxError::ENOPROTOOPT);
        }
    } else if level == IPPROTO_UDP as _ {
        match optname {
            _ => return Err(LinuxError::ENOPROTOOPT),
        }
    } else {
        return Err(LinuxError::ENOPROTOOPT);
    }
    Ok(0)
}
