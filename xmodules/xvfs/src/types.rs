use core::{fmt::Debug, time::Duration};

/// Filesystem node type.
///
/// Represents the different types of filesystem nodes that can exist.
/// The numeric values correspond to UNIX file type constants.
#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NodeType {
    /// Unknown or unspecified file type
    Unknown = 0,
    /// Named pipe (FIFO)
    Fifo = 0o1,
    /// Character device
    CharacterDevice = 0o2,
    /// Directory
    Directory = 0o4,
    /// Block device
    BlockDevice = 0o6,
    /// Regular file
    RegularFile = 0o10,
    /// Symbolic link
    Symlink = 0o12,
    /// Socket
    Socket = 0o14,
}
impl From<u8> for NodeType {
    fn from(value: u8) -> Self {
        match value {
            0o1 => Self::Fifo,
            0o2 => Self::CharacterDevice,
            0o4 => Self::Directory,
            0o6 => Self::BlockDevice,
            0o10 => Self::RegularFile,
            0o12 => Self::Symlink,
            0o14 => Self::Socket,
            _ => Self::Unknown,
        }
    }
}

bitflags::bitflags! {
    /// Inode permission mode.
    ///
    /// Represents UNIX-style file permissions using octal notation.
    /// Permissions are organized into owner, group, and other categories.
    #[derive(Debug, Clone, Copy)]
    pub struct NodePermission: u16 {
        /// Socket.
        const SOCKET = 0o140000;
        /// Whiteout.
        const SYMLINK = 0o120000;
        /// Regular file.
        const REGULAR_FILE = 0o10000;
        /// Block device.
        const BLOCK_DEVICE = 0o60000;
        /// Directory.
        const DIRECTORY = 0o40000;
        /// Character device.
        const CHARACTER_DEVICE = 0o20000;
        /// FIFO.
        const FIFO = 0o10000;

        /// Set user ID on execution.
        const SET_UID = 0o4000;
        /// Set group ID on execution.
        const SET_GID = 0o2000;
        /// Sticky bit.
        const STICKY = 0o1000;

        /// Owner has read permission.
        const OWNER_READ = 0o400;
        /// Owner has write permission.
        const OWNER_WRITE = 0o200;
        /// Owner has execute permission.
        const OWNER_EXEC = 0o100;

        /// Group has read permission.
        const GROUP_READ = 0o40;
        /// Group has write permission.
        const GROUP_WRITE = 0o20;
        /// Group has execute permission.
        const GROUP_EXEC = 0o10;

        /// Others have read permission.
        const OTHER_READ = 0o4;
        /// Others have write permission.
        const OTHER_WRITE = 0o2;
        /// Others have execute permission.
        const OTHER_EXEC = 0o1;
    }
}
impl Default for NodePermission {
    fn default() -> Self {
        Self::from_bits_truncate(0o666)
    }
}

/// Filesystem node metadata.
///
/// Contains all the metadata information associated with a filesystem node,
/// including permissions, ownership, size, and timestamps.
#[derive(Clone, Debug)]
pub struct Metadata {
    /// ID of device containing file
    pub device: u64,
    /// Inode number
    pub inode: u64,
    /// Number of hard links
    pub nlink: u64,
    /// Permission mode
    pub mode: NodePermission,
    /// Node type
    pub node_type: NodeType,
    /// User ID of owner
    pub uid: u32,
    /// Group ID of owner
    pub gid: u32,
    /// Total size in bytes
    pub size: u64,
    /// Block size for filesystem I/O
    pub block_size: u64,
    /// Number of 512B blocks allocated
    pub blocks: u64,
    /// Device ID (if special file)
    pub rdev: DeviceId,

    /// Time of last access
    pub atime: Duration,
    /// Time of last modification
    pub mtime: Duration,
    /// Time of last status change
    pub ctime: Duration,
}

/// Filesystem node metadata update.
///
/// Used to specify which metadata fields should be updated.
/// Only the fields that are `Some` will be modified.
#[derive(Default, Clone, Debug)]
pub struct MetadataUpdate {
    /// Permission mode
    pub mode: Option<NodePermission>,
    /// The owner (uid, gid)
    pub owner: Option<(u32, u32)>,

    /// Time of last access
    pub atime: Option<Duration>,
    /// Time of last modification
    pub mtime: Option<Duration>,
}

/// Device Id
///
/// Represents a device identifier using major and minor numbers,
/// encoded according to UNIX conventions.
#[derive(Default, Clone, PartialEq, Eq, Copy)]
pub struct DeviceId(pub u64);

impl DeviceId {
    /// Creates a new device ID from major and minor numbers
    pub const fn new(major: u32, minor: u32) -> Self {
        let major = major as u64;
        let minor = minor as u64;
        Self(
            ((major & 0xffff_f000) << 32)
                | ((major & 0x0000_0fff) << 8)
                | ((minor & 0xffff_ff00) << 12)
                | (minor & 0x0000_00ff),
        )
    }

    /// Extracts the major number from the device ID
    pub const fn major(&self) -> u32 {
        ((self.0 >> 32) & 0xffff_f000 | (self.0 >> 8) & 0x0000_0fff) as u32
    }

    /// Extracts the minor number from the device ID
    pub const fn minor(&self) -> u32 {
        ((self.0 >> 12) & 0xffff_ff00 | self.0 & 0x0000_00ff) as u32
    }
}

impl Debug for DeviceId {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("DeviceId")
            .field("major", &self.major())
            .field("minor", &self.minor())
            .finish()
    }
}
