use core::ffi::{c_char, c_void};

use axerrno::LinuxResult;

use xuspace::UserConstPtr;

/// Mount a filesystem.
///
/// # Arguments
/// * `_source` - Device or filesystem to mount (currently unused)
/// * `_target` - Mount point (currently unused)
/// * `_fs_type` - Filesystem type (currently unused)
/// * `_flags` - Mount flags (currently unused)
/// * `_data` - Mount options (currently unused)
pub fn sys_mount(
    _source: UserConstPtr<c_char>,
    _target: UserConstPtr<c_char>,
    _fs_type: UserConstPtr<c_char>,
    _flags: i32,
    _data: UserConstPtr<c_void>,
) -> LinuxResult<isize> {
    Ok(0)
}

/// Unmount a filesystem.
///
/// # Arguments
/// * `_target` - Mount point to unmount (currently unused)
/// * `_flags` - Unmount flags (currently unused)
pub fn sys_umount2(_target: UserConstPtr<c_char>, _flags: i32) -> LinuxResult<isize> {
    Ok(0)
}
