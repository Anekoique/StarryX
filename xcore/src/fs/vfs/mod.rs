pub mod dev;
pub mod etc;
pub mod proc;
pub mod tmp;
pub mod virt_file;
pub mod virt_fs;

pub use virt_file::*;
pub use virt_fs::*;

use axerrno::LinuxResult;
use axfs_ng::FS_CONTEXT;
use axfs_ng_vfs::{Filesystem, NodePermission};
use axsync::RawMutex;

/// Initialize a virtual filesystem at the given path
fn mount_fs(path: &str, fs: Filesystem<RawMutex>, permissions: NodePermission) -> LinuxResult<()> {
    let root = FS_CONTEXT.lock();
    root.create_dir(path, permissions)?;
    root.resolve(path)?.mount(&fs)?;
    Ok(())
}

/// Initialize all virtual filesystems
pub fn init_root() -> LinuxResult<()> {
    mount_fs(
        "/dev",
        self::dev::init_devfs()?,
        NodePermission::from_bits_truncate(0o755),
    )?;
    mount_fs(
        "/tmp",
        self::tmp::init_tmpfs(),
        NodePermission::from_bits_truncate(0o1777),
    )?;
    mount_fs(
        "/proc",
        self::proc::init_procfs(),
        NodePermission::from_bits_truncate(0o555),
    )?;
    mount_fs(
        "/etc",
        self::etc::init_etcfs(),
        NodePermission::from_bits_truncate(0o555),
    )?;
    Ok(())
}

pub fn is_virtual_fs(path: &str) -> bool {
    path.starts_with("/dev")
        || path.starts_with("/tmp")
        || path.starts_with("/proc")
        || path.starts_with("/etc")
        || path.starts_with("/sys")
}
