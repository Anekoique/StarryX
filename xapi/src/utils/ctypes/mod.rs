pub mod fs;
pub mod ipc;
pub mod mm;
pub mod net;
pub mod sys;
pub mod task;

pub use linux_raw_sys::ctypes::*;
pub use linux_raw_sys::general::*;
pub use linux_raw_sys::ioctl::RTC_RD_TIME;
pub use linux_raw_sys::net::{
    __kernel_sa_family_t, AF_INET, AF_INET6, AF_UNIX, IPPROTO_TCP, IPPROTO_UDP, SOCK_DGRAM,
    SOCK_STREAM, in_addr, in6_addr, sockaddr, sockaddr_in, sockaddr_in6, socklen_t,
};
pub use linux_raw_sys::system::{new_utsname, sysinfo};

// net
pub const SOCK_CLOEXEC: u32 = O_CLOEXEC;
pub const SOCK_NONBLOCK: u32 = O_NONBLOCK;

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
