use core::net::SocketAddr;
use core::sync::atomic::{AtomicBool, Ordering};

use axhal::time::current_ticks;
use axio::{PollState, Read, Write};
use axsync::Mutex;
use spin::RwLock;

use smoltcp::iface::SocketHandle;
use smoltcp::socket::udp::{self, BindError, SendError};
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};

use super::config::UNSPECIFIED_ENDPOINT;
use super::{SOCKET_SET, SocketSetWrapper};
use crate::{NetError, NetResult, net_error_to_axio};

/// A UDP socket that provides POSIX-like APIs.
pub struct UdpSocket {
    handle: SocketHandle,
    local_addr: RwLock<Option<IpEndpoint>>,
    peer_addr: RwLock<Option<IpEndpoint>>,
    nonblock: AtomicBool,
    reuse_addr: AtomicBool,
}

impl UdpSocket {
    /// Creates a new UDP socket.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let socket = SocketSetWrapper::new_udp_socket();
        let handle = SOCKET_SET.add(socket);
        Self {
            handle,
            local_addr: RwLock::new(None),
            peer_addr: RwLock::new(None),
            nonblock: AtomicBool::new(false),
            reuse_addr: AtomicBool::new(false),
        }
    }

    /// Returns the local address and port, or
    /// [`Err(NotConnected)`](NetError::ENOTCONN) if not connected.
    pub fn local_addr(&self) -> NetResult<SocketAddr> {
        match self.local_addr.try_read() {
            Some(addr) => addr.map(IpEndpoint::into).ok_or(NetError::ENOTCONN),
            None => Err(NetError::ENOTCONN),
        }
    }

    /// Returns the remote address and port, or
    /// [`Err(NotConnected)`](NetError::ENOTCONN) if not connected.
    pub fn peer_addr(&self) -> NetResult<SocketAddr> {
        self.remote_endpoint().map(IpEndpoint::into)
    }

    /// Returns whether this socket is in nonblocking mode.
    #[inline]
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// Moves this UDP socket into or out of nonblocking mode.
    ///
    /// This will result in `recv`, `recv_from`, `send`, and `send_to`
    /// operations becoming nonblocking, i.e., immediately returning from their
    /// calls. If the IO operation is successful, `Ok` is returned and no
    /// further action is required. If the IO operation could not be completed
    /// and needs to be retried, an error with kind
    /// [`Err(WouldBlock)`](AxError::WouldBlock) is returned.
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    /// Set the TTL (time-to-live) option for this socket.
    ///
    /// The TTL is the number of hops that a packet is allowed to live.
    pub fn set_socket_ttl(&self, ttl: u8) {
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            socket.set_hop_limit(Some(ttl))
        });
    }

    /// Returns whether this socket is in reuse address mode.
    #[inline]
    pub fn is_reuse_addr(&self) -> bool {
        self.reuse_addr.load(Ordering::Acquire)
    }

    /// Moves this UDP socket into or out of reuse address mode.
    ///
    /// When a socket is bound, the `SO_REUSEADDR` option allows multiple sockets to be bound to the
    /// same address if they are bound to different local addresses. This option must be set before
    /// calling `bind`.
    #[inline]
    pub fn set_reuse_addr(&self, reuse_addr: bool) {
        self.reuse_addr.store(reuse_addr, Ordering::Release);
    }

    /// Binds an unbound socket to the given address and port.
    ///
    /// It's must be called before [`send_to`](Self::send_to) and
    /// [`recv_from`](Self::recv_from).
    pub fn bind(&self, mut local_addr: SocketAddr) -> NetResult {
        let mut self_local_addr = self.local_addr.write();

        if local_addr.port() == 0 {
            local_addr.set_port(get_ephemeral_port()?);
        }
        if self_local_addr.is_some() {
            return Err(NetError::EINVAL);
        }

        let local_endpoint: IpEndpoint = local_addr.into();
        let endpoint = IpListenEndpoint {
            addr: (!local_endpoint.addr.is_unspecified()).then_some(local_endpoint.addr),
            port: local_endpoint.port,
        };

        if !self.is_reuse_addr() {
            // Check if the address is already in use
            SOCKET_SET.bind_check(local_endpoint.addr, local_endpoint.port)?;
        }

        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            socket.bind(endpoint).map_err(|e| match e {
                BindError::InvalidState => NetError::EEXIST,
                BindError::Unaddressable => NetError::EINVAL,
            })
        })?;

        *self_local_addr = Some(local_endpoint);
        debug!("UDP socket {}: bound on {}", self.handle, endpoint);
        Ok(())
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    pub fn send_to(&self, buf: &[u8], remote_addr: SocketAddr) -> NetResult<usize> {
        if remote_addr.port() == 0 || remote_addr.ip().is_unspecified() {
            return Err(NetError::EINVAL);
        }
        self.send_impl(buf, remote_addr.into())
    }

    /// Receives a single datagram message on the socket. On success, returns
    /// the number of bytes read and the origin.
    pub fn recv_from(&self, buf: &mut [u8]) -> NetResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| match socket.recv_slice(buf) {
            Ok((len, meta)) => Ok((len, meta.endpoint.into())),
            Err(_) => Err(NetError::EFAULT),
        })
    }

    /// Receives data from the socket, stores it in the given buffer.
    ///
    /// It will return [`Err(Timeout)`](NetError::ETIMEDOUT) if expired.
    pub fn recv_from_timeout(&self, buf: &mut [u8], ticks: u64) -> NetResult<(usize, SocketAddr)> {
        let expire_at = current_ticks() + ticks;
        self.recv_impl(|socket| match socket.recv_slice(buf) {
            Ok((len, meta)) => Ok((len, meta.endpoint.into())),
            Err(_) => {
                if current_ticks() > expire_at {
                    // TODO:timeout
                    Err(NetError::ETIMEDOUT)
                } else {
                    Err(NetError::EAGAIN)
                }
            }
        })
    }

    /// Receives a single datagram message on the socket, without removing it from
    /// the queue. On success, returns the number of bytes read and the origin.
    pub fn peek_from(&self, buf: &mut [u8]) -> NetResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| match socket.peek_slice(buf) {
            Ok((len, meta)) => Ok((len, meta.endpoint.into())),
            Err(_) => Err(NetError::EFAULT),
        })
    }

    /// Connects this UDP socket to a remote address, allowing the `send` and
    /// `recv` to be used to send data and also applies filters to only receive
    /// data from the specified address.
    ///
    /// The local port will be generated automatically if the socket is not bound.
    /// It's must be called before [`send`](Self::send) and
    /// [`recv`](Self::recv).
    pub fn connect(&self, addr: SocketAddr) -> NetResult {
        let mut self_peer_addr = self.peer_addr.write();

        if self.local_addr.read().is_none() {
            self.bind(UNSPECIFIED_ENDPOINT.into())?;
        }

        *self_peer_addr = Some(addr.into());
        debug!("UDP socket {}: connected to {}", self.handle, addr);
        Ok(())
    }

    /// Sends data on the socket to the remote address to which it is connected.
    pub fn send(&self, buf: &[u8]) -> NetResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.send_impl(buf, remote_endpoint)
    }

    /// Receives a single datagram message on the socket from the remote address
    /// to which it is connected. On success, returns the number of bytes read.
    pub fn recv(&self, buf: &mut [u8]) -> NetResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.recv_impl(|socket| {
            let (len, meta) = socket.recv_slice(buf).map_err(|_| NetError::EFAULT)?;
            if !remote_endpoint.addr.is_unspecified() && remote_endpoint.addr != meta.endpoint.addr
            {
                return Err(NetError::EAGAIN);
            }
            if remote_endpoint.port != 0 && remote_endpoint.port != meta.endpoint.port {
                return Err(NetError::EAGAIN);
            }
            Ok(len)
        })
    }

    /// Close the socket.
    pub fn shutdown(&self) -> NetResult {
        SOCKET_SET.poll_interfaces();
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            debug!("UDP socket {}: shutting down", self.handle);
            socket.close();
        });
        Ok(())
    }

    /// Whether the socket is readable or writable.
    pub fn poll(&self) -> NetResult<PollState> {
        if self.local_addr.read().is_none() {
            return Ok(PollState {
                readable: false,
                writable: false,
            });
        }
        SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
            Ok(PollState {
                readable: socket.can_recv(),
                writable: socket.can_send(),
            })
        })
    }
}

