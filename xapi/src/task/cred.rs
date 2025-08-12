// ref NoAxiom
use axerrno::{LinuxError, LinuxResult};

use xcore::task::with_xprocess;
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess};

/// Get real user ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_getuid() -> LinuxResult<isize> {
    with_xprocess(|process| Ok(process.uid() as isize))
}

/// Set real user ID of the calling process.
///
/// # Arguments
/// * `uid` - User ID to set
pub fn sys_setuid(uid: u32) -> LinuxResult<isize> {
    with_xprocess(|process| {
        if process.uid() == 0 {
            process.set_uid(uid);
            process.set_euid(uid);
            process.set_suid(uid);
            process.set_fsuid(uid);
        } else {
            if uid != process.uid() && uid != process.suid() {
                return Err(LinuxError::EPERM);
            }
            process.set_euid(uid);
            process.set_fsuid(uid);
        }
        Ok(0)
    })
}

/// Get real group ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_getgid() -> LinuxResult<isize> {
    with_xprocess(|process| Ok(process.gid() as isize))
}

/// Set real group ID of the calling process.
///
/// # Arguments
/// * `gid` - Group ID to set
pub fn sys_setgid(gid: u32) -> LinuxResult<isize> {
    with_xprocess(|process| {
        if process.uid() == 0 {
            process.set_gid(gid);
            process.set_egid(gid);
            process.set_sgid(gid);
            process.set_fsgid(gid);
        } else {
            if gid != process.gid() && gid != process.sgid() {
                return Err(LinuxError::EPERM);
            }
            process.set_egid(gid);
            process.set_fsgid(gid);
        }
        Ok(0)
    })
}

/// Set file system user ID of the calling process.
///
/// # Arguments
/// * `fsuid` - File system user ID to set
pub fn sys_setfsuid(fsuid: i32) -> LinuxResult<isize> {
    with_xprocess(|process| {
        let origin_fsuid = process.fsuid();
        if process.euid() == 0 {
            if fsuid != -1 {
                process.set_fsuid(fsuid as _);
            }
        } else if fsuid == process.uid() as i32
            || fsuid == process.euid() as i32
            || fsuid == process.suid() as i32
            || fsuid == process.fsuid() as i32
        {
            process.set_fsuid(fsuid as _);
        }
        Ok(origin_fsuid as _)
    })
}

/// Set file system group ID of the calling process.
///
/// # Arguments
/// * `fsgid` - File system group ID to set
pub fn sys_setfsgid(fsgid: i32) -> LinuxResult<isize> {
    with_xprocess(|process| {
        let origin_fsgid = process.fsgid();
        if process.euid() == 0 {
            if fsgid != -1 {
                process.set_fsgid(fsgid as _);
            }
        } else if fsgid == process.gid() as i32
            || fsgid == process.egid() as i32
            || fsgid == process.sgid() as i32
            || fsgid == process.fsgid() as i32
        {
            process.set_fsgid(fsgid as _);
        }
        Ok(origin_fsgid as _)
    })
}

/// Get effective user ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_geteuid() -> LinuxResult<isize> {
    with_xprocess(|process| Ok(process.euid() as isize))
}

/// Get effective group ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_getegid() -> LinuxResult<isize> {
    with_xprocess(|process| Ok(process.egid() as isize))
}

/// Set real and effective user IDs of the calling process.
///
/// # Arguments
/// * `uid` - User ID to set
/// * `euid` - Effective user ID to set
pub fn sys_setreuid(uid: i32, euid: i32) -> LinuxResult<isize> {
    with_xprocess(|process| {
        let origin_uid = process.uid() as _;
        let origin_euid = process.euid() as _;
        let origin_suid = process.suid() as _;
        if process.euid() == 0 {
            if uid != -1 {
                process.set_uid(uid as _);
            }
            if euid != -1 {
                process.set_euid(euid as _);
            }
        } else {
            if uid != -1 {
                if uid != origin_uid && uid != origin_euid {
                    return Err(LinuxError::EPERM);
                }
                process.set_uid(uid as _);
            }
            if euid != -1 {
                if euid != origin_uid && euid != origin_suid && euid != origin_euid {
                    return Err(LinuxError::EPERM);
                }
                process.set_euid(euid as _);
                process.set_fsuid(euid as _);
            }
        }
        if uid != -1 || (euid != -1 && euid != origin_uid) {
            process.set_suid(process.euid());
        }
        Ok(0)
    })
}

/// Set real and effective group IDs of the calling process.
///
/// # Arguments
/// * `gid` - Group ID to set
/// * `egid` - Effective group ID to set
pub fn sys_setregid(gid: i32, egid: i32) -> LinuxResult<isize> {
    with_xprocess(|process| {
        let origin_gid = process.gid() as _;
        let origin_egid = process.egid() as _;
        let origin_sgid = process.sgid() as _;
        if process.egid() == 0 {
            if gid != -1 {
                process.set_gid(gid as _);
            }
            if egid != -1 {
                process.set_egid(egid as _);
            }
        } else {
            if gid != -1 {
                if gid != origin_gid && gid != origin_egid {
                    return Err(LinuxError::EPERM);
                }
                process.set_gid(gid as _);
            }
            if egid != -1 {
                if egid != origin_gid && egid != origin_sgid && egid != origin_egid {
                    return Err(LinuxError::EPERM);
                }
                process.set_egid(egid as _);
                process.set_fsgid(egid as _);
            }
        }
        if gid != -1 || (egid != -1 && egid != origin_gid) {
            process.set_sgid(process.egid());
        }
        Ok(0)
    })
}

