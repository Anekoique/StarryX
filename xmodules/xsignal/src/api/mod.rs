/// Process-wide signal operations
mod process;
/// Thread-specific signal operations  
mod thread;

pub use process::*;
pub use thread::*;

use core::time::Duration;

/// A wait queue abstraction for thread synchronization
///
/// This trait provides a generic interface for thread wait queues that can be
/// used for various synchronization primitives. It supports both blocking waits
/// and waits with timeouts, as well as notification mechanisms to wake up
/// waiting threads.
///
/// The wait queue is used internally by signal handling code to manage threads
/// that are waiting for signals or other synchronization events.
pub trait WaitQueue: Default {
    /// Waits for a notification, with an optional timeout
    ///
    /// This function blocks the calling thread until either:
    /// - A notification is received via `notify_one()` or `notify_all()`
    /// - The specified timeout expires (if provided)
    ///
    /// # Arguments
    /// * `timeout` - Optional timeout duration. If `None`, waits indefinitely.
    ///
    /// # Returns
    /// `true` if a notification was received, `false` if the timeout expired.
    fn wait_timeout(&self, timeout: Option<Duration>) -> bool;

    /// Waits for a notification indefinitely
    ///
    /// This is a convenience method that calls `wait_timeout(None)`.
    /// The thread will block until a notification is received.
    fn wait(&self) {
        self.wait_timeout(None);
    }

    /// Notifies one waiting thread
    ///
    /// Wakes up one thread that is currently blocked in `wait()` or `wait_timeout()`.
    /// If multiple threads are waiting, which thread is woken is implementation-defined.
    ///
    /// # Returns
    /// `true` if a thread was notified, `false` if no threads were waiting.
    fn notify_one(&self) -> bool;

    /// Notifies all waiting threads
    ///
    /// Wakes up all threads that are currently blocked in `wait()` or `wait_timeout()`.
    /// This is implemented by repeatedly calling `notify_one()` until no more
    /// threads are waiting.
    fn notify_all(&self) {
        while self.notify_one() {}
    }
}
