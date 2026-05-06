use axhal::arch::TrapFrame;

use crate::{SignalSet, SignalStack};

// signal_trampoline removed; now lives in the vDSO (`__vdso_rt_sigreturn`).
// (x86_64 is not in the root build, but the file is kept arch-symmetric.)

/// Machine context for x86_64 architecture
///
/// This structure stores the complete processor state (registers) for signal handling.
/// It's compatible with the x86_64 mcontext_t structure and is used to save and restore
/// the processor state when delivering signals.
#[repr(C)]
#[derive(Clone)]
pub struct MContext {
    r8: usize,
    r9: usize,
    r10: usize,
    r11: usize,
    r12: usize,
    r13: usize,
    r14: usize,
    r15: usize,
    rdi: usize,
    rsi: usize,
    rbp: usize,
    rbx: usize,
    rdx: usize,
    rax: usize,
    rcx: usize,
    rsp: usize,
    rip: usize,
    eflags: usize,
    cs: u16,
    gs: u16,
    fs: u16,
    _pad: u16,
    err: usize,
    trapno: usize,
    oldmask: usize,
    cr2: usize,
    fpstate: usize,
    _reserved1: [usize; 8],
}

impl MContext {
    /// Creates a new machine context from a trap frame
    ///
    /// This copies the current processor state from the trap frame into
    /// the machine context structure for signal handling.
    pub fn new(tf: &TrapFrame) -> Self {
        Self {
            r8: tf.r8 as _,
            r9: tf.r9 as _,
            r10: tf.r10 as _,
            r11: tf.r11 as _,
            r12: tf.r12 as _,
            r13: tf.r13 as _,
            r14: tf.r14 as _,
            r15: tf.r15 as _,
            rdi: tf.rdi as _,
            rsi: tf.rsi as _,
            rbp: tf.rbp as _,
            rbx: tf.rbx as _,
            rdx: tf.rdx as _,
            rax: tf.rax as _,
            rcx: tf.rcx as _,
            rsp: tf.rsp as _,
            rip: tf.rip as _,
            eflags: tf.rflags as _,
            cs: tf.cs as _,
            gs: 0,
            fs: 0,
            _pad: 0,
            err: tf.error_code as _,
            trapno: tf.vector as _,
            oldmask: 0,
            cr2: 0,
            fpstate: 0,
            _reserved1: [0; 8],
        }
    }

    /// Restores the processor state from this machine context
    ///
    /// This copies the saved processor state back into the trap frame,
    /// effectively restoring the context that was active before the signal.
    pub fn restore(&self, tf: &mut TrapFrame) {
        tf.r8 = self.r8 as _;
        tf.r9 = self.r9 as _;
        tf.r10 = self.r10 as _;
        tf.r11 = self.r11 as _;
        tf.r12 = self.r12 as _;
        tf.r13 = self.r13 as _;
        tf.r14 = self.r14 as _;
        tf.r15 = self.r15 as _;
        tf.rdi = self.rdi as _;
        tf.rsi = self.rsi as _;
        tf.rbp = self.rbp as _;
        tf.rbx = self.rbx as _;
        tf.rdx = self.rdx as _;
        tf.rax = self.rax as _;
        tf.rcx = self.rcx as _;
        tf.rsp = self.rsp as _;
        tf.rip = self.rip as _;
        tf.rflags = self.eflags as _;
        tf.cs = self.cs as _;
        tf.error_code = self.err as _;
        tf.vector = self.trapno as _;
    }
}

/// User context for x86_64 signal handling
///
/// This structure represents the complete context that is saved when a signal
/// is delivered. It includes the machine context, signal mask, and stack information.
/// It's compatible with the POSIX ucontext_t structure.
#[repr(C)]
#[derive(Clone)]
pub struct UContext {
    /// Context flags (currently unused)
    pub flags: usize,
    /// Link to the next context (for context switching, currently unused)
    pub link: usize,
    /// Signal stack information
    pub stack: SignalStack,
    /// Machine-specific processor context
    pub mcontext: MContext,
    /// Signal mask that was active before the signal
    pub sigmask: SignalSet,
}

impl UContext {
    /// Creates a new user context from a trap frame and signal mask
    ///
    /// # Arguments
    /// * `tf` - The trap frame containing the current processor state
    /// * `sigmask` - The signal mask that should be restored after signal handling
    pub fn new(tf: &TrapFrame, sigmask: SignalSet) -> Self {
        Self {
            flags: 0,
            link: 0,
            stack: SignalStack::default(),
            mcontext: MContext::new(tf),
            sigmask,
        }
    }
}
