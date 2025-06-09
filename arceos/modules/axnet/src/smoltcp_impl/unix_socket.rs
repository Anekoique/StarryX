use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use spin::Mutex;

use axerrno::{AxError, AxResult, ax_err};
use axio::{PollState, Read, Write};
use axtask::yield_now;

// Unix Socket 地址类型
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

// Unix Socket 状态
const STATE_CLOSED: u8 = 0;
const STATE_BUSY: u8 = 1;
const STATE_CONNECTING: u8 = 2;
const STATE_CONNECTED: u8 = 3;
const STATE_LISTENING: u8 = 4;

// 消息缓冲区
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

    fn write(&mut self, buf: &[u8]) -> AxResult<usize> {
        if self.data.len() + buf.len() > self.max_size {
            return ax_err!(WouldBlock, "buffer full");
        }

        for &byte in buf {
            self.data.push_back(byte);
        }
        Ok(buf.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> usize {
        let to_read = buf.len().min(self.data.len());
        for i in 0..to_read {
            buf[i] = self.data.pop_front().unwrap();
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

// 连接对，用于已连接的 socket
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

// 全局监听表 - 使用 BTreeMap 替代 HashMap
type ListenTable = Arc<Mutex<BTreeMap<UnixAddr, VecDeque<UnixSocket>>>>;
static LISTEN_TABLE: spin::Lazy<ListenTable> =
    spin::Lazy::new(|| Arc::new(Mutex::new(BTreeMap::new())));

/// Unix Domain Socket 实现
///
/// 支持 SOCK_STREAM 类型的 Unix Socket，提供类似 POSIX 的 API：
/// - `connect` 用于客户端连接
/// - `bind`, `listen`, `accept` 用于服务端
/// - `send`, `recv` 用于数据传输
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
    /// 创建新的 Unix Socket
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_CLOSED),
            local_addr: UnsafeCell::new(UnixAddr::Unnamed),
            peer_addr: UnsafeCell::new(UnixAddr::Unnamed),
            connection: UnsafeCell::new(None),
            nonblock: AtomicBool::new(false),
            buffer_size: 8192, // 默认缓冲区大小
        }
    }

    /// 创建 socket pair
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

    /// 创建已连接的 Unix Socket
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

    /// 获取本地地址
    pub fn local_addr(&self) -> AxResult<UnixAddr> {
        match self.get_state() {
            STATE_CONNECTED | STATE_LISTENING => {
                Ok(unsafe { self.local_addr.get().read() }.clone())
            }
            _ => ax_err!(NotConnected, "socket not bound or connected"),
        }
    }

    /// 获取对端地址
    pub fn peer_addr(&self) -> AxResult<UnixAddr> {
        match self.get_state() {
            STATE_CONNECTED => Ok(unsafe { self.peer_addr.get().read() }.clone()),
            _ => ax_err!(NotConnected, "socket not connected"),
        }
    }

    /// 检查是否为非阻塞模式
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// 设置非阻塞模式
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    /// 设置缓冲区大小
    pub fn set_buffer_size(&mut self, size: usize) {
        self.buffer_size = size;
    }

    /// 连接到指定地址
    pub fn connect(&self, addr: UnixAddr) -> AxResult {
        self.update_state(STATE_CLOSED, STATE_CONNECTING, || {
            // 检查目标地址是否在监听
            let mut listen_table = LISTEN_TABLE.lock();
            let mut listeners = listen_table.get_mut(&addr);

            if listeners.is_none() || listeners.as_ref().unwrap().is_empty() {
                return ax_err!(ConnectionRefused, "no listener on target address");
            }

            // 创建连接对
            let (client_conn, server_conn) = ConnectionPair::new(self.buffer_size);

            // 创建服务端 socket
            let server_addr = addr.clone();
            let client_addr = UnixAddr::Unnamed; // 客户端通常使用未命名地址
            let server_socket =
                UnixSocket::new_connected(server_addr, client_addr.clone(), server_conn);

            // 将服务端 socket 加入接受队列
            listeners.as_mut().unwrap().push_back(server_socket);

            // 设置客户端连接信息
            unsafe {
                self.peer_addr.get().write(addr);
                self.local_addr.get().write(client_addr);
                self.connection.get().write(Some(client_conn));
            }

            Ok(())
        })
        .unwrap_or_else(|_| ax_err!(AlreadyExists, "socket already connected"))?;

        self.set_state(STATE_CONNECTED);
        Ok(())
    }

    /// 绑定到指定地址
    pub fn bind(&self, addr: UnixAddr) -> AxResult {
        self.update_state(STATE_CLOSED, STATE_CLOSED, || {
            // 检查地址是否已被使用
            if matches!(addr, UnixAddr::Pathname(_)) {
                let listen_table = LISTEN_TABLE.lock();
                if listen_table.contains_key(&addr) {
                    return ax_err!(AddrInUse, "address already in use");
                }
            }

            unsafe {
                let old_addr = self.local_addr.get().read();
                if !old_addr.is_unnamed() {
                    return ax_err!(InvalidInput, "socket already bound");
                }
                self.local_addr.get().write(addr);
            }
            Ok(())
        })
        .unwrap_or_else(|_| ax_err!(InvalidInput, "socket already bound"))
    }

    /// 开始监听
    pub fn listen(&self) -> AxResult {
        self.update_state(STATE_CLOSED, STATE_LISTENING, || {
            let local_addr = unsafe { self.local_addr.get().read() }.clone();
            if local_addr.is_unnamed() {
                return ax_err!(InvalidInput, "socket not bound");
            }

            let mut listen_table = LISTEN_TABLE.lock();
            listen_table.insert(local_addr, VecDeque::new());
            Ok(())
        })
        .unwrap_or(Ok(())) // 忽略重复监听
    }

    /// 接受连接
    pub fn accept(&self) -> AxResult<UnixSocket> {
        if !self.is_listening() {
            return ax_err!(InvalidInput, "socket not listening");
        }

        let local_addr = unsafe { self.local_addr.get().read() }.clone();

        self.block_on(|| {
            let mut listen_table = LISTEN_TABLE.lock();
            match listen_table.get_mut(&local_addr) {
                Some(listeners) => {
                    if let Some(client_socket) = listeners.pop_front() {
                        Ok(client_socket)
                    } else {
                        Err(AxError::WouldBlock)
                    }
                }
                None => ax_err!(InvalidInput, "socket not listening"),
            }
        })
    }

    /// 发送数据
    pub fn send(&self, buf: &[u8]) -> AxResult<usize> {
        if !self.is_connected() {
            return ax_err!(NotConnected, "socket not connected");
        }

        let connection = unsafe {
            match (*self.connection.get()).as_ref() {
                Some(conn) => conn.clone(),
                None => return ax_err!(NotConnected, "no connection"),
            }
        };

        if connection.peer_closed.load(Ordering::Acquire) {
            return ax_err!(ConnectionReset, "peer closed connection");
        }

        self.block_on(|| {
            let mut send_buf = connection.send_buf.lock();
            if send_buf.available_space() == 0 {
                Err(AxError::WouldBlock)
            } else {
                send_buf.write(buf)
            }
        })
    }

    /// 接收数据
    pub fn recv(&self, buf: &mut [u8]) -> AxResult<usize> {
        if !self.is_connected() {
            return ax_err!(NotConnected, "socket not connected");
        }

        let connection = unsafe {
            match (*self.connection.get()).as_ref() {
                Some(conn) => conn.clone(),
                None => return ax_err!(NotConnected, "no connection"),
            }
        };

        self.block_on(|| {
            let mut recv_buf = connection.recv_buf.lock();
            if recv_buf.is_empty() {
                if connection.peer_closed.load(Ordering::Acquire) {
                    Ok(0) // EOF
                } else {
                    Err(AxError::WouldBlock)
                }
            } else {
                Ok(recv_buf.read(buf))
            }
        })
    }

    /// 关闭 socket
    pub fn shutdown(&self) -> AxResult {
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

    /// 轮询 socket 状态
    pub fn poll(&self) -> AxResult<PollState> {
        match self.get_state() {
            STATE_CONNECTED => self.poll_stream(),
            STATE_LISTENING => self.poll_listener(),
            _ => Ok(PollState {
                readable: false,
                writable: false,
            }),
        }
    }

    // 私有方法
    fn get_state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    fn set_state(&self, state: u8) {
        self.state.store(state, Ordering::Release);
    }

    fn update_state<F, T>(&self, expect: u8, new: u8, f: F) -> Result<AxResult<T>, u8>
    where
        F: FnOnce() -> AxResult<T>,
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

    fn poll_stream(&self) -> AxResult<PollState> {
        let connection = unsafe {
            match (*self.connection.get()).as_ref() {
                Some(conn) => conn.clone(),
                None => return ax_err!(NotConnected, "no connection"),
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

    fn poll_listener(&self) -> AxResult<PollState> {
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

    fn block_on<F, T>(&self, mut f: F) -> AxResult<T>
    where
        F: FnMut() -> AxResult<T>,
    {
        if self.is_nonblocking() {
            f()
        } else {
            loop {
                match f() {
                    Ok(t) => return Ok(t),
                    Err(AxError::WouldBlock) => yield_now(),
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

impl Read for UnixSocket {
    fn read(&mut self, buf: &mut [u8]) -> AxResult<usize> {
        self.recv(buf)
    }
}

impl Write for UnixSocket {
    fn write(&mut self, buf: &[u8]) -> AxResult<usize> {
        self.send(buf)
    }

    fn flush(&mut self) -> AxResult {
        Ok(()) // Unix sockets don't need explicit flushing
    }
}

impl Drop for UnixSocket {
    fn drop(&mut self) {
        self.shutdown().ok();
    }
}
