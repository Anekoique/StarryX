use core::ffi::c_ulong;

use axerrno::LinuxError;
use bitflags::bitflags;
use linux_raw_sys::{
    general::{
        __kernel_sighandler_t, __sigrestore_t, SA_NODEFER, SA_ONSTACK, SA_RESETHAND, SA_RESTART,
        SA_SIGINFO, kernel_sigaction,
    },
    signal_macros::sig_ign,
};

use crate::SignalSet;

/// Default signal actions
///
/// Defines the default behavior for signals when no custom handler is installed.
/// Each signal type has one of these default actions.
#[derive(Debug)]
pub enum DefaultSignalAction {
    /// Terminate the process.
    Terminate,

    /// Ignore the signal.
    Ignore,

    /// Terminate the process and generate a core dump.
    CoreDump,

    /// Stop the process.
    Stop,

    /// Continue the process if stopped.
    Continue,
}

/// Signal action that should be properly handled by the OS
///
/// Represents the action that the operating system should take when
/// a signal is delivered. This is used by the signal management system
/// to determine what OS-level action is required.
///
/// This enum is returned by signal checking methods to indicate what
/// the operating system should do in response to a signal.
pub enum SignalOSAction {
    /// Terminate the process.
    Terminate,
    /// Generate a core dump and terminate the process.
    CoreDump,
    /// Stop the process.
    Stop,
    /// Continue the process if stopped.
    Continue,
    /// A signal handler is pushed into the signal stack. The OS doesn't need to
    /// do anything.
    Handler,
}

bitflags! {
    /// Signal action flags
    ///
    /// These flags modify the behavior of signal handlers and signal delivery.
    /// They correspond to the flags used in the `sigaction` system call.
    #[derive(Default, Debug)]
    pub struct SignalActionFlags: c_ulong {
        /// Use extended signal information (siginfo_t) in handler
        const SIGINFO = SA_SIGINFO as _;
        /// Don't block this signal while handler is running
        const NODEFER = SA_NODEFER as _;
        /// Reset handler to default after one execution
        const RESETHAND = SA_RESETHAND as _;
        /// Restart interrupted system calls
        const RESTART = SA_RESTART as _;
        /// Use alternate signal stack
        const ONSTACK = SA_ONSTACK as _;
        /// Don't create zombie on child death
        const NOCLDSTOP = 0x20000000;
        /// Custom restorer function is provided
        const RESTORER = 0x4000000;
    }
}

// FIXME: replace with `kernel_sigaction` after finishing above "TODO"s for `SignalSet`
/// Kernel-level signal action structure
///
/// Low-level representation of signal actions compatible with kernel interfaces.
/// This is an internal structure used for interfacing with the kernel's signal
/// handling mechanisms.
#[derive(Clone, Copy)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub struct k_sigaction {
    handler: __kernel_sighandler_t,
    flags: c_ulong,
    restorer: __sigrestore_t,
    /// Signal mask to apply during handler execution
    pub mask: SignalSet,
}

/// Signal disposition (handler type)
///
/// Defines what should happen when a signal is delivered:
/// - Use default behavior
/// - Ignore the signal
/// - Execute a custom handler function
#[derive(Default)]
pub enum SignalDisposition {
    #[default]
    /// Use the default signal action.
    Default,
    /// Ignore the signal.
    Ignore,
    /// Custom signal handler.
    Handler(unsafe extern "C" fn(i32)),
}

/// Signal action configuration
///
/// Corresponds to `struct sigaction` in libc. This structure defines
/// how a particular signal should be handled, including the handler
/// function, flags, and signal mask to apply during handler execution.
#[derive(Default)]
pub struct SignalAction {
    /// Flags that modify signal handling behavior
    pub flags: SignalActionFlags,
    /// Signals to block while this handler is executing
    pub mask: SignalSet,
    /// What to do when the signal is received
    pub disposition: SignalDisposition,
    /// Optional signal restorer function
    pub restorer: __sigrestore_t,
}

impl SignalAction {
    /// Converts to C-compatible sigaction representation
    pub fn to_ctype(&self, dest: &mut kernel_sigaction) {
        dest.sa_flags = self.flags.bits() as _;
        self.mask.to_ctype(&mut dest.sa_mask);
        match &self.disposition {
            SignalDisposition::Default => {
                dest.sa_handler_kernel = None;
            }
            SignalDisposition::Ignore => {
                dest.sa_handler_kernel = sig_ign();
            }
            SignalDisposition::Handler(handler) => {
                dest.sa_handler_kernel = Some(*handler);
            }
        }
        #[cfg(sa_restorer)]
        {
            dest.sa_restorer = self.restorer;
        }
    }
}

impl TryFrom<kernel_sigaction> for SignalAction {
    type Error = LinuxError;

    /// Converts from C-compatible sigaction representation
    fn try_from(value: kernel_sigaction) -> Result<Self, Self::Error> {
        let Some(flags) = SignalActionFlags::from_bits(value.sa_flags) else {
            warn!("unrecognized signal flags: {}", value.sa_flags);
            return Err(LinuxError::EINVAL);
        };
        let disposition = {
            match value.sa_handler_kernel {
                None => {
                    // SIG_DFL
                    SignalDisposition::Default
                }
                Some(h) if h as usize == 1 => {
                    // SIG_IGN
                    SignalDisposition::Ignore
                }
                Some(h) => {
                    // Custom signal handler
                    SignalDisposition::Handler(h)
                }
            }
        };

        #[cfg(sa_restorer)]
        let restorer = if flags.contains(SignalActionFlags::RESTORER) {
            value.sa_restorer
        } else {
            None
        };
        #[cfg(not(sa_restorer))]
        let restorer = None;

        Ok(SignalAction {
            flags,
            mask: value.sa_mask.into(),
            disposition,
            restorer,
        })
    }
}
