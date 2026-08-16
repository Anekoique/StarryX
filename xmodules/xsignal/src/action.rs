use core::ffi::c_ulong;

use bitflags::bitflags;
use linux_raw_sys::general::{SA_NODEFER, SA_ONSTACK, SA_RESETHAND, SA_RESTART, SA_SIGINFO};
use xerrno::LinuxError;

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
    /// Address of a custom signal handler in user space.
    Handler(usize),
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
    /// Optional signal-return trampoline address in user space.
    pub restorer: Option<usize>,
}

impl SignalAction {
    /// Builds a signal action from the raw values supplied by the syscall ABI.
    pub fn from_raw_parts(
        handler: usize,
        flags: c_ulong,
        mask: SignalSet,
        restorer: Option<usize>,
    ) -> Result<Self, LinuxError> {
        let Some(flags) = SignalActionFlags::from_bits(flags) else {
            warn!("unrecognized signal flags: {flags}");
            return Err(LinuxError::EINVAL);
        };
        let disposition = match handler {
            0 => SignalDisposition::Default,
            1 => SignalDisposition::Ignore,
            address => SignalDisposition::Handler(address),
        };
        Ok(Self {
            flags,
            mask,
            disposition,
            restorer,
        })
    }

    /// Returns the Linux ABI value for the configured disposition.
    pub fn handler_address(&self) -> usize {
        match &self.disposition {
            SignalDisposition::Default => 0,
            SignalDisposition::Ignore => 1,
            SignalDisposition::Handler(address) => *address,
        }
    }
}
