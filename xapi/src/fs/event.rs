use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use xcore::fs::{fd::EventFd, file::FileLike};
use xutils::ctypes::{EFD_CLOEXEC, EFD_NONBLOCK, EFD_SEMAPHORE};

/// Create an eventfd file descriptor.
///
/// # Arguments
/// * `initval` - Initial value for the eventfd counter
/// * `flags` - Flags controlling eventfd behavior (EFD_CLOEXEC, EFD_NONBLOCK, EFD_SEMAPHORE)
pub fn sys_eventfd2(initval: u32, flags: i32) -> LinuxResult<isize> {
    debug!("sys_eventfd2 <= initval: {}, flags: {:#x}", initval, flags);

    // Validate flags
    let valid_flags = EFD_CLOEXEC | EFD_NONBLOCK | EFD_SEMAPHORE;
    if (flags as u32) & !valid_flags != 0 {
        return Err(LinuxError::EINVAL);
    }

    // Create the EventFd instance
    let eventfd = EventFd::new(initval as u64, flags);

    // Add to file descriptor table
    let fd = eventfd.add_to_fd_table(
        FileFlags::READ | FileFlags::WRITE,
        (flags as u32 & EFD_CLOEXEC) != 0,
    )?;

    debug!("sys_eventfd2 => fd: {}", fd);
    Ok(fd as isize)
}

/// Legacy eventfd syscall (without flags parameter).
///
/// # Arguments  
/// * `initval` - Initial value for the eventfd counter
pub fn sys_eventfd(initval: u32) -> LinuxResult<isize> {
    sys_eventfd2(initval, 0)
}
