use axerrno::LinuxResult;
use axtask::current;
use num_enum::TryFromPrimitive;
use starry_core::task::TaskExt;

pub fn sys_getpid() -> LinuxResult<isize> {
    Ok(TaskExt::from_task(&current()).thread.process().pid() as _)
}

pub fn sys_getppid() -> LinuxResult<isize> {
    Ok(TaskExt::from_task(&current())
        .thread
        .process()
        .parent()
        .unwrap()
        .pid() as _)
}

pub fn sys_gettid() -> LinuxResult<isize> {
    Ok(axtask::current().id().as_u64() as _)
}

/// Creates a new session if the calling process is not a process group leader.
/// Returns the session ID (which equals the process ID) on success.
pub fn sys_setsid() -> LinuxResult<isize> {
    let process = TaskExt::from_task(&current()).thread.process();

    // According to POSIX: setsid() shall fail if the calling process is already a process group leader
    let current_group = process.group();
    if current_group.pgid() == process.pid() {
        return Err(axerrno::LinuxError::EPERM);
    }

    // Create new session and process group
    // The process becomes the session leader and process group leader of the new session
    if let Some((session, _group)) = process.create_session() {
        Ok(session.sid() as _)
    } else {
        // This should not happen given our check above, but be defensive
        Err(axerrno::LinuxError::EPERM)
    }
}

/// ARCH_PRCTL codes
///
/// It is only avaliable on x86_64, and is not convenient
/// to generate automatically via c_to_rust binding.
#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(i32)]
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

/// To set the clear_child_tid field in the task extended data.
///
/// The set_tid_address() always succeeds
pub fn sys_set_tid_address(clear_child_tid: usize) -> LinuxResult<isize> {
    TaskExt::from_task(&current())
        .thread_data()
        .set_clear_child_tid(clear_child_tid);
    Ok(current().id().as_u64() as isize)
}

#[cfg(target_arch = "x86_64")]
pub fn sys_arch_prctl(
    tf: &mut axhal::arch::TrapFrame,
    code: i32,
    addr: usize,
) -> LinuxResult<isize> {
    use crate::ptr::UserPtr;

    let code = ArchPrctlCode::try_from(code).map_err(|_| axerrno::LinuxError::EINVAL)?;
    debug!("sys_arch_prctl: code = {:?}, addr = {:#x}", code, addr);

    match code {
        // According to Linux implementation, SetFs & SetGs does not return
        // error at all
        ArchPrctlCode::GetFs => {
            *UserPtr::from(addr).get_as_mut()? = tf.tls();
            Ok(0)
        }
        ArchPrctlCode::SetFs => {
            tf.set_tls(addr);
            Ok(0)
        }
        ArchPrctlCode::GetGs => {
            *UserPtr::from(addr).get_as_mut()? =
                unsafe { x86::msr::rdmsr(x86::msr::IA32_KERNEL_GSBASE) };
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
}
