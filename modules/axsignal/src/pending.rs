use core::array;

use alloc::collections::vec_deque::VecDeque;

use crate::{SignalInfo, SignalSet};

/// Structure to record pending signals
///
/// Manages a queue of signals that have been delivered to a process or thread
/// but have not yet been handled. This structure handles both standard signals
/// (which are coalesced - only one instance can be pending) and real-time
/// signals (which are queued - multiple instances can be pending).
///
/// Note: The term "pending" here refers to any signal that has been delivered
/// and not yet handled, which differs from the Linux kernel's definition of
/// "pending signals" (which refers specifically to delivered but blocked signals).
pub struct PendingSignals {
    /// The set of signals that have pending instances
    ///
    /// This is a bitmask indicating which signal numbers have at least
    /// one pending instance waiting to be handled.
    pub set: SignalSet,

    /// Signal info for standard signals (1-31)
    ///
    /// For standard signals, at most one instance can be pending at a time.
    /// If a standard signal is delivered while an instance is already pending,
    /// the new signal is discarded (coalesced).
    info_std: [Option<SignalInfo>; 32],

    /// Signal info queues for real-time signals (32-64)
    ///
    /// Real-time signals can have multiple instances queued. Each real-time
    /// signal number has its own queue to maintain delivery order.
    info_rt: [VecDeque<SignalInfo>; 33],
}

impl PendingSignals {
    /// Creates a new empty pending signals structure
    pub fn new() -> Self {
        Self {
            set: SignalSet::default(),
            info_std: Default::default(),
            info_rt: array::from_fn(|_| VecDeque::new()),
        }
    }

    /// Puts a signal into the pending queue
    ///
    /// For standard signals (1-31), only one instance can be pending at a time.
    /// If a standard signal is already pending, this function returns `false`
    /// and the new signal is ignored (coalesced).
    ///
    /// For real-time signals (32-64), multiple instances can be queued and
    /// this function always returns `true`.
    ///
    /// # Returns
    /// `true` if the signal was added, `false` if it was a standard signal
    /// that was already pending and thus ignored.
    pub fn put_signal(&mut self, sig: SignalInfo) -> bool {
        let signo = sig.signo();
        let added = self.set.add(signo);

        if signo.is_realtime() {
            self.info_rt[signo as usize - 32].push_back(sig);
        } else {
            if !added {
                // At most one standard signal can be pending.
                return false;
            }
            self.info_std[signo as usize] = Some(sig);
        }
        trace!("put_signal: {:?}", signo);
        true
    }

    /// Dequeues the next pending signal contained in `mask`, if any
    ///
    /// This function looks for the lowest-numbered signal that is both
    /// pending and included in the provided mask, removes it from the
    /// pending set, and returns its signal information.
    ///
    /// For real-time signals, if multiple instances are queued, only
    /// the first one is removed. The signal remains in the pending set
    /// if more instances are still queued.
    ///
    /// # Arguments
    /// * `mask` - A signal set indicating which signals can be dequeued
    ///
    /// # Returns
    /// The signal information for the dequeued signal, or `None` if no
    /// matching signals are pending.
    pub fn dequeue_signal(&mut self, mask: &SignalSet) -> Option<SignalInfo> {
        self.set.dequeue(mask).and_then(|signo| {
            if signo.is_realtime() {
                let queue = &mut self.info_rt[signo as usize - 32];
                let result = queue.pop_front();
                if !queue.is_empty() {
                    self.set.add(signo);
                }
                result
            } else {
                self.info_std[signo as usize].take()
            }
        })
    }
}
