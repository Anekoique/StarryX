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
        TCSETSW, TIOCGPGRP, TIOCGSID, TIOCGWINSZ, TIOCNOTTY, TIOCSCTTY, TIOCSPGRP, TIOCSWINSZ,
    },
    loop_device::{LOOP_CLR_FD, LOOP_GET_STATUS, LOOP_SET_FD, LOOP_SET_STATUS, loop_info},
    net::{
        __kernel_sa_family_t, AF_INET, AF_INET6, AF_UNIX, IP_RECVERR, IPPROTO_AH, IPPROTO_BEETPH,
        IPPROTO_COMP, IPPROTO_DCCP, IPPROTO_EGP, IPPROTO_ENCAP, IPPROTO_ESP, IPPROTO_ETHERNET,
        IPPROTO_GRE, IPPROTO_ICMP, IPPROTO_IDP, IPPROTO_IGMP, IPPROTO_IP, IPPROTO_IPIP,
        IPPROTO_IPV6, IPPROTO_L2TP, IPPROTO_MAX, IPPROTO_MPLS, IPPROTO_MPTCP, IPPROTO_MTP,
        IPPROTO_PIM, IPPROTO_PUP, IPPROTO_RAW, IPPROTO_RSVP, IPPROTO_SCTP, IPPROTO_SMC,
        IPPROTO_TCP, IPPROTO_TP, IPPROTO_UDP, IPPROTO_UDPLITE, MCAST_JOIN_GROUP, MCAST_LEAVE_GROUP,
        SO_DONTROUTE, SO_KEEPALIVE, SO_RCVBUF, SO_RCVTIMEO, SO_REUSEADDR, SO_SNDBUF,
        SO_SNDBUFFORCE, SOCK_DGRAM, SOCK_STREAM, SOL_SOCKET, TCP_CONGESTION, TCP_INFO,
        TCP_KEEPIDLE, TCP_MAXSEG, TCP_NODELAY, in_addr, in6_addr, sockaddr, sockaddr_in,
        sockaddr_in6, socklen_t,
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
pub const L_IGMP: i32 = IPPROTO_IGMP as _;
pub const L_IPIP: i32 = IPPROTO_IPIP as _;
pub const L_EGP: i32 = IPPROTO_EGP as _;
pub const L_PUP: i32 = IPPROTO_PUP as _;
pub const L_IDP: i32 = IPPROTO_IDP as _;
pub const L_TP: i32 = IPPROTO_TP as _;
pub const L_DCCP: i32 = IPPROTO_DCCP as _;
pub const L_IPV6: i32 = IPPROTO_IPV6 as _;
pub const L_RSVP: i32 = IPPROTO_RSVP as _;
pub const L_GRE: i32 = IPPROTO_GRE as _;
pub const L_ESP: i32 = IPPROTO_ESP as _;
pub const L_AH: i32 = IPPROTO_AH as _;
pub const L_MTP: i32 = IPPROTO_MTP as _;
pub const L_BEETPH: i32 = IPPROTO_BEETPH as _;
pub const L_ENCAP: i32 = IPPROTO_ENCAP as _;
pub const L_PIM: i32 = IPPROTO_PIM as _;
pub const L_COMP: i32 = IPPROTO_COMP as _;
pub const L_L2TP: i32 = IPPROTO_L2TP as _;
pub const L_SCTP: i32 = IPPROTO_SCTP as _;
pub const L_UDPLITE: i32 = IPPROTO_UDPLITE as _;
pub const L_MPLS: i32 = IPPROTO_MPLS as _;
pub const L_ETHERNET: i32 = IPPROTO_ETHERNET as _;
pub const L_RAW: i32 = IPPROTO_RAW as _;
pub const L_SMC: i32 = IPPROTO_SMC as _;
pub const L_MPTCP: i32 = IPPROTO_MPTCP as _;
pub const L_MAX: i32 = IPPROTO_MAX as _;

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

// fanotify
pub const FAN_ACCESS: u64 = 0x0000_0001;
pub const FAN_MODIFY: u64 = 0x0000_0002;
pub const FAN_ATTRIB: u64 = 0x0000_0004;
pub const FAN_CLOSE_WRITE: u64 = 0x0000_0008;
pub const FAN_CLOSE_NOWRITE: u64 = 0x0000_0010;
pub const FAN_OPEN: u64 = 0x0000_0020;
pub const FAN_MOVED_FROM: u64 = 0x0000_0040;
pub const FAN_MOVED_TO: u64 = 0x0000_0080;
pub const FAN_CREATE: u64 = 0x0000_0100;
pub const FAN_DELETE: u64 = 0x0000_0200;
pub const FAN_DELETE_SELF: u64 = 0x0000_0400;
pub const FAN_MOVE_SELF: u64 = 0x0000_0800;
pub const FAN_OPEN_EXEC: u64 = 0x0000_1000;

pub const FAN_Q_OVERFLOW: u64 = 0x0000_4000;
pub const FAN_FS_ERROR: u64 = 0x0000_8000;

pub const FAN_OPEN_PERM: u64 = 0x0001_0000;
pub const FAN_ACCESS_PERM: u64 = 0x0002_0000;
pub const FAN_OPEN_EXEC_PERM: u64 = 0x0004_0000;

pub const FAN_EVENT_ON_CHILD: u64 = 0x0800_0000;

pub const FAN_RENAME: u64 = 0x1000_0000;

pub const FAN_ONDIR: u64 = 0x4000_0000;

pub const FAN_CLOSE: u64 = FAN_CLOSE_WRITE | FAN_CLOSE_NOWRITE;
pub const FAN_MOVE: u64 = FAN_MOVED_FROM | FAN_MOVED_TO;

pub const FAN_CLOEXEC: c_uint = 0x0000_0001;
pub const FAN_NONBLOCK: c_uint = 0x0000_0002;

pub const FAN_CLASS_NOTIF: c_uint = 0x0000_0000;
pub const FAN_CLASS_CONTENT: c_uint = 0x0000_0004;
pub const FAN_CLASS_PRE_CONTENT: c_uint = 0x0000_0008;

pub const FAN_UNLIMITED_QUEUE: c_uint = 0x0000_0010;
pub const FAN_UNLIMITED_MARKS: c_uint = 0x0000_0020;
pub const FAN_ENABLE_AUDIT: c_uint = 0x0000_0040;

pub const FAN_REPORT_PIDFD: c_uint = 0x0000_0080;
pub const FAN_REPORT_TID: c_uint = 0x0000_0100;
pub const FAN_REPORT_FID: c_uint = 0x0000_0200;
pub const FAN_REPORT_DIR_FID: c_uint = 0x0000_0400;
pub const FAN_REPORT_NAME: c_uint = 0x0000_0800;
pub const FAN_REPORT_TARGET_FID: c_uint = 0x0000_1000;
pub const FAN_REPORT_FD_ERROR: c_uint = 0x0000_2000;

pub const FAN_REPORT_DFID_NAME: c_uint = FAN_REPORT_DIR_FID | FAN_REPORT_NAME;
pub const FAN_REPORT_DFID_NAME_TARGET: c_uint =
    FAN_REPORT_DFID_NAME | FAN_REPORT_FID | FAN_REPORT_TARGET_FID;

pub const FAN_MARK_ADD: c_uint = 0x0000_0001;
pub const FAN_MARK_REMOVE: c_uint = 0x0000_0002;
pub const FAN_MARK_DONT_FOLLOW: c_uint = 0x0000_0004;
pub const FAN_MARK_ONLYDIR: c_uint = 0x0000_0008;
pub const FAN_MARK_IGNORED_MASK: c_uint = 0x0000_0020;
pub const FAN_MARK_IGNORED_SURV_MODIFY: c_uint = 0x0000_0040;
pub const FAN_MARK_FLUSH: c_uint = 0x0000_0080;
pub const FAN_MARK_EVICTABLE: c_uint = 0x0000_0200;
pub const FAN_MARK_IGNORE: c_uint = 0x0000_0400;

pub const FAN_MARK_INODE: c_uint = 0x0000_0000;
pub const FAN_MARK_MOUNT: c_uint = 0x0000_0010;
pub const FAN_MARK_FILESYSTEM: c_uint = 0x0000_0100;

pub const FAN_MARK_IGNORE_SURV: c_uint = FAN_MARK_IGNORE | FAN_MARK_IGNORED_SURV_MODIFY;

pub const FANOTIFY_METADATA_VERSION: u8 = 3;

pub const FAN_EVENT_INFO_TYPE_FID: u8 = 1;
pub const FAN_EVENT_INFO_TYPE_DFID_NAME: u8 = 2;
pub const FAN_EVENT_INFO_TYPE_DFID: u8 = 3;
pub const FAN_EVENT_INFO_TYPE_PIDFD: u8 = 4;
pub const FAN_EVENT_INFO_TYPE_ERROR: u8 = 5;

pub const FAN_EVENT_INFO_TYPE_OLD_DFID_NAME: u8 = 10;
pub const FAN_EVENT_INFO_TYPE_NEW_DFID_NAME: u8 = 12;

pub const FAN_RESPONSE_INFO_NONE: u8 = 0;
pub const FAN_RESPONSE_INFO_AUDIT_RULE: u8 = 1;

pub const FAN_ALLOW: u32 = 0x01;
pub const FAN_DENY: u32 = 0x02;
pub const FAN_AUDIT: u32 = 0x10;
pub const FAN_INFO: u32 = 0x20;

pub const FAN_NOFD: c_int = -1;
pub const FAN_NOPIDFD: c_int = FAN_NOFD;
pub const FAN_EPIDFD: c_int = -2;

// pidfd
pub const PIDFD_NONBLOCK: u32 = O_NONBLOCK;
