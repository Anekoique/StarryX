//! Virtual filesystems implementation
//!
//! This module provides various virtual filesystems including:
//! - `/dev` - Device filesystem (devfs)
//! - `/tmp` - Temporary filesystem (tmpfs)
//! - `/proc` - Process information filesystem (procfs)
#![allow(dead_code)]
#![allow(clippy::len_without_is_empty)]

pub mod api;
pub mod fd;
pub mod file;
pub mod vfs;

pub use api::*;

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
        vfs::dev::init_devfs()?,
        NodePermission::from_bits_truncate(0o755),
    )?;
    mount_fs(
        "/tmp",
        vfs::tmp::init_tmpfs(),
        NodePermission::from_bits_truncate(0o1777),
    )?;
    mount_fs(
        "/proc",
        vfs::proc::init_procfs(),
        NodePermission::from_bits_truncate(0o555),
    )?;
    mount_fs(
        "/etc",
        vfs::etc::init_etcfs(),
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
