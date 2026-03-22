use alloc::sync::Arc;

use axerrno::{LinuxError, LinuxResult};
use axtask::current;
#[cfg(target_arch = "x86_64")]
use num_enum::TryFromPrimitive;

use xprocess::{Pid, Process};

use xcore::task::{get_process, get_process_group, with_process, with_xthread};

/// Get process ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_getpid() -> LinuxResult<isize> {
    Ok(with_process(|process| process.pid()) as _)
}

/// Get parent process ID of the calling process.
///
/// # Arguments
/// None
pub fn sys_getppid() -> LinuxResult<isize> {
    Ok(with_process(|process| process.parent().unwrap().pid()) as _)
}

/// Get thread ID of the calling thread.
///
/// # Arguments
/// None
pub fn sys_gettid() -> LinuxResult<isize> {
    Ok(axtask::current().id().as_u64() as _)
}

/// Creates a new session if the calling process is not a process group leader.
/// Returns the session ID (which equals the process ID) on success.
///
/// # Arguments
/// None
pub fn sys_setsid() -> LinuxResult<isize> {
    with_process(|process| {
        let current_group = process.group();
        if current_group.pgid() == process.pid() {
            return Err(axerrno::LinuxError::EPERM);
        }

        if let Some((session, _group)) = process.create_session() {
            Ok(session.sid() as _)
        } else {
            Err(axerrno::LinuxError::EPERM)
        }
    })
}

pub fn sys_getsid(pid: Pid) -> LinuxResult<isize> {
    Ok(get_process(pid)?.group().session().sid() as isize)
}

pub fn sys_getpgid(pid: Pid) -> LinuxResult<isize> {
    Ok(get_process(pid)?.group().pgid() as isize)
}

pub fn sys_setpgid(pid: Pid, pgid: Pid) -> LinuxResult<isize> {
    let f = |process: &Arc<Process>| {
        if pgid == 0 {
            process.create_group();
            Ok(0)
        } else {
            let group = get_process_group(pgid)?;
            if !process.move_to_group(&group) {
                Err(LinuxError::EPERM)
            } else {
                Ok(0)
            }
        }
    };
    f(&get_process(pid)?)
}

/// ARCH_PRCTL codes
///
/// It is only avaliable on x86_64, and is not convenient
/// to generate automatically via c_to_rust binding.
#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(i32)]
#[cfg(target_arch = "x86_64")]
enum ArchPrctlCode {
    /// Set the GS segment base
    SetGs = 0x1001,
    /// Set the FS segment base
    SetFs = 0x1002,
    /// Get the FS segment base
    GetFs = 0x1003,
    /// Get the GS segment base
    GetGs = 0x1004,
    /// The setting of the flag manipulated by ARCH_SET_CPUID
    GetCpuid = 0x1011,
    /// Enable (addr != 0) or disable (addr == 0) the cpuid instruction for the calling thread.
    SetCpuid = 0x1012,
}

/// Set the clear_child_tid field in the task extended data.
///
/// # Arguments
/// * `clear_child_tid` - Address to clear when thread exits
pub fn sys_set_tid_address(clear_child_tid: usize) -> LinuxResult<isize> {
    with_xthread(|xthread| xthread.set_clear_child_tid(clear_child_tid));
    Ok(current().id().as_u64() as isize)
}

#[cfg(target_arch = "x86_64")]
/// Architecture-specific process control operations.
///
/// # Arguments
/// * `tf` - Trap frame
/// * `code` - Operation code
/// * `addr` - Address parameter
pub fn sys_arch_prctl(
    tf: &mut axhal::arch::TrapFrame,
    code: i32,
    addr: usize,
) -> LinuxResult<isize> {
    use xcore::task::api::with_uspace;
    use xuspace::{UserPtr, UserSpaceAccess};

    with_uspace(|uspace| {
        let code = ArchPrctlCode::try_from(code).map_err(|_| axerrno::LinuxError::EINVAL)?;
        debug!("sys_arch_prctl: code = {:?}, addr = {:#x}", code, addr);

        match code {
            ArchPrctlCode::GetFs => {
                uspace.write(UserPtr::from(addr), tf.tls())?;
                Ok(0)
            }
            ArchPrctlCode::SetFs => {
                tf.set_tls(addr);
                Ok(0)
            }
            ArchPrctlCode::GetGs => {
                uspace.write(UserPtr::from(addr), unsafe {
                    x86::msr::rdmsr(x86::msr::IA32_KERNEL_GSBASE)
                })?;
                Ok(0)
            }
            ArchPrctlCode::SetGs => {
                unsafe {
                    x86::msr::wrmsr(x86::msr::IA32_KERNEL_GSBASE, addr as _);
                }
                Ok(0)
            }
            ArchPrctlCode::GetCpuid => Ok(0),
            ArchPrctlCode::SetCpuid => Err(axerrno::LinuxError::ENODEV),
        }
    })
}
