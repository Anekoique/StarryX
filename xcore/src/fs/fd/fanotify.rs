use alloc::{
    collections::BTreeMap,
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::any::Any;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng_vfs::{Mountpoint, NodeOps};
use axio::PollState;
use axsync::{Mutex, RawMutex};

use xutils::ctypes::{
    S_IFIFO,
    fs::{
        FanEventMask, FanInitEventFlags, FanInitFlags, FanMarkFlags, FanotifyEventMetadata, Kstat,
    },
};

use crate::fs::file::FileLike;

#[derive(Debug, Clone)]
pub enum FanTarget {
    Inode(Weak<dyn NodeOps<RawMutex>>),
    Mountpoint(Weak<Mountpoint<RawMutex>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FanKey {
    Inode(u64),
    Mountpoint(u64),
}

/// A watch entry for fanotify monitoring
#[derive(Debug, Clone)]
pub struct FanWatch {
    pub target: FanTarget,
    pub mark: FanMarkFlags,
    pub mask: FanEventMask,
    pub ignore: FanEventMask,
}

impl FanWatch {
    pub fn new(
        target: FanTarget,
        mark: FanMarkFlags,
        mask: FanEventMask,
        ignore: FanEventMask,
    ) -> Self {
        Self {
            target,
            mark,
            mask,
            ignore,
        }
    }

    pub fn set_mark(&mut self, mark: FanMarkFlags) {
        self.mark = mark;
    }

    pub fn add_mark(&mut self, mark: FanEventMask) {
        self.mask |= mark;
    }

    pub fn add_ignore(&mut self, ignore: FanEventMask) {
        self.ignore |= ignore;
    }

    pub fn remove_mark(&mut self, mark: FanEventMask) {
        self.mask &= !mark;
    }

    pub fn remove_ignore(&mut self, ignore: FanEventMask) {
        self.ignore &= !ignore;
    }
}

/// Event queue entry
#[derive(Debug, Clone)]
pub struct FanEvent {
    pub metadata: FanotifyEventMetadata,
    pub path: Option<String>,
}

/// Fanotify file descriptor implementation
pub struct FanotifyGroup {
    /// Initialization flags
    flags: FanInitFlags,
    /// Event configuration flags
    event_flags: FanInitEventFlags,
    /// Watch list - path/fd -> watch info
    watches: Mutex<BTreeMap<FanKey, Arc<Mutex<FanWatch>>>>,
    /// Event queue
    event_queue: Mutex<Vec<FanEvent>>,
    /// Whether this fd is non-blocking
    nonblocking: bool,
}

impl FanotifyGroup {
    /// Create a new FanotifyGroup instance
    pub fn new(flags: FanInitFlags, event_flags: FanInitEventFlags) -> Self {
        Self {
            flags,
            event_flags,
            watches: Mutex::new(BTreeMap::new()),
            event_queue: Mutex::new(Vec::new()),
            nonblocking: flags.contains(FanInitFlags::NONBLOCK),
        }
    }

    pub fn flags(&self) -> FanInitFlags {
        self.flags
    }

    pub fn get_watch(&self, key: FanKey) -> Option<Arc<Mutex<FanWatch>>> {
        self.watches.lock().get(&key).cloned()
    }

    pub fn add_watch(&self, key: FanKey, watch: Arc<Mutex<FanWatch>>) {
        self.watches.lock().insert(key, watch);
    }

    pub fn has_events(&self) -> bool {
        !self.event_queue.lock().is_empty()
    }

    pub fn flush_normal_entries(&self) {
        let mut watches = self.watches.lock();
        watches.retain(|_, watch| !matches!(watch.lock().target, FanTarget::Inode(_)));
    }

    pub fn flush_mount_entries(&self) {
        let mut watches = self.watches.lock();
        watches.retain(|_, watch| !matches!(watch.lock().target, FanTarget::Mountpoint(_)));
    }

    pub fn flush_filesystem_entries(&self) {
        let mut watches = self.watches.lock();
        watches.retain(|_, watch| !matches!(watch.lock().target, FanTarget::Mountpoint(_)));
    }
}

impl FileLike for FanotifyGroup {
    fn read(&self, _buf: &mut [u8]) -> LinuxResult<usize> {
        Ok(0)
    }

    fn write(&self, _buf: &[u8]) -> LinuxResult<usize> {
        Err(LinuxError::EBADF)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        Ok(Kstat {
            mode: S_IFIFO | 0o600u32,
            ..Default::default()
        })
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        Ok(PollState {
            readable: self.has_events(),
            writable: false,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) {}

    fn is_nonblocking(&self) -> bool {
        self.nonblocking
    }
}
