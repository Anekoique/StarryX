use alloc::sync::Arc;
use core::net::{Ipv4Addr, SocketAddr};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axio::{PollState, Read};
use axnet::{TcpSocket, UdpSocket, UnixSocket};
use axsync::Mutex;

use xutils::ctypes::{S_IFSOCK, fs::Kstat};

use crate::fs::{fd::get_file_like, file::FileLike};

pub enum Socket {
    Udp(Mutex<UdpSocket>),
    Tcp(Mutex<TcpSocket>),
    Unix(Mutex<UnixSocket>),
}

macro_rules! impl_socket {
    ($pub:vis fn $name:ident(&self $(,$arg:ident: $arg_ty:ty)*) -> $ret:ty) => {
        $pub fn $name(&self, $($arg: $arg_ty),*) -> $ret {
            match self {
                Socket::Udp(udpsocket) => Ok(udpsocket.lock().$name($($arg),*)?),
                Socket::Tcp(tcpsocket) => Ok(tcpsocket.lock().$name($($arg),*)?),
                Socket::Unix(unixsocket) => Ok(unixsocket.lock().$name($($arg),*)?),
            }
        }
    };
}

impl Socket {
    pub fn recv(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        match self {
            Socket::Udp(udpsocket) => Ok(udpsocket.lock().recv_from(buf).map(|e| e.0)?),
            Socket::Tcp(tcpsocket) => Ok(tcpsocket.lock().recv(buf)?),
            Socket::Unix(unixsocket) => Ok(unixsocket.lock().read(buf)?),
        }
    }

