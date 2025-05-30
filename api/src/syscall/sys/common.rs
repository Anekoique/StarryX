use core::ffi::c_char;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{FileFlags, OpenOptions};
use rand::{RngCore, SeedableRng, rngs::SmallRng};
use starry_core::task::processes;

use crate::{
    ctypes::{AT_FDCWD, GRND_NONBLOCK, GRND_RANDOM, new_utsname, sysinfo},
    fs::with_fs,
    ptr::UserPtr,
    time::wall_time_nanos,
};

pub fn sys_getuid() -> LinuxResult<isize> {
    Ok(0)
}

pub fn sys_geteuid() -> LinuxResult<isize> {
    Ok(1)
}

pub fn sys_getgid() -> LinuxResult<isize> {
    Ok(0)
}

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
    sysname: pad_str("Starry"),
    nodename: pad_str("Starry - machine[0]"),
    release: pad_str("10.0.0"),
    version: pad_str("10.0.0"),
    machine: pad_str("10.0.0"),
    domainname: pad_str("https://github.com/oscomp/starry-next"),
};

pub fn sys_uname(name: UserPtr<new_utsname>) -> LinuxResult<isize> {
    *name.get_as_mut()? = UTSNAME;
    Ok(0)
}

pub fn sys_sysinfo(info: UserPtr<sysinfo>) -> LinuxResult<isize> {
    let info = info.get_as_mut()?;
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
}

pub fn sys_syslog(_type: i32, _buf: UserPtr<c_char>, _len: usize) -> LinuxResult<isize> {
    Ok(0)
}

pub fn sys_getrandom(buf: UserPtr<u8>, len: usize, flags: u32) -> LinuxResult<isize> {
    // Validate flags
    if flags & !(GRND_NONBLOCK | GRND_RANDOM) != 0 {
        return Err(LinuxError::EINVAL);
    }

    let buffer = buf.get_as_mut_slice(len)?;
    let device_path = if flags & GRND_RANDOM != 0 {
        "/dev/random"
    } else {
        "/dev/urandom"
    };

    // Try reading from device, fallback to PRNG
    with_fs(AT_FDCWD, device_path, |fs| {
        OpenOptions::new()
            .read(true)
            .open(fs, device_path)?
            .into_file()?
            .access(FileFlags::READ)?
            .read_at(buffer, 0)
    })
    .map(|bytes_read| bytes_read as isize)
    .or_else(|_| {
        let seed = (buffer.as_ptr() as u64) + len as u64 + wall_time_nanos();
        SmallRng::seed_from_u64(seed).fill_bytes(buffer);
        Ok(len as isize)
    })
}
