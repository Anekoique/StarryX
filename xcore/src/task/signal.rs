use core::time::Duration;

use axerrno::{LinuxError, LinuxResult};
use axsync::RawMutex;
use axtask::WaitQueue;

use xprocess::{Process, ProcessGroup, Thread};
use xsignal::{
    SignalInfo,
    api::{ProcessSignalManager, ThreadSignalManager},
};

use crate::task::{XProcess, XThread, with_xthread};

pub type ProcessSignal = ProcessSignalManager<RawMutex, WaitQueueWrapper>;
pub type ThreadSignal = ThreadSignalManager<RawMutex, WaitQueueWrapper>;

pub struct WaitQueueWrapper(WaitQueue);

impl Default for WaitQueueWrapper {
    fn default() -> Self {
        Self(WaitQueue::new())
    }
}

impl xsignal::api::WaitQueue for WaitQueueWrapper {
    fn wait_timeout(&self, timeout: Option<Duration>) -> bool {
        if let Some(timeout) = timeout {
            self.0.wait_timeout(timeout)
        } else {
            self.0.wait();
            true
        }
    }

    fn notify_one(&self) -> bool {
        self.0.notify_one(false)
    }
}

pub fn send_signal_thread(thread: &Thread, sig: SignalInfo) -> LinuxResult<()> {
    info!("Send signal {:?} to thread {}", sig.signo(), thread.tid());
    let Some(thread) = thread.data::<XThread>() else {
        return Err(LinuxError::EPERM);
    };
    thread.signal.send_signal(sig);
    Ok(())
}

pub fn send_signal_process(proc: &Process, sig: SignalInfo) -> LinuxResult<()> {
    info!("Send signal {:?} to process {}", sig.signo(), proc.pid());
    let Some(proc) = proc.data::<XProcess>() else {
        return Err(LinuxError::EPERM);
    };
    proc.signal.send_signal(sig);
    Ok(())
}

pub fn send_signal_process_group(pg: &ProcessGroup, sig: SignalInfo) -> usize {
    info!(
        "Send signal {:?} to process group {}",
        sig.signo(),
        pg.pgid()
    );
    let mut count = 0;
    for proc in pg.processes() {
        count += send_signal_process(&proc, sig.clone()).is_ok() as usize;
    }
    count
}

pub fn have_signals() -> bool {
    with_xthread(|xthread| !xthread.signal.pending().is_empty())
}
