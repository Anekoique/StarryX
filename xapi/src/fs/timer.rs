use axerrno::LinuxResult;
use axfs_ng::FileFlags;

use xcore::{
    fs::{fd::TimerFd, file::FileLike},
    task::with_uspace,
};
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::ctypes::{__kernel_clockid_t, TFD_CLOEXEC, sys::itimerspec};

/// Create a new timer file descriptor
///
/// # Arguments
/// * `clock_id` - Clock type (CLOCK_REALTIME or CLOCK_MONOTONIC)
/// * `flags` - Creation flags (TFD_CLOEXEC, TFD_NONBLOCK)
pub fn sys_timerfd_create(clock_id: __kernel_clockid_t, flags: i32) -> LinuxResult<isize> {
    trace!("sys_timerfd_create: clock_id={}, flags={}", clock_id, flags);
    let timer_fd = TimerFd::new(clock_id, flags)?;
    let fd = timer_fd.add_to_fd_table(FileFlags::READ, (flags & TFD_CLOEXEC as i32) != 0)?;
    Ok(fd as isize)
}

/// Set timer parameters
///
/// # Arguments
/// * `fd` - Timer file descriptor
/// * `flags` - Timer flags (TFD_TIMER_ABSTIME, etc.)
/// * `new_value` - New timer specification
/// * `old_value` - Buffer to store previous timer specification (can be NULL)
pub fn sys_timerfd_settime(
    fd: i32,
    flags: i32,
    new_value: UserConstPtr<itimerspec>,
    old_value: UserPtr<itimerspec>,
) -> LinuxResult<isize> {
    trace!("sys_timerfd_settime: fd={}, flags={}", fd, flags);
    with_uspace(|uspace| {
        let new_spec = uspace.read(new_value)?;
        let file = TimerFd::from_fd(fd, FileFlags::WRITE, FileFlags::empty())?;
        let old_spec = file.set_timer(new_spec, flags)?;
        nullable!(uspace.write(old_value, old_spec))?;
        Ok(0)
    })
}

/// Get timer parameters
///
/// # Arguments
/// * `fd` - Timer file descriptor
/// * `old_value` - Buffer to store current timer specification (can be NULL)
/// * `curr_value` - Buffer to store current timer specification
pub fn sys_timerfd_gettime(
    fd: i32,
    _old_value: UserPtr<itimerspec>,
    curr_value: UserPtr<itimerspec>,
) -> LinuxResult<isize> {
    trace!("sys_timerfd_gettime: fd={}", fd);
    with_uspace(|uspace| {
        let file = TimerFd::from_fd(fd, FileFlags::READ, FileFlags::empty())?;
        let curr_spec = file.get_timer();
        nullable!(uspace.write(curr_value, curr_spec))?;
        Ok(0)
    })
}