    pub fn sendto(&self, buf: &[u8], addr: SocketAddr) -> LinuxResult<usize> {
        match self {
            // diff: must bind before sendto
            Socket::Udp(udpsocket) => {
                let inner = udpsocket.lock();
                inner
                    .bind(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0))
                    .ok();
                Ok(inner.send_to(buf, addr)?)
            }
            Socket::Tcp(_) => Err(LinuxError::EISCONN),
            Socket::Unix(_) => Err(LinuxError::EOPNOTSUPP),
        }
    }

    pub fn recvfrom(&self, buf: &mut [u8]) -> LinuxResult<(usize, Option<SocketAddr>)> {
        match self {
            // diff: must bind before recvfrom
            Socket::Udp(udpsocket) => Ok(udpsocket
                .lock()
                .recv_from(buf)
                .map(|res| (res.0, Some(res.1)))?),
            Socket::Tcp(tcpsocket) => Ok(tcpsocket.lock().recv(buf).map(|res| (res, None))?),
            Socket::Unix(unixsocket) => Ok(unixsocket.lock().read(buf).map(|res| (res, None))?),
        }
    }

    pub fn listen(&self) -> LinuxResult {
        match self {
            Socket::Udp(_) => Err(LinuxError::EOPNOTSUPP),
            Socket::Tcp(tcpsocket) => Ok(tcpsocket.lock().listen()?),
            Socket::Unix(unixsocket) => Ok(unixsocket.lock().listen()?),
        }
    }

    pub fn accept(&self) -> LinuxResult<Socket> {
        match self {
            Socket::Udp(_) => Err(LinuxError::EOPNOTSUPP),
            Socket::Tcp(tcpsocket) => tcpsocket
                .lock()
                .accept()
                .map(|socket| Socket::Tcp(Mutex::new(socket))),
            Socket::Unix(unixsocket) => unixsocket
                .lock()
                .accept()
                .map(|socket| Socket::Unix(Mutex::new(socket))),
        }
    }

    // These methods need special handling for Unix sockets
    pub fn local_addr(&self) -> LinuxResult<SocketAddr> {
        match self {
            Socket::Udp(udpsocket) => Ok(udpsocket.lock().local_addr()?),
            Socket::Tcp(tcpsocket) => Ok(tcpsocket.lock().local_addr()?),
            Socket::Unix(_) => {
                // Unix sockets don't have IP:port addresses, return a dummy address
                Ok(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0))
            }
        }
    }

    pub fn peer_addr(&self) -> LinuxResult<SocketAddr> {
        match self {
            Socket::Udp(udpsocket) => Ok(udpsocket.lock().peer_addr()?),
            Socket::Tcp(tcpsocket) => Ok(tcpsocket.lock().peer_addr()?),
            Socket::Unix(_) => {
                // Unix sockets don't have IP:port addresses, return a dummy address
                Ok(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0))
            }
        }
    }

    pub fn bind(&self, addr: SocketAddr) -> LinuxResult {
        match self {
            Socket::Udp(udpsocket) => Ok(udpsocket.lock().bind(addr)?),
            Socket::Tcp(tcpsocket) => Ok(tcpsocket.lock().bind(addr)?),
            Socket::Unix(_) => {
                // Unix sockets use different address types
                Err(LinuxError::EOPNOTSUPP)
            }
        }
    }

    pub fn connect(&self, addr: SocketAddr) -> LinuxResult {
        match self {
            Socket::Udp(udpsocket) => Ok(udpsocket.lock().connect(addr)?),
            Socket::Tcp(tcpsocket) => Ok(tcpsocket.lock().connect(addr)?),
            Socket::Unix(_) => {
                // Unix sockets use different address types
                Err(LinuxError::EOPNOTSUPP)
            }
        }
    }

    pub fn set_nagle_enabled(&self, enabled: bool) -> LinuxResult {
        match self {
            Socket::Tcp(tcpsocket) => Ok(tcpsocket.lock().set_nagle_enabled(enabled)?),
            _ => Err(LinuxError::EOPNOTSUPP),
        }
    }

    pub fn set_reuse_addr(&self, reuse_addr: bool) -> LinuxResult {
        match self {
            Socket::Udp(udpsocket) => {
                udpsocket.lock().set_reuse_addr(reuse_addr);
                Ok(())
            }
            Socket::Tcp(tcpsocket) => {
                tcpsocket.lock().set_reuse_addr(reuse_addr);
                Ok(())
            }
            _ => Err(LinuxError::EOPNOTSUPP),
        }
    }

    pub fn set_keep_alive(&self, keep_alive: bool) -> LinuxResult {
        match self {
            Socket::Tcp(tcpsocket) => {
                tcpsocket.lock().set_keep_alive(keep_alive);
                Ok(())
            }
            _ => Err(LinuxError::EOPNOTSUPP),
        }
    }

    pub fn get_recv_buffer_size(&self) -> LinuxResult<u32> {
        Ok(64 * 1024)
    }

    pub fn get_send_buffer_size(&self) -> LinuxResult<u32> {
        Ok(64 * 1024)
    }

    impl_socket!(pub fn send(&self, buf: &[u8]) -> LinuxResult<usize>);
    impl_socket!(pub fn poll(&self) -> LinuxResult<PollState>);
    impl_socket!(pub fn shutdown(&self) -> LinuxResult);
}

impl FileLike for Socket {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        self.recv(buf)
    }

    fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        self.send(buf)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        // not really implemented
        Ok(Kstat {
            mode: S_IFSOCK | 0o777u32, // rwxrwxrwx
            blksize: 4096,
            ..Default::default()
        })
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        self.poll()
    }

    fn set_nonblocking(&self, nonblock: bool) {
        match self {
            Socket::Udp(udpsocket) => udpsocket.lock().set_nonblocking(nonblock),
            Socket::Tcp(tcpsocket) => tcpsocket.lock().set_nonblocking(nonblock),
            Socket::Unix(unixsocket) => unixsocket.lock().set_nonblocking(nonblock),
        }
    }

    fn is_nonblocking(&self) -> bool {
        match self {
            Socket::Udp(udpsocket) => udpsocket.lock().is_nonblocking(),
            Socket::Tcp(tcpsocket) => tcpsocket.lock().is_nonblocking(),
            Socket::Unix(unixsocket) => unixsocket.lock().is_nonblocking(),
        }
    }

    fn from_fd(fd: i32, required: FileFlags, forbidden: FileFlags) -> LinuxResult<Arc<Self>>
    where
        Self: Sized + 'static,
    {
        get_file_like(fd)?
            .validate(required, forbidden)?
            .clone()
            .into_any()
            .downcast::<Self>()
            .map_err(|_| LinuxError::ENOTSOCK)
    }
}
