use core::mem;

use derive_more::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use linux_raw_sys::general::{SS_DISABLE, kernel_sigset_t, siginfo_t};
use strum_macros::FromRepr;

use crate::DefaultSignalAction;

/// Signal number enumeration
///
/// Represents all UNIX signal numbers from 1 to 64, including both standard
/// signals (1-31) and real-time signals (32-64). Each signal has a specific
/// meaning and default behavior in the system.
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromRepr)]
pub enum Signo {
    /// Hangup detected on controlling terminal or death of controlling process
    SIGHUP = 1,
    /// Interrupt from keyboard (Ctrl+C)
    SIGINT = 2,
    /// Quit from keyboard (Ctrl+\)
    SIGQUIT = 3,
    /// Illegal instruction
    SIGILL = 4,
    /// Trace/breakpoint trap
    SIGTRAP = 5,
    /// Abort signal from abort(3)
    SIGABRT = 6,
    /// Bus error (bad memory access)
    SIGBUS = 7,
    /// Floating-point exception
    SIGFPE = 8,
    /// Kill signal (cannot be caught or ignored)
    SIGKILL = 9,
    /// User-defined signal 1
    SIGUSR1 = 10,
    /// Segmentation fault (invalid memory reference)
    SIGSEGV = 11,
    /// User-defined signal 2
    SIGUSR2 = 12,
    /// Broken pipe: write to pipe with no readers
    SIGPIPE = 13,
    /// Timer signal from alarm(2)
    SIGALRM = 14,
    /// Termination signal
    SIGTERM = 15,
    /// Stack fault on coprocessor (unused)
    SIGSTKFLT = 16,
    /// Child stopped or terminated
    SIGCHLD = 17,
    /// Continue if stopped
    SIGCONT = 18,
    /// Stop process (cannot be caught or ignored)
    SIGSTOP = 19,
    /// Stop typed at terminal
    SIGTSTP = 20,
    /// Terminal input for background process
    SIGTTIN = 21,
    /// Terminal output for background process
    SIGTTOU = 22,
    /// Urgent condition on socket
    SIGURG = 23,
    /// CPU time limit exceeded
    SIGXCPU = 24,
    /// File size limit exceeded
    SIGXFSZ = 25,
    /// Virtual alarm clock
    SIGVTALRM = 26,
    /// Profiling alarm clock
    SIGPROF = 27,
    /// Window resize signal
    SIGWINCH = 28,
    /// I/O now possible
    SIGIO = 29,
    /// Power failure (System V)
    SIGPWR = 30,
    /// Bad system call (SVr4)
    SIGSYS = 31,

    // Real-time signals (32-64)
    /// Real-time signal 0 (minimum)
    SIGRTMIN = 32,
    /// Real-time signal 1
    SIGRT1 = 33,
    /// Real-time signal 2
    SIGRT2 = 34,
    /// Real-time signal 3
    SIGRT3 = 35,
    /// Real-time signal 4
    SIGRT4 = 36,
    /// Real-time signal 5
    SIGRT5 = 37,
    /// Real-time signal 6
    SIGRT6 = 38,
    /// Real-time signal 7
    SIGRT7 = 39,
    /// Real-time signal 8
    SIGRT8 = 40,
    /// Real-time signal 9
    SIGRT9 = 41,
    /// Real-time signal 10
    SIGRT10 = 42,
    /// Real-time signal 11
    SIGRT11 = 43,
    /// Real-time signal 12
    SIGRT12 = 44,
    /// Real-time signal 13
    SIGRT13 = 45,
    /// Real-time signal 14
    SIGRT14 = 46,
    /// Real-time signal 15
    SIGRT15 = 47,
    /// Real-time signal 16
    SIGRT16 = 48,
    /// Real-time signal 17
    SIGRT17 = 49,
    /// Real-time signal 18
    SIGRT18 = 50,
    /// Real-time signal 19
    SIGRT19 = 51,
    /// Real-time signal 20
    SIGRT20 = 52,
    /// Real-time signal 21
    SIGRT21 = 53,
    /// Real-time signal 22
    SIGRT22 = 54,
    /// Real-time signal 23
    SIGRT23 = 55,
    /// Real-time signal 24
    SIGRT24 = 56,
    /// Real-time signal 25
    SIGRT25 = 57,
    /// Real-time signal 26
    SIGRT26 = 58,
    /// Real-time signal 27
    SIGRT27 = 59,
    /// Real-time signal 28
    SIGRT28 = 60,
    /// Real-time signal 29
    SIGRT29 = 61,
    /// Real-time signal 30
    SIGRT30 = 62,
    /// Real-time signal 31
    SIGRT31 = 63,
    /// Real-time signal 32 (maximum)
    SIGRT32 = 64,
}