/// Private methods
impl UdpSocket {
    fn remote_endpoint(&self) -> NetResult<IpEndpoint> {
        match self.peer_addr.try_read() {
            Some(addr) => addr.ok_or(NetError::ENOTCONN),
            None => Err(NetError::ENOTCONN),
        }
    }

    fn send_impl(&self, buf: &[u8], remote_endpoint: IpEndpoint) -> NetResult<usize> {
        if self.local_addr.read().is_none() {
            return Err(NetError::ENOTCONN);
        }
        // info!("send to addr: {:?}", remote_endpoint);
        self.block_on(|| {
            SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                if !socket.is_open() {
                    // not connected
                    Err(NetError::ENOTCONN)
                } else if socket.can_send() {
                    socket
                        .send_slice(buf, remote_endpoint)
                        .map_err(|e| match e {
                            SendError::BufferFull => NetError::EAGAIN,
                            SendError::Unaddressable => NetError::ECONNREFUSED,
                        })?;
                    Ok(buf.len())
                } else {
                    // tx buffer is full
                    Err(NetError::EAGAIN)
                }
            })
        })
    }

    fn recv_impl<F, T>(&self, mut op: F) -> NetResult<T>
    where
        F: FnMut(&mut udp::Socket) -> NetResult<T>,
    {
        if self.local_addr.read().is_none() {
            return Err(NetError::ENOTCONN);
        }

        self.block_on(|| {
            SOCKET_SET.with_socket_mut::<udp::Socket, _, _>(self.handle, |socket| {
                if !socket.is_open() {
                    // not bound
                    Err(NetError::ENOTCONN)
                } else if socket.can_recv() {
                    // data available
                    op(socket)
                } else {
                    // no more data
                    Err(NetError::EAGAIN)
                }
            })
        })
    }

    fn block_on<F, T>(&self, mut f: F) -> NetResult<T>
    where
        F: FnMut() -> NetResult<T>,
    {
        if self.is_nonblocking() {
            SOCKET_SET.poll_interfaces();
            f()
        } else {
            loop {
                SOCKET_SET.poll_interfaces();
                match f() {
                    Ok(t) => return Ok(t),
                    Err(NetError::EAGAIN) => axtask::yield_now(),
                    Err(e) => return Err(e),
                }
            }
        }
    }

    /// To get the socket and call the given function.
    ///
    /// If the socket is not connected, it will return None.
    ///
    /// Or it will return the result of the given function.
    pub fn with_socket<R>(&self, f: impl FnOnce(&udp::Socket) -> R) -> R {
        SOCKET_SET.with_socket(self.handle, |s| f(s))
    }
}

impl Read for UdpSocket {
    fn read(&mut self, buf: &mut [u8]) -> axerrno::AxResult<usize> {
        self.recv(buf).map_err(net_error_to_axio)
    }
}

impl Write for UdpSocket {
    fn write(&mut self, buf: &[u8]) -> axerrno::AxResult<usize> {
        self.send(buf).map_err(net_error_to_axio)
    }

    fn flush(&mut self) -> axerrno::AxResult {
        Err(net_error_to_axio(NetError::ENOSYS))
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        self.shutdown().ok();
        SOCKET_SET.remove(self.handle);
    }
}

fn get_ephemeral_port() -> NetResult<u16> {
    const PORT_START: u16 = 0xc000;
    const PORT_END: u16 = 0xffff;
    static CURR: Mutex<u16> = Mutex::new(PORT_START);
    let mut curr = CURR.lock();

    let port = *curr;
    if *curr == PORT_END {
        *curr = PORT_START;
    } else {
        *curr += 1;
    }
    Ok(port)
}