pub fn sys_getresuid(
    uid: UserPtr<u32>,
    euid: UserPtr<u32>,
    suid: UserPtr<u32>,
) -> LinuxResult<isize> {
    with_xprocess(|process| {
        let uspace = process.uspace();
        uspace.write(uid, process.uid())?;
        uspace.write(euid, process.euid())?;
        uspace.write(suid, process.suid())?;
        Ok(0)
    })
}

pub fn sys_getresgid(
    gid: UserPtr<u32>,
    egid: UserPtr<u32>,
    sgid: UserPtr<u32>,
) -> LinuxResult<isize> {
    with_xprocess(|process| {
        let uspace = process.uspace();
        uspace.write(gid, process.gid())?;
        uspace.write(egid, process.egid())?;
        uspace.write(sgid, process.sgid())?;
        Ok(0)
    })
}

/// Set real, effective, and saved user IDs of the calling process.
///
/// # Arguments
/// * `uid` - User ID to set
/// * `euid` - Effective user ID to set
/// * `suid` - Saved user ID to set
pub fn sys_setresuid(uid: i32, euid: i32, suid: i32) -> LinuxResult<isize> {
    with_xprocess(|process| {
        let origin_uid = process.uid() as _;
        let origin_euid = process.euid() as _;
        let origin_suid = process.suid() as _;
        if process.euid() == 0 {
            if uid != -1 {
                process.set_uid(uid as _);
            }
            if euid != -1 {
                process.set_euid(euid as _);
            }
            if suid != -1 {
                process.set_suid(suid as _);
            }
        } else {
            if uid != -1 {
                if uid != origin_uid && uid != origin_euid {
                    return Err(LinuxError::EPERM);
                }
                process.set_uid(uid as _);
            }
            if euid != -1 {
                if euid != origin_uid && euid != origin_suid && euid != origin_euid {
                    return Err(LinuxError::EPERM);
                }
                process.set_euid(euid as _);
                process.set_fsuid(euid as _);
            }
            if suid != -1 {
                if suid != origin_uid && suid != origin_suid && suid != origin_euid {
                    return Err(LinuxError::EPERM);
                }
                process.set_suid(suid as _);
            }
        }
        Ok(0)
    })
}

pub fn sys_setresgid(gid: i32, egid: i32, sgid: i32) -> LinuxResult<isize> {
    with_xprocess(|process| {
        let origin_gid = process.gid() as _;
        let origin_egid = process.egid() as _;
        let origin_sgid = process.sgid() as _;
        if process.egid() == 0 {
            if gid != -1 {
                process.set_gid(gid as _);
            }
            if egid != -1 {
                process.set_egid(egid as _);
            }
            if sgid != -1 {
                process.set_sgid(sgid as _);
            }
        } else {
            if gid != -1 {
                if gid != origin_gid && gid != origin_egid {
                    return Err(LinuxError::EPERM);
                }
                process.set_gid(gid as _);
            }
            if egid != -1 {
                if egid != origin_gid && egid != origin_sgid && egid != origin_egid {
                    return Err(LinuxError::EPERM);
                }
                process.set_egid(egid as _);
                process.set_fsgid(egid as _);
            }
            if sgid != -1 {
                if sgid != origin_gid && sgid != origin_sgid && sgid != origin_egid {
                    return Err(LinuxError::EPERM);
                }
                process.set_sgid(sgid as _);
            }
        }
        Ok(0)
    })
}

pub fn sys_getgroups(size: usize, list: UserPtr<u32>) -> LinuxResult<isize> {
    with_xprocess(|process| {
        const NGROUPS_MAX: usize = 32;
        if size > NGROUPS_MAX {
            return Err(LinuxError::EINVAL);
        }

        let sup_group = process.sup_group();
        let len = sup_group.lock().len();
        if size == 0 {
            return Ok(len as _);
        }
        if len > size {
            return Err(LinuxError::EINVAL);
        }

        process.uspace().write_slice(list, &sup_group.lock())?;
        Ok(len as _)
    })
}

pub fn sys_setgroups(size: usize, list: UserConstPtr<u32>) -> LinuxResult<isize> {
    with_xprocess(|process| {
        const NGROUPS_MAX: usize = 32;
        if size > NGROUPS_MAX {
            return Err(LinuxError::EINVAL);
        }
        if process.euid() != 0 {
            return Err(LinuxError::EPERM);
        }
        let sup_group = process.sup_group();
        let mut sup_group = sup_group.lock();
        if size == 0 {
            sup_group.clear();
        } else {
            process.uspace().read_slice_to(list, &mut sup_group)?;
        }
        Ok(0)
    })
}
