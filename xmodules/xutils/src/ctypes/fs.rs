use axfs_ng::OpenOptions;
use axfs_ng_vfs::{DeviceId, Metadata};
use axhal::time::TimeValue;

use super::{
    __kernel_mode_t, F_OK, O_APPEND, O_CREAT, O_DIRECT, O_DIRECTORY, O_EXCL, O_EXEC, O_PATH,
    O_RDONLY, O_TRUNC, O_WRONLY, POLLERR, POLLHUP, POLLIN, POLLNVAL, POLLOUT, POLLPRI, R_OK, W_OK,
    X_OK, c_int, stat, statx, statx_timestamp,
};

/// Convert open flags to [`OpenOptions`].
pub fn flags_to_options(
    flags: c_int,
    mode: __kernel_mode_t,
    (uid, gid): (u32, u32),
) -> OpenOptions {
    let flags = flags as u32;
    let mut options = OpenOptions::new();
    options.mode(mode).user(uid, gid);
    match flags & 0b11 {
        O_RDONLY => options.read(true),
        O_WRONLY => options.write(true),
        _ => options.read(true).write(true),
    };
    if flags & O_APPEND != 0 {
        options.append(true);
    }
    if flags & O_TRUNC != 0 {
        options.truncate(true);
    }
    if flags & O_CREAT != 0 {
        options.create(true);
    }
    if flags & O_EXEC != 0 {
        options.execute(true);
    }
    if flags & O_EXCL != 0 {
        options.create_new(true);
    }
    if flags & O_DIRECTORY != 0 {
        options.directory(true);
    }
    if flags & O_DIRECT != 0 {
        options.direct(true);
    }
    if flags & O_PATH != 0 {
        options.path(true);
    }
    options
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    #[repr(C)]
    pub struct IoEvents: i16 {
        const IN    = POLLIN as i16;
        const PRI   = POLLPRI as i16;
        const OUT   = POLLOUT as i16;
        const ERR   = POLLERR as i16;
        const HUP   = POLLHUP as i16;
        const NVAL  = POLLNVAL as i16;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Kstat {
    pub dev: u64,
    pub ino: u64,
    pub nlink: u32,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub blksize: u32,
    pub blocks: u64,
    pub rdev: DeviceId,
    pub atime: TimeValue,
    pub mtime: TimeValue,
    pub ctime: TimeValue,
}

impl Default for Kstat {
    fn default() -> Self {
        Self {
            dev: 0,
            ino: 1,
            nlink: 1,
            mode: 0,
            uid: 1,
            gid: 1,
            size: 0,
            blksize: 4096,
            blocks: 0,
            rdev: DeviceId::default(),
            atime: TimeValue::default(),
            mtime: TimeValue::default(),
            ctime: TimeValue::default(),
        }
    }
}

pub fn metadata_to_kstat(metadata: &Metadata) -> Kstat {
    let ty = metadata.node_type as u8;
    let perm = metadata.mode.bits() as u32;
    let mode = ((ty as u32) << 12) | perm;
    Kstat {
        dev: metadata.device,
        ino: metadata.inode,
        mode,
        nlink: metadata.nlink as _,
        uid: metadata.uid,
        gid: metadata.gid,
        size: metadata.size,
        blksize: metadata.block_size as _,
        blocks: metadata.blocks,
        rdev: metadata.rdev,
        atime: metadata.atime,
        mtime: metadata.mtime,
        ctime: metadata.ctime,
    }
}

impl From<Kstat> for stat {
    fn from(value: Kstat) -> Self {
        // SAFETY: valid for stat
        let mut stat: stat = unsafe { core::mem::zeroed() };
        stat.st_dev = value.dev as _;
        stat.st_ino = value.ino as _;
        stat.st_nlink = value.nlink as _;
        stat.st_mode = value.mode as _;
        stat.st_uid = value.uid as _;
        stat.st_gid = value.gid as _;
        stat.st_size = value.size as _;
        stat.st_blksize = value.blksize as _;
        stat.st_blocks = value.blocks as _;
        stat.st_rdev = value.rdev.0 as _;

        stat.st_atime = value.atime.as_secs() as _;
        stat.st_atime_nsec = value.atime.subsec_nanos() as _;
        stat.st_mtime = value.mtime.as_secs() as _;
        stat.st_mtime_nsec = value.mtime.subsec_nanos() as _;
        stat.st_ctime = value.ctime.as_secs() as _;
        stat.st_ctime_nsec = value.ctime.subsec_nanos() as _;

        stat
    }
}

impl From<Kstat> for statx {
    fn from(value: Kstat) -> Self {
        // SAFETY: valid for statx
        let mut statx: statx = unsafe { core::mem::zeroed() };
        statx.stx_blksize = value.blksize as _;
        statx.stx_attributes = value.mode as _;
        statx.stx_nlink = value.nlink as _;
        statx.stx_uid = value.uid as _;
        statx.stx_gid = value.gid as _;
        statx.stx_mode = value.mode as _;
        statx.stx_ino = value.ino as _;
        statx.stx_size = value.size as _;
        statx.stx_blocks = value.blocks as _;
        statx.stx_rdev_major = value.rdev.major();
        statx.stx_rdev_minor = value.rdev.minor();

        fn time_to_statx(time: &TimeValue) -> statx_timestamp {
            statx_timestamp {
                tv_sec: time.as_secs() as _,
                tv_nsec: time.subsec_nanos() as _,
                __reserved: 0,
            }
        }
        statx.stx_atime = time_to_statx(&value.atime);
        statx.stx_ctime = time_to_statx(&value.ctime);
        statx.stx_mtime = time_to_statx(&value.mtime);

        statx.stx_dev_major = (value.dev >> 32) as _;
        statx.stx_dev_minor = value.dev as _;

        statx
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FanotifyEventMetadata {
    pub event_len: u32,
    pub vers: u8,
    pub reserved: u8,
    pub metadata_len: u16,
    pub mask: u64,
    pub fd: i32,
    pub pid: i32,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    #[repr(transparent)]
    pub struct AccessMode: u32 {
        const F_OK = F_OK;
        const R_OK = R_OK;
        const W_OK = W_OK;
        const X_OK = X_OK;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FanInitFlags: u32 {
        const CLASS_PRE_CONTENT = super::FAN_CLASS_PRE_CONTENT;
        const CLASS_CONTENT = super::FAN_CLASS_CONTENT;
        const CLASS_NOTIF = super::FAN_CLASS_NOTIF;

        const CLOEXEC = super::FAN_CLOEXEC;
        const NONBLOCK = super::FAN_NONBLOCK;

        const UNLIMITED_QUEUE = super::FAN_UNLIMITED_QUEUE;
        const UNLIMITED_MARKS = super::FAN_UNLIMITED_MARKS;

        const REPORT_TID = super::FAN_REPORT_TID;
        const ENABLE_AUDIT = super::FAN_ENABLE_AUDIT;
        const REPORT_FID = super::FAN_REPORT_FID;
        const REPORT_DIR_FID = super::FAN_REPORT_DIR_FID;
        const REPORT_NAME = super::FAN_REPORT_NAME;
        const REPORT_DFID_NAME = super::FAN_REPORT_DFID_NAME;
        const REPORT_TARGET_FID = super::FAN_REPORT_TARGET_FID;
        const REPORT_DFID_NAME_TARGET = super::FAN_REPORT_DFID_NAME_TARGET;
        const REPORT_PIDFD = super::FAN_REPORT_PIDFD;
        const REPORT_FD_ERROR = super::FAN_REPORT_FD_ERROR;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FanInitEventFlags: u32 {
        const RDONLY = super::O_RDONLY;
        const WRONLY = super::O_WRONLY;
        const RDWR = super::O_RDWR;
        const LARGEFILE = super::O_LARGEFILE;
        const CLOEXEC = super::O_CLOEXEC;
        const APPEND = super::O_APPEND;
        const DSYNC = super::O_DSYNC;
        const NOATIME = super::O_NOATIME;
        const NONBLOCK = super::O_NONBLOCK;
        const SYNC = super::O_SYNC;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FanMarkFlags: u32 {
        const ADD = super::FAN_MARK_ADD;
        const REMOVE = super::FAN_MARK_REMOVE;
        const FLUSH = super::FAN_MARK_FLUSH;
        const DONT_FOLLOW = super::FAN_MARK_DONT_FOLLOW;
        const ONLYDIR = super::FAN_MARK_ONLYDIR;
        const MOUNT = super::FAN_MARK_MOUNT;
        const FILESYSTEM = super::FAN_MARK_FILESYSTEM;
        const IGNORED_MASK = super::FAN_MARK_IGNORED_MASK;
        const IGNORE = super::FAN_MARK_IGNORE;
        const IGNORED_SURV_MODIFY = super::FAN_MARK_IGNORED_SURV_MODIFY;
        const EVICTABLE = super::FAN_MARK_EVICTABLE;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FanEventMask: u64 {
        const ACCESS = super::FAN_ACCESS;
        const MODIFY = super::FAN_MODIFY;
        const CLOSE_WRITE = super::FAN_CLOSE_WRITE;
        const CLOSE_NOWRITE = super::FAN_CLOSE_NOWRITE;
        const OPEN = super::FAN_OPEN;
        const OPEN_EXEC = super::FAN_OPEN_EXEC;
        const ATTRIB = super::FAN_ATTRIB;
        const CREATE = super::FAN_CREATE;
        const DELETE = super::FAN_DELETE;
        const DELETE_SELF = super::FAN_DELETE_SELF;
        const FS_ERROR = super::FAN_FS_ERROR;
        const MOVED_FROM = super::FAN_MOVED_FROM;
        const MOVED_TO = super::FAN_MOVED_TO;
        const RENAME = super::FAN_RENAME;
        const MOVE_SELF = super::FAN_MOVE_SELF;
        const ACCESS_PERM = super::FAN_ACCESS_PERM;
        const OPEN_PERM = super::FAN_OPEN_PERM;
        const OPEN_EXEC_PERM = super::FAN_OPEN_EXEC_PERM;
        const CLOSE = super::FAN_CLOSE;
        const MOVE = super::FAN_MOVE;
        const ONDIR = super::FAN_ONDIR;
        const EVENT_ON_CHILD = super::FAN_EVENT_ON_CHILD;
        const Q_OVERFLOW = super::FAN_Q_OVERFLOW;

        const FILE_EVENT_MASK =
            Self::ACCESS.bits()
          | Self::MODIFY.bits()
          | Self::CLOSE_WRITE.bits()
          | Self::CLOSE_NOWRITE.bits()
          | Self::OPEN.bits()
          | Self::OPEN_EXEC.bits()
          | Self::ATTRIB.bits()
          | Self::DELETE_SELF.bits()
          | Self::FS_ERROR.bits()
          | Self::MOVE_SELF.bits()
          | Self::ACCESS_PERM.bits()
          | Self::OPEN_PERM.bits()
          | Self::OPEN_EXEC_PERM.bits();

        const DIR_EVENT_MASK =
            Self::CREATE.bits()
          | Self::DELETE.bits()
          | Self::MOVED_FROM.bits()
          | Self::MOVED_TO.bits()
          | Self::RENAME.bits();

        const FID_EVENT_MASK =
            Self::ATTRIB.bits()
          | Self::CREATE.bits()
          | Self::DELETE.bits()
          | Self::DELETE_SELF.bits()
          | Self::FS_ERROR.bits()
          | Self::MOVED_FROM.bits()
          | Self::MOVED_TO.bits()
          | Self::RENAME.bits()
          | Self::MOVE_SELF.bits();
    }
}
