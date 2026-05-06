use core::{
    array,
    ops::{Index, IndexMut},
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::sync::Arc;
use lock_api::{Mutex, RawMutex};

use crate::{PendingSignals, SignalAction, SignalInfo, SignalSet, Signo};

use super::WaitQueue;

/// Signal actions for a process.
pub struct SignalActions(pub(crate) [SignalAction; 64]);
impl Default for SignalActions {
    fn default() -> Self {
        Self(array::from_fn(|_| SignalAction::default()))
    }
}
impl Index<Signo> for SignalActions {
    type Output = SignalAction;
    fn index(&self, signo: Signo) -> &SignalAction {
        &self.0[signo as usize - 1]
    }
}
impl IndexMut<Signo> for SignalActions {
    fn index_mut(&mut self, signo: Signo) -> &mut SignalAction {
        &mut self.0[signo as usize - 1]
    }
}

/// Process-level signal manager.
pub struct ProcessSignalManager<M, WQ> {
    /// The process-level shared pending signals
    pending: Mutex<M, PendingSignals>,

    /// The signal actions
    pub actions: Arc<Mutex<M, SignalActions>>,

    /// The wait queue for signal. Used by `rt_sigtimedwait`, etc.
    ///
    /// Note that this is shared by all threads in the process, so false wakeups
    /// may occur.
    pub(crate) wq: WQ,

    /// The default restorer (signal-return trampoline) address. Patched
    /// per-process by `xcore::vdso::install` after the vDSO is mapped.
    pub(crate) default_restorer: AtomicUsize,
}
impl<M: RawMutex, WQ: WaitQueue> ProcessSignalManager<M, WQ> {
    /// Creates a new process signal manager. `default_restorer` is the
    /// initial trampoline address; pass `0` if the vDSO will fill it in
    /// later.
    pub fn new(actions: Arc<Mutex<M, SignalActions>>, default_restorer: usize) -> Self {
        Self {
            pending: Mutex::new(PendingSignals::new()),
            actions,
            wq: WQ::default(),
            default_restorer: AtomicUsize::new(default_restorer),
        }
    }

    /// Update the default restorer (called by `xcore::vdso::install` once
    /// the per-process vDSO mapping is in place).
    pub fn set_default_restorer(&self, addr: usize) {
        self.default_restorer.store(addr, Ordering::Release);
    }

    /// Read the current default restorer address.
    pub(crate) fn default_restorer(&self) -> usize {
        self.default_restorer.load(Ordering::Acquire)
    }

    pub(crate) fn dequeue_signal(&self, mask: &SignalSet) -> Option<SignalInfo> {
        self.pending.lock().dequeue_signal(mask)
    }

    /// Sends a signal to the process.
    ///
    /// This sends a signal to the process-wide pending signal queue. Any thread
    /// in the process can handle this signal. For thread-specific signals,
    /// use the thread-level `send_signal` method instead.
    pub fn send_signal(&self, sig: SignalInfo) {
        self.pending.lock().put_signal(sig);
        self.wq.notify_one();
    }

    /// Gets currently pending signals.
    pub fn pending(&self) -> SignalSet {
        self.pending.lock().set
    }

    /// Suspends current task until a signal is delivered. Note that this could
    /// return early if a signal is delivered to another thread in this process.
    pub fn wait_signal(&self) {
        self.wq.wait();
    }
}