impl Signo {
    /// Returns true if this is a real-time signal (>= SIGRTMIN)
    ///
    /// Real-time signals have different queueing behavior than standard signals.
    /// Multiple instances of the same real-time signal can be queued, while
    /// standard signals are coalesced.
    pub fn is_realtime(&self) -> bool {
        *self >= Signo::SIGRTMIN
    }

    /// Returns the default action for this signal
    ///
    /// Each signal has a predefined default behavior that occurs when
    /// no custom handler is installed.
    pub fn default_action(&self) -> DefaultSignalAction {
        match self {
            Signo::SIGHUP => DefaultSignalAction::Terminate,
            Signo::SIGINT => DefaultSignalAction::Terminate,
            Signo::SIGQUIT => DefaultSignalAction::CoreDump,
            Signo::SIGILL => DefaultSignalAction::CoreDump,
            Signo::SIGTRAP => DefaultSignalAction::CoreDump,
            Signo::SIGABRT => DefaultSignalAction::CoreDump,
            Signo::SIGBUS => DefaultSignalAction::CoreDump,
            Signo::SIGFPE => DefaultSignalAction::CoreDump,
            Signo::SIGKILL => DefaultSignalAction::Terminate,
            Signo::SIGUSR1 => DefaultSignalAction::Terminate,
            Signo::SIGSEGV => DefaultSignalAction::CoreDump,
            Signo::SIGUSR2 => DefaultSignalAction::Terminate,
            Signo::SIGPIPE => DefaultSignalAction::Terminate,
            Signo::SIGALRM => DefaultSignalAction::Terminate,
            Signo::SIGTERM => DefaultSignalAction::Terminate,
            Signo::SIGSTKFLT => DefaultSignalAction::Terminate,
            Signo::SIGCHLD => DefaultSignalAction::Ignore,
            Signo::SIGCONT => DefaultSignalAction::Continue,
            Signo::SIGSTOP => DefaultSignalAction::Stop,
            Signo::SIGTSTP => DefaultSignalAction::Stop,
            Signo::SIGTTIN => DefaultSignalAction::Stop,
            Signo::SIGTTOU => DefaultSignalAction::Stop,
            Signo::SIGURG => DefaultSignalAction::Ignore,
            Signo::SIGXCPU => DefaultSignalAction::CoreDump,
            Signo::SIGXFSZ => DefaultSignalAction::CoreDump,
            Signo::SIGVTALRM => DefaultSignalAction::Terminate,
            Signo::SIGPROF => DefaultSignalAction::Terminate,
            Signo::SIGWINCH => DefaultSignalAction::Ignore,
            Signo::SIGIO => DefaultSignalAction::Terminate,
            Signo::SIGPWR => DefaultSignalAction::Terminate,
            Signo::SIGSYS => DefaultSignalAction::CoreDump,
            _ => DefaultSignalAction::Ignore,
        }
    }
}

/// Signal set - a bitmask representing a collection of signals
///
/// Compatible with `struct sigset_t` in libc. Used for signal masking,
/// pending signal tracking, and other signal set operations.
#[derive(Default, Debug, Clone, Copy, Not, BitOr, BitOrAssign, BitAnd, BitAndAssign)]
#[repr(transparent)]
pub struct SignalSet(u64);
impl SignalSet {
    /// Returns the bit position for a given signal number
    fn signo_bit(signo: Signo) -> u64 {
        1 << (signo as u8 - 1)
    }

    /// Adds a signal to the set.
    ///
    /// Returns `true` if the signal was added, `false` if it was already present.
    pub fn add(&mut self, signal: Signo) -> bool {
        let bit = Self::signo_bit(signal);
        if self.0 & bit != 0 {
            return false;
        }
        self.0 |= bit;
        true
    }

