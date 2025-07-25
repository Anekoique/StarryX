use core::ffi::c_char;

use axerrno::{LinuxError, LinuxResult};
use rand::{RngCore, SeedableRng, rngs::SmallRng};

use axuspace::{UserPtr, UserSpaceAccess};
use xcore::task::{processes, with_uspace};

use crate::{
    ctypes::{AT_FDCWD, GRND_NONBLOCK, GRND_RANDOM, new_utsname, sysinfo},
    fs::with_fs,
    time::wall_time_nanos,
};

/// Get real user ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_getuid() -> LinuxResult<isize> {
    Ok(0)
}

/// Set real user ID of the calling process.
///
/// # Arguments
/// * `uid` - User ID to set
pub fn sys_setuid(_uid: u32) -> LinuxResult<isize> {
    Ok(0)
}

/// Get effective user ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_geteuid() -> LinuxResult<isize> {
    Ok(0)
}

/// Set real and effective user IDs of the calling process.
///
/// # Arguments
/// * `uid` - User ID to set
/// * `euid` - Effective user ID to set
pub fn sys_setreuid(_uid: u32, _euid: u32) -> LinuxResult<isize> {
    Ok(0)
}

/// Set real, effective, and saved user IDs of the calling process.
///
/// # Arguments
/// * `uid` - User ID to set
/// * `euid` - Effective user ID to set
/// * `suid` - Saved user ID to set
pub fn sys_setresuid(_uid: u32, _euid: u32, _suid: u32) -> LinuxResult<isize> {
    Ok(0)
}

/// Get real group ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_getgid() -> LinuxResult<isize> {
    Ok(0)
}

/// Get effective group ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_getegid() -> LinuxResult<isize> {
    Ok(1)
}

const fn pad_str(info: &str) -> [c_char; 65] {
    let mut data: [c_char; 65] = [0; 65];
    // this needs #![feature(const_copy_from_slice)]
    // data[..info.len()].copy_from_slice(info.as_bytes());
    unsafe {
        core::ptr::copy_nonoverlapping(info.as_ptr().cast(), data.as_mut_ptr(), info.len());
    }
    data
}

const UTSNAME: new_utsname = new_utsname {
    sysname: pad_str("Linux"),
    nodename: pad_str("StarryX - machine[0]"),
    release: pad_str("10.0.0"),
    version: pad_str("10.0.0"),
    machine: pad_str("10.0.0"),
    domainname: pad_str("https://github.com/Anekoique/StarryX"),
};

/// Get system identification information.
///
/// # Arguments
/// * `name` - Buffer to store system information
pub fn sys_uname(name: UserPtr<new_utsname>) -> LinuxResult<isize> {
    with_uspace(|uspace| uspace.write(name, UTSNAME))?;
    Ok(0)
}

/// Get system information.
///
/// # Arguments
/// * `info` - Buffer to store system information
pub fn sys_sysinfo(info: UserPtr<sysinfo>) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        let info = uspace.raw_ptr(info)?;
        info.uptime = 0;
        info.loads = [0, 0, 0];
        info.totalram = 0;
        info.freeram = 0;
        info.sharedram = 0;
        info.bufferram = 0;
        info.totalswap = 0;
        info.freeswap = 0;
        info.procs = processes().len() as _;
        info.totalhigh = 0;
        info.freehigh = 0;
        info.mem_unit = 1;
        Ok(0)
    })
}

/// Read from system log.
///
/// # Arguments
/// * `_type` - Log type (currently unused)
/// * `_buf` - Buffer to store log data (currently unused)
/// * `_len` - Buffer length (currently unused)
pub fn sys_syslog(_type: i32, _buf: UserPtr<c_char>, _len: usize) -> LinuxResult<isize> {
    Ok(0)
}

/// Get random bytes.
///
/// # Arguments
/// * `buf` - Buffer to store random bytes
/// * `len` - Number of bytes to generate
/// * `flags` - Flags controlling the operation
pub fn sys_getrandom(buf: UserPtr<u8>, len: usize, flags: u32) -> LinuxResult<isize> {
    if flags & !(GRND_NONBLOCK | GRND_RANDOM) != 0 {
        return Err(LinuxError::EINVAL);
    }

    let buffer = with_uspace(|uspace| uspace.raw_slice(buf, len))?;
    let device_path = if flags & GRND_RANDOM != 0 {
        "/dev/random"
    } else {
        "/dev/urandom"
    };

    with_fs(AT_FDCWD, device_path, |fs| {
        fs.open_file(device_path)?.read_at(buffer, 0)
    })
    .map(|bytes_read| bytes_read as isize)
    .or_else(|_| {
        let seed = (buffer.as_ptr() as u64) + len as u64 + wall_time_nanos();
        SmallRng::seed_from_u64(seed).fill_bytes(buffer);
        Ok(len as isize)
    })
}
