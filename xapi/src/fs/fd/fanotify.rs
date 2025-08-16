// ref Nighthawk's implementation
use alloc::sync::Arc;
use core::ffi::{c_char, c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axsync::Mutex;

use xcore::{
    fs::{
        fd::{FanKey, FanTarget, FanWatch, FanotifyGroup},
        file::FileLike,
        with_location,
    },
    task::with_uspace,
};
use xuspace::{UserConstPtr, UserSpaceAccess, nullable};
use xutils::ctypes::{
    AT_SYMLINK_NOFOLLOW,
    fs::{FanEventMask, FanInitEventFlags, FanInitFlags, FanMarkFlags},
};

/// Create a fanotify file descriptor.
///
/// # Arguments
/// * `flags` - Flags controlling fanotify behavior (FAN_CLOEXEC, FAN_NONBLOCK, etc.)
/// * `event_flags` - Event configuration flags
pub fn sys_fanotify_init(flags: u32, event_flags: u32) -> LinuxResult<isize> {
    debug!(
        "sys_fanotify_init <= flags: {:#x}, event_flags: {:#x}",
        flags, event_flags
    );
    let flags = FanInitFlags::from_bits(flags).ok_or(LinuxError::EINVAL)?;
    let event_flags = FanInitEventFlags::from_bits(event_flags).ok_or(LinuxError::EINVAL)?;

    if flags.contains(FanInitFlags::CLASS_PRE_CONTENT | FanInitFlags::CLASS_CONTENT) {
        return Err(LinuxError::EINVAL);
    }
    if flags.intersects(FanInitFlags::CLASS_PRE_CONTENT | FanInitFlags::CLASS_CONTENT)
        && flags.contains(FanInitFlags::REPORT_FID)
    {
        return Err(LinuxError::EINVAL);
    }
    if flags.contains(FanInitFlags::REPORT_NAME) && !flags.contains(FanInitFlags::REPORT_DIR_FID) {
        return Err(LinuxError::EINVAL);
    }
    if flags.contains(FanInitFlags::REPORT_TARGET_FID)
        && !flags.contains(
            FanInitFlags::REPORT_FID | FanInitFlags::REPORT_DIR_FID | FanInitFlags::REPORT_NAME,
        )
    {
        return Err(LinuxError::EINVAL);
    }
    if flags.contains(FanInitFlags::REPORT_PIDFD | FanInitFlags::REPORT_TID) {
        return Err(LinuxError::EINVAL);
    }

    if flags.intersects(
        FanInitFlags::UNLIMITED_MARKS | FanInitFlags::ENABLE_AUDIT | FanInitFlags::REPORT_PIDFD,
    ) {
        unimplemented!("Unsupported fanotify flags: {flags:?}");
    }
    let group = FanotifyGroup::new(flags, event_flags);

    let fd = group.add_to_fd_table(
        FileFlags::READ | FileFlags::WRITE,
        flags.contains(FanInitFlags::CLOEXEC),
    )?;

    Ok(fd as isize)
}

/// Mark a filesystem object to watch for events.
///
/// # Arguments
/// * `fanotify_fd` - The fanotify file descriptor
/// * `flags` - Mark operation flags
/// * `mask` - Events to watch for
/// * `dirfd` - Directory file descriptor for relative paths
/// * `pathname` - Path to the object to mark
pub fn sys_fanotify_mark(
    fanotify_fd: c_int,
    flags: u32,
    mask: u64,
    dirfd: c_int,
    pathname: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    let path = with_uspace(|uspace| nullable!(uspace.read_str(pathname)))?;
    debug!(
        "sys_fanotify_mark <= fanotify_fd: {}, flags: {:#x}, mask: {:#x}, dirfd: {}, pathname: {:?}",
        fanotify_fd, flags, mask, dirfd, path
    );

    let fan_group = FanotifyGroup::from_fd(
        fanotify_fd,
        FileFlags::READ | FileFlags::WRITE,
        FileFlags::empty(),
    )?;
    let group_flags = fan_group.flags();
    let flags = FanMarkFlags::from_bits(flags).ok_or(LinuxError::EINVAL)?;
    let mask = FanEventMask::from_bits(mask).ok_or(LinuxError::EINVAL)?;

    if flags
        .intersection(FanMarkFlags::ADD | FanMarkFlags::REMOVE | FanMarkFlags::FLUSH)
        .bits()
        .count_ones()
        != 1
    {
        return Err(LinuxError::EINVAL);
    }
    if flags.contains(FanMarkFlags::MOUNT) && mask.intersects(FanEventMask::FID_EVENT_MASK) {
        return Err(LinuxError::EINVAL);
    }
    if flags.contains(FanMarkFlags::IGNORE)
        && flags.intersects(FanMarkFlags::MOUNT | FanMarkFlags::FILESYSTEM)
        && !flags.contains(FanMarkFlags::IGNORED_SURV_MODIFY)
    {
        return Err(LinuxError::EINVAL);
    }
    if mask.contains(FanEventMask::RENAME) && !group_flags.contains(FanInitFlags::REPORT_NAME) {
        return Err(LinuxError::EINVAL);
    }
    if flags.contains(FanMarkFlags::FLUSH) {
        if flags == FanMarkFlags::FLUSH {
            fan_group.flush_normal_entries();
        } else if flags == FanMarkFlags::FLUSH | FanMarkFlags::MOUNT {
            fan_group.flush_mount_entries();
        } else if flags == FanMarkFlags::FLUSH | FanMarkFlags::FILESYSTEM {
            fan_group.flush_filesystem_entries();
        } else {
            return Err(LinuxError::EINVAL);
        };
        return Ok(0);
    }

    // if mask.intersects(
    //     FanEventMask::OPEN_EXEC
    //         | FanEventMask::FS_ERROR
    //         | FanEventMask::ACCESS_PERM
    //         | FanEventMask::OPEN_PERM
    //         | FanEventMask::OPEN_EXEC_PERM,
    // ) {
    //     unimplemented!("Unsupported fanotify mask: {mask:?}");
    // }

    let loc_flag = if flags.contains(FanMarkFlags::DONT_FOLLOW) {
        AT_SYMLINK_NOFOLLOW
    } else {
        0
    };

    with_location(dirfd, path, loc_flag, |location| {
        if !location.is_dir() {
            if flags.contains(FanMarkFlags::ONLYDIR) {
                return Err(LinuxError::ENOTDIR);
            }
            if !flags.intersects(FanMarkFlags::MOUNT | FanMarkFlags::FILESYSTEM) {
                if mask.contains(FanEventMask::RENAME) {
                    return Err(LinuxError::ENOTDIR);
                }
                if (flags.contains(FanMarkFlags::IGNORE)
                    || group_flags.contains(FanInitFlags::REPORT_TARGET_FID))
                    && mask.intersects(
                        FanEventMask::DIR_EVENT_MASK
                            | FanEventMask::ONDIR
                            | FanEventMask::EVENT_ON_CHILD,
                    )
                {
                    return Err(LinuxError::ENOTDIR);
                }
            }
        }
        if mask.intersects(FanEventMask::FID_EVENT_MASK)
            && !group_flags.intersects(FanInitFlags::REPORT_FID | FanInitFlags::REPORT_DIR_FID)
        {
            return Err(LinuxError::EINVAL);
        }
        if flags.contains(FanMarkFlags::IGNORE)
            && !flags.contains(FanMarkFlags::IGNORED_SURV_MODIFY)
            && location.is_dir()
        {
            return Err(LinuxError::EISDIR);
        }

        let (key, target) = if flags.intersects(FanMarkFlags::MOUNT | FanMarkFlags::FILESYSTEM) {
            (
                FanKey::Mountpoint(location.mountpoint().device()),
                FanTarget::Mountpoint(Arc::downgrade(location.mountpoint())),
            )
        } else {
            (
                FanKey::Inode(location.inode()),
                FanTarget::Inode(location.node()),
            )
        };

        let (mark, ignore) = if flags.intersects(FanMarkFlags::IGNORED_MASK | FanMarkFlags::IGNORE)
        {
            (FanEventMask::empty(), mask)
        } else {
            (mask, FanEventMask::empty())
        };

        if let Some(watch) = fan_group.get_watch(key) {
            let mut watch = watch.lock();
            let old_flags = watch.mark;
            if flags.contains(FanMarkFlags::IGNORED_MASK)
                && old_flags.contains(FanMarkFlags::IGNORE)
            {
                return Err(LinuxError::EEXIST);
            }
            if !flags.contains(FanMarkFlags::IGNORED_SURV_MODIFY)
                && old_flags.contains(FanMarkFlags::IGNORE | FanMarkFlags::IGNORED_SURV_MODIFY)
            {
                return Err(LinuxError::EEXIST);
            }
            watch.set_mark(flags);
            if flags.contains(FanMarkFlags::ADD) {
                watch.add_mark(mark);
                watch.add_ignore(ignore);
            } else {
                watch.remove_mark(mark);
                watch.remove_ignore(ignore);
            }
        } else if flags.contains(FanMarkFlags::ADD) {
            fan_group.add_watch(
                key,
                Arc::new(Mutex::new(FanWatch::new(target, flags, mark, ignore))),
            );
        } else {
            return Err(LinuxError::EINVAL);
        }
        Ok(0)
    })
}
