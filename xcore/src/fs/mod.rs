//! Virtual filesystems implementation
//!
//! This module provides various virtual filesystems including:
//! - `/dev` - Device filesystem (devfs)
//! - `/tmp` - Temporary filesystem (tmpfs)
//! - `/proc` - Process information filesystem (procfs)

mod dev;
mod proc;
mod tmp;
mod virt_file;
mod virt_fs;

// Re-export commonly used types and constants
pub use dev::RTC0_DEVICE_ID;

use axerrno::LinuxResult;
use axfs_ng::FS_CONTEXT;
use axfs_ng_vfs::{Filesystem, NodePermission, StatFs, path::MAX_NAME_LEN};
use axsync::RawMutex;

/// Create a dummy statfs for virtual filesystems
pub(crate) fn dummy_stat(fs_type: u32) -> StatFs {
    StatFs {
        fs_type,
        block_size: 4096,
        blocks: 0,
        blocks_free: 0,
        blocks_available: 0,
        file_count: 0,
        free_file_count: 0,
        name_length: MAX_NAME_LEN as _,
        fragment_size: 0,
        mount_flags: 0,
    }
}

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
        dev::init_devfs()?,
        NodePermission::from_bits_truncate(0o755),
    )?;
    mount_fs(
        "/tmp",
        tmp::init_tmpfs(),
        NodePermission::from_bits_truncate(0o1777),
    )?;
    mount_fs(
        "/proc",
        proc::init_procfs(),
        NodePermission::from_bits_truncate(0o555),
    )?;
    Ok(())
}

pub fn is_virtual_fs(path: &str) -> bool {
    path.starts_with("/dev")
        || path.starts_with("/tmp")
        || path.starts_with("/proc")
        || path.starts_with("/sys")
}
