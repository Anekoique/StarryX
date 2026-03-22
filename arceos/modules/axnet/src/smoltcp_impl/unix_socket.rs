use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use spin::Mutex;

use axio::{PollState, Read, Write};
use axtask::yield_now;

use crate::{NetError, NetResult, net_error_to_axio};

// Unix Socket address type
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnixAddr {
    Unnamed,
    Pathname(alloc::string::String),
    Abstract(Vec<u8>),
}

impl UnixAddr {
    pub fn from_path(path: &str) -> Self {
        Self::Pathname(alloc::string::String::from(path))
    }

    pub fn from_abstract(name: Vec<u8>) -> Self {
        Self::Abstract(name)
    }

    pub fn is_unnamed(&self) -> bool {
        matches!(self, Self::Unnamed)
    }
}

// Unix Socket state
const STATE_CLOSED: u8 = 0;
const STATE_BUSY: u8 = 1;
const STATE_CONNECTING: u8 = 2;
const STATE_CONNECTED: u8 = 3;
const STATE_LISTENING: u8 = 4;

// Message buffer
#[derive(Debug)]
struct MessageBuffer {
    data: VecDeque<u8>,
    max_size: usize,
}

impl MessageBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            data: VecDeque::new(),
            max_size,
        }
    }

    fn write(&mut self, buf: &[u8]) -> NetResult<usize> {
        if self.data.len() + buf.len() > self.max_size {
            return Err(NetError::EAGAIN);
        }

        for &byte in buf {
            self.data.push_back(byte);
        }
        Ok(buf.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> usize {
        let to_read = buf.len().min(self.data.len());
        for slot in buf.iter_mut().take(to_read) {
            *slot = self.data.pop_front().unwrap();
        }
        to_read
    }

    #[allow(dead_code)]
    fn len(&self) -> usize {
        self.data.len()
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn available_space(&self) -> usize {
        self.max_size - self.data.len()
    }
}

// Connection pair, used for connected socket
#[derive(Debug, Clone)]
struct ConnectionPair {
    send_buf: Arc<Mutex<MessageBuffer>>,
    recv_buf: Arc<Mutex<MessageBuffer>>,
    peer_closed: Arc<AtomicBool>,
}

impl ConnectionPair {
    fn new(buffer_size: usize) -> (Self, Self) {
        let buf1 = Arc::new(Mutex::new(MessageBuffer::new(buffer_size)));
        let buf2 = Arc::new(Mutex::new(MessageBuffer::new(buffer_size)));
        let closed1 = Arc::new(AtomicBool::new(false));
        let closed2 = Arc::new(AtomicBool::new(false));

        let conn1 = ConnectionPair {
            send_buf: buf1.clone(),
            recv_buf: buf2.clone(),
            peer_closed: closed2.clone(),
        };

        let conn2 = ConnectionPair {
            send_buf: buf2,
            recv_buf: buf1,
            peer_closed: closed1,
        };

        (conn1, conn2)
    }
}

// Global listen table - using BTreeMap instead of HashMap
type ListenTable = Arc<Mutex<BTreeMap<UnixAddr, VecDeque<UnixSocket>>>>;
static LISTEN_TABLE: spin::Lazy<ListenTable> =
    spin::Lazy::new(|| Arc::new(Mutex::new(BTreeMap::new())));

/// Unix Domain Socket implementation
///
/// Supports Unix Socket of SOCK_STREAM type, providing similar POSIX API:
/// - `connect` for client connection
/// - `bind`, `listen`, `accept` for server
/// - `send`, `recv` for data transmission
pub struct UnixSocket {
    state: AtomicU8,
    local_addr: UnsafeCell<UnixAddr>,
    peer_addr: UnsafeCell<UnixAddr>,
    connection: UnsafeCell<Option<ConnectionPair>>,
    nonblock: AtomicBool,
    buffer_size: usize,
}

unsafe impl Sync for UnixSocket {}

impl UnixSocket {
    /// Create a new Unix Socket
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_CLOSED),
            local_addr: UnsafeCell::new(UnixAddr::Unnamed),
            peer_addr: UnsafeCell::new(UnixAddr::Unnamed),
            connection: UnsafeCell::new(None),
            nonblock: AtomicBool::new(false),
            buffer_size: 8192, // Default buffer size
        }
    }

    /// Create socket pair
    pub fn pair() -> (Self, Self) {
        let (conn1, conn2) = ConnectionPair::new(8192);

        let socket1 = Self {
            state: AtomicU8::new(STATE_CONNECTED),
            local_addr: UnsafeCell::new(UnixAddr::Unnamed),
            peer_addr: UnsafeCell::new(UnixAddr::Unnamed),
            connection: UnsafeCell::new(Some(conn1)),
            nonblock: AtomicBool::new(false),
            buffer_size: 8192,
        };

        let socket2 = Self {
            state: AtomicU8::new(STATE_CONNECTED),
            local_addr: UnsafeCell::new(UnixAddr::Unnamed),
            peer_addr: UnsafeCell::new(UnixAddr::Unnamed),
            connection: UnsafeCell::new(Some(conn2)),
            nonblock: AtomicBool::new(false),
            buffer_size: 8192,
        };

        (socket1, socket2)
    }

    /// Create a connected Unix Socket
    fn new_connected(
        local_addr: UnixAddr,
        peer_addr: UnixAddr,
        connection: ConnectionPair,
    ) -> Self {
        Self {
            state: AtomicU8::new(STATE_CONNECTED),
            local_addr: UnsafeCell::new(local_addr),
            peer_addr: UnsafeCell::new(peer_addr),
            connection: UnsafeCell::new(Some(connection)),
            nonblock: AtomicBool::new(false),
            buffer_size: 8192,
        }
    }

    /// Get local address
    pub fn local_addr(&self) -> NetResult<UnixAddr> {
        match self.get_state() {
            STATE_CONNECTED | STATE_LISTENING => {
                Ok(unsafe { self.local_addr.get().read() }.clone())
            }
            _ => Err(NetError::ENOTCONN),
        }
    }

    /// Get peer address
    pub fn peer_addr(&self) -> NetResult<UnixAddr> {
        match self.get_state() {
            STATE_CONNECTED => Ok(unsafe { self.peer_addr.get().read() }.clone()),
            _ => Err(NetError::ENOTCONN),
        }
    }

    /// Check if it is non-blocking mode
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// Set non-blocking mode
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    /// Set buffer size
    pub fn set_buffer_size(&mut self, size: usize) {
        self.buffer_size = size;
    }

    /// Connect to a specific address
    pub fn connect(&self, addr: UnixAddr) -> NetResult {
        self.update_state(STATE_CLOSED, STATE_CONNECTING, || {
            // Check if the target address is being listened to
            let mut listen_table = LISTEN_TABLE.lock();
            let mut listeners = listen_table.get_mut(&addr);

            if listeners.is_none() || listeners.as_ref().unwrap().is_empty() {
                return Err(NetError::ECONNREFUSED);
            }

            // Create connection pair
            let (client_conn, server_conn) = ConnectionPair::new(self.buffer_size);

            // Create server socket
            let server_addr = addr.clone();
            let client_addr = UnixAddr::Unnamed; // Client usually uses unnamed address
            let server_socket =
                UnixSocket::new_connected(server_addr, client_addr.clone(), server_conn);

            // Add server socket to accept queue
            listeners.as_mut().unwrap().push_back(server_socket);

            // Set client connection information
            unsafe {
                self.peer_addr.get().write(addr);
                self.local_addr.get().write(client_addr);
                self.connection.get().write(Some(client_conn));
            }

            Ok(())
        })
        .unwrap_or(Err(NetError::EEXIST))?;

        self.set_state(STATE_CONNECTED);
        Ok(())
    }

    /// Bind to a specific address
    pub fn bind(&self, addr: UnixAddr) -> NetResult {
        self.update_state(STATE_CLOSED, STATE_CLOSED, || {
            // Check if the address is already in use
            if matches!(addr, UnixAddr::Pathname(_)) {
                let listen_table = LISTEN_TABLE.lock();
                if listen_table.contains_key(&addr) {
                    return Err(NetError::EADDRINUSE);
                }
            }

            unsafe {
                let old_addr = self.local_addr.get().read();
                if !old_addr.is_unnamed() {
                    return Err(NetError::EINVAL);
                }
                self.local_addr.get().write(addr);
            }
            Ok(())
        })
        .unwrap_or(Err(NetError::EINVAL))
    }

    /// Start listening
    pub fn listen(&self) -> NetResult {
        self.update_state(STATE_CLOSED, STATE_LISTENING, || {
            let local_addr = unsafe { self.local_addr.get().read() }.clone();
            if local_addr.is_unnamed() {
                return Err(NetError::EINVAL);
            }

            let mut listen_table = LISTEN_TABLE.lock();
            listen_table.insert(local_addr, VecDeque::new());
            Ok(())
        })
        .unwrap_or(Ok(())) // Ignore repeated listening
    }

    /// Accept a connection
    pub fn accept(&self) -> NetResult<UnixSocket> {
        if !self.is_listening() {
            return Err(NetError::EINVAL);
        }

        let local_addr = unsafe { self.local_addr.get().read() }.clone();

        self.block_on(|| {
            let mut listen_table = LISTEN_TABLE.lock();
            match listen_table.get_mut(&local_addr) {
                Some(listeners) => {
                    if let Some(client_socket) = listeners.pop_front() {
                        Ok(client_socket)
                    } else {
                        Err(NetError::EAGAIN)
                    }
                }
                None => Err(NetError::EINVAL),
            }
        })
    }

    /// Send data
    pub fn send(&self, buf: &[u8]) -> NetResult<usize> {
        if !self.is_connected() {
            return Err(NetError::ENOTCONN);
        }

        let connection = unsafe {
            match (*self.connection.get()).as_ref() {
                Some(conn) => conn.clone(),
                None => return Err(NetError::ENOTCONN),
            }
        };

        if connection.peer_closed.load(Ordering::Acquire) {
            return Err(NetError::ECONNRESET);
        }

        self.block_on(|| {
            let mut send_buf = connection.send_buf.lock();
            if send_buf.available_space() == 0 {
                Err(NetError::EAGAIN)
            } else {
                send_buf.write(buf)
            }
        })
    }

    /// Receive data
    pub fn recv(&self, buf: &mut [u8]) -> NetResult<usize> {
        if !self.is_connected() {
            return Err(NetError::ENOTCONN);
        }

        let connection = unsafe {
            match (*self.connection.get()).as_ref() {
                Some(conn) => conn.clone(),
                None => return Err(NetError::ENOTCONN),
            }
        };

        self.block_on(|| {
            let mut recv_buf = connection.recv_buf.lock();
            if recv_buf.is_empty() {
                if connection.peer_closed.load(Ordering::Acquire) {
                    Ok(0) // EOF
                } else {
                    Err(NetError::EAGAIN)
                }
            } else {
                Ok(recv_buf.read(buf))
            }
        })
    }

    /// Close socket
    pub fn shutdown(&self) -> NetResult {
        match self.get_state() {
            STATE_CONNECTED => {
                unsafe {
                    if let Some(connection) = (*self.connection.get()).as_ref() {
                        connection.peer_closed.store(true, Ordering::Release);
                    }
                }
                self.set_state(STATE_CLOSED);
            }
            STATE_LISTENING => {
                let local_addr = unsafe { self.local_addr.get().read() }.clone();
                let mut listen_table = LISTEN_TABLE.lock();
                listen_table.remove(&local_addr);
                self.set_state(STATE_CLOSED);
            }
            _ => {}
        }

        unsafe {
            self.local_addr.get().write(UnixAddr::Unnamed);
            self.peer_addr.get().write(UnixAddr::Unnamed);
            self.connection.get().write(None);
        }

        Ok(())
    }

    /// Poll socket state
    pub fn poll(&self) -> NetResult<PollState> {
        match self.get_state() {
            STATE_CONNECTED => self.poll_stream(),
            STATE_LISTENING => self.poll_listener(),
            _ => Ok(PollState {
                readable: false,
                writable: false,
            }),
        }
    }

    // Private methods
    fn get_state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    fn set_state(&self, state: u8) {
        self.state.store(state, Ordering::Release);
    }

    fn update_state<F, T>(&self, expect: u8, new: u8, f: F) -> Result<NetResult<T>, u8>
    where
        F: FnOnce() -> NetResult<T>,
    {
        match self
            .state
            .compare_exchange(expect, STATE_BUSY, Ordering::Acquire, Ordering::Acquire)
        {
            Ok(_) => {
                let res = f();
                if res.is_ok() {
                    self.set_state(new);
                } else {
                    self.set_state(expect);
                }
                Ok(res)
            }
            Err(old) => Err(old),
        }
    }

    fn is_connected(&self) -> bool {
        self.get_state() == STATE_CONNECTED
    }

    fn is_listening(&self) -> bool {
        self.get_state() == STATE_LISTENING
    }

    fn poll_stream(&self) -> NetResult<PollState> {
        let connection = unsafe {
            match (*self.connection.get()).as_ref() {
                Some(conn) => conn.clone(),
                None => return Err(NetError::ENOTCONN),
            }
        };

        let recv_buf = connection.recv_buf.lock();
        let send_buf = connection.send_buf.lock();
        let peer_closed = connection.peer_closed.load(Ordering::Acquire);

        Ok(PollState {
            readable: !recv_buf.is_empty() || peer_closed,
            writable: send_buf.available_space() > 0 && !peer_closed,
        })
    }

    fn poll_listener(&self) -> NetResult<PollState> {
        let local_addr = unsafe { self.local_addr.get().read() }.clone();
        let listen_table = LISTEN_TABLE.lock();
        let has_pending = listen_table
            .get(&local_addr)
            .map(|queue| !queue.is_empty())
            .unwrap_or(false);

        Ok(PollState {
            readable: has_pending,
            writable: false,
        })
    }

    fn block_on<F, T>(&self, mut f: F) -> NetResult<T>
    where
        F: FnMut() -> NetResult<T>,
    {
        if self.is_nonblocking() {
            f()
        } else {
            loop {
                match f() {
                    Ok(t) => return Ok(t),
                    Err(NetError::EAGAIN) => yield_now(),
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

impl Read for UnixSocket {
    fn read(&mut self, buf: &mut [u8]) -> axerrno::AxResult<usize> {
        self.recv(buf).map_err(net_error_to_axio)
    }
}

impl Write for UnixSocket {
    fn write(&mut self, buf: &[u8]) -> axerrno::AxResult<usize> {
        self.send(buf).map_err(net_error_to_axio)
    }

    fn flush(&mut self) -> axerrno::AxResult {
        Ok(()) // Unix sockets don't need explicit flushing
    }
}

impl Drop for UnixSocket {
    fn drop(&mut self) {
        self.shutdown().ok();
    }
}
