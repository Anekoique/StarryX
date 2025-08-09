pub mod fs;
pub mod ipc;
pub mod mm;
// pub mod net;
pub mod sys;
pub mod task;

pub use linux_raw_sys::{
    ctypes::*,
    general::*,
    ioctl::RTC_RD_TIME,
    ioctl::{
        BLKGETSIZE, BLKGETSIZE64, BLKRAGET, BLKRASET, BLKROGET, BLKROSET, TCGETS, TCSETS, TCSETSF,
        TCSETSW, TIOCGWINSZ, TIOCSWINSZ,
    },
    loop_device::{LOOP_CLR_FD, LOOP_GET_STATUS, LOOP_SET_FD, LOOP_SET_STATUS},
    net::{
        __kernel_sa_family_t, AF_INET, AF_INET6, AF_UNIX, IP_RECVERR, IPPROTO_ICMP, IPPROTO_IP,
        IPPROTO_TCP, IPPROTO_UDP, IPPROTO_UDPLITE, MCAST_JOIN_GROUP, MCAST_LEAVE_GROUP,
        SO_DONTROUTE, SO_KEEPALIVE, SO_RCVBUF, SO_RCVTIMEO, SO_REUSEADDR, SO_SNDBUF,
        SO_SNDBUFFORCE, SOCK_DGRAM, SOCK_STREAM, SOL_SOCKET, TCP_CONGESTION, TCP_INFO, TCP_MAXSEG,
        TCP_NODELAY, in_addr, in6_addr, sockaddr, sockaddr_in, sockaddr_in6, socklen_t,
    },
    select_macros::*,
    system::{new_utsname, sysinfo},
};

// net
pub const SOCK_CLOEXEC: u32 = O_CLOEXEC;
pub const SOCK_NONBLOCK: u32 = O_NONBLOCK;
pub const L_SOCKET: i32 = SOL_SOCKET as _;
pub const L_IP: i32 = IPPROTO_IP as _;
pub const L_TCP: i32 = IPPROTO_TCP as _;
pub const L_UDP: i32 = IPPROTO_UDP as _;
pub const L_ICMP: i32 = IPPROTO_ICMP as _;

// fs
pub const O_EXEC: u32 = O_PATH;

// ipc
pub const IPC_PRIVATE: i32 = 0;

pub const IPC_CREAT: u32 = 0o1000;
pub const IPC_EXCL: u32 = 0o2000;
pub const IPC_NOWAIT: u32 = 0o4000;

pub const IPC_RMID: u32 = 0;
pub const IPC_SET: u32 = 1;
pub const IPC_STAT: u32 = 2;
pub const IPC_INFO: u32 = 3;

// shm
pub const SHMMIN: usize = 1;
pub const SHMMNI: usize = 4096;
pub const SHMMAX: usize = usize::MAX - (1 << 24);
pub const SHMALL: usize = usize::MAX - (1 << 24);
pub const SHMSEG: usize = SHMMNI;

// msg
pub const MSGMAX: usize = 8192;
pub const MSGMNB: usize = 16384;
pub const MSGMNI: usize = 32000;
pub const MSGTQL: usize = 1024;
pub const MSGPOOL: usize = MSGMNI * MSGMNB;

// sem
pub const SEMMSL: usize = 250;
pub const SEMMNS: usize = 32000;
pub const SEMOPM: usize = 32;
pub const SEMMNI: usize = 128;
pub const SEMVMX: usize = 32767;

// eventfd
pub const EFD_CLOEXEC: u32 = O_CLOEXEC;
pub const EFD_NONBLOCK: u32 = O_NONBLOCK;
pub const EFD_SEMAPHORE: u32 = 0o1;