    /// Removes a signal from the set.
    ///
    /// Returns `true` if the signal was removed, `false` if it wasn't present.
    pub fn remove(&mut self, signal: Signo) -> bool {
        let bit = Self::signo_bit(signal);
        if self.0 & bit == 0 {
            return false;
        }
        self.0 &= !bit;
        true
    }

    /// Checks if the set contains a signal.
    pub fn has(&self, signal: Signo) -> bool {
        (self.0 & Self::signo_bit(signal)) != 0
    }

    /// Returns `true` if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Dequeues the first signal in `mask` from this set, if any.
    ///
    /// This finds the lowest-numbered signal that is both in this set
    /// and in the provided mask, removes it from this set, and returns it.
    pub fn dequeue(&mut self, mask: &SignalSet) -> Option<Signo> {
        let bits = self.0 & mask.0;
        if bits == 0 {
            None
        } else {
            let signal = bits.trailing_zeros();
            self.0 &= !(1 << signal);
            Signo::from_repr((signal + 1) as u8)
        }
    }

    /// Converts to C-compatible sigset_t representation
    pub fn to_ctype(&self, dest: &mut kernel_sigset_t) {
        // SAFETY: `kernel_sigset_t` always has the same layout as `[c_ulong; 1]`.
        unsafe {
            *mem::transmute::<&mut kernel_sigset_t, &mut u64>(dest) = self.0;
        }
    }
}

impl From<kernel_sigset_t> for SignalSet {
    fn from(value: kernel_sigset_t) -> Self {
        // SAFETY: `kernel_sigset_t` always has the same layout as `[c_ulong; 1]`.
        unsafe { Self(*mem::transmute::<&kernel_sigset_t, &u64>(&value)) }
    }
}

/// Signal information structure
///
/// Compatible with `struct siginfo` in libc. Contains detailed information
/// about a signal, including the signal number, code, and additional data
/// depending on the signal type.
#[derive(Clone)]
#[repr(transparent)]
pub struct SignalInfo(pub siginfo_t);

impl SignalInfo {
    /// Creates a new signal info with the given signal number and code
    pub fn new(signo: Signo, code: i32) -> Self {
        let mut result: Self = unsafe { mem::zeroed() };
        result.set_signo(signo);
        result.set_code(code);
        result
    }

    /// Returns the signal number
    pub fn signo(&self) -> Signo {
        unsafe { Signo::from_repr(self.0.__bindgen_anon_1.__bindgen_anon_1.si_signo as _).unwrap() }
    }

    /// Sets the signal number
    pub fn set_signo(&mut self, signo: Signo) {
        self.0.__bindgen_anon_1.__bindgen_anon_1.si_signo = signo as _;
    }

    /// Returns the signal code (reason for signal generation)
    pub fn code(&self) -> i32 {
        unsafe { self.0.__bindgen_anon_1.__bindgen_anon_1.si_code }
    }

    /// Sets the signal code
    pub fn set_code(&mut self, code: i32) {
        self.0.__bindgen_anon_1.__bindgen_anon_1.si_code = code;
    }
}

unsafe impl Send for SignalInfo {}
unsafe impl Sync for SignalInfo {}

/// Signal stack configuration
///
/// Compatible with `struct sigaltstack` in libc. Used to define an alternate
/// signal stack for signal handler execution. This allows signal handlers
/// to execute on a separate stack, which is useful for handling signals
/// like SIGSEGV that might be caused by stack overflow.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SignalStack {
    /// Stack pointer (base address of the stack)
    pub sp: usize,
    /// Stack flags (typically SS_DISABLE or 0)
    pub flags: u32,
    /// Size of the stack in bytes
    pub size: usize,
}

impl Default for SignalStack {
    fn default() -> Self {
        Self {
            sp: 0,
            flags: SS_DISABLE,
            size: 0,
        }
    }
}

impl SignalStack {
    /// Checks if signal stack is disabled.
    ///
    /// Returns true if the SS_DISABLE flag is set, indicating that
    /// no alternate signal stack is configured.
    pub fn disabled(&self) -> bool {
        self.flags == SS_DISABLE
    }
}
