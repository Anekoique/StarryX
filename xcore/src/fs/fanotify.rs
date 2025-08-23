use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use axsync::Mutex;
use lazy_static::lazy_static;
use xutils::ctypes::fs::FanEventMask;

use crate::fs::fd::FanotifyGroup;

lazy_static! {
    pub static ref FANOTIFY_GROUPS: FanManager = FanManager::new();
}

pub struct FanManager {
    groups: Mutex<Vec<Weak<FanotifyGroup>>>,
}

impl FanManager {
    pub fn new() -> Self {
        Self {
            groups: Mutex::new(Vec::new()),
        }
    }

    pub fn register_group(&self, group: Arc<FanotifyGroup>) {
        self.cleanup_groups();
        self.groups.lock().push(Arc::downgrade(&group));
    }

    pub fn cleanup_groups(&self) {
        let mut groups = self.groups.lock();
        groups.retain(|weak| weak.upgrade().is_some());
    }

    pub fn notify_event(&self, mask: FanEventMask, fd: i32, inode: u64, device: Option<u64>) {
        self.cleanup_groups();
        let groups = self.groups.lock();
        for weak_group in groups.iter() {
            if let Some(group) = weak_group.upgrade() {
                let _ = group.generate_event(mask, fd, inode);
                if let Some(dev) = device {
                    let _ = group.generate_mount_event(mask, fd, dev);
                }
            }
        }
    }
}

pub fn notify_access(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::ACCESS, fd, inode, device);
}

pub fn notify_modify(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::MODIFY, fd, inode, device);
}

pub fn notify_open(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::OPEN, fd, inode, device);
}

pub fn notify_close_write(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::CLOSE_WRITE, fd, inode, device);
}

pub fn notify_close_nowrite(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::CLOSE_NOWRITE, fd, inode, device);
}

pub fn notify_create(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::CREATE, fd, inode, device);
}

pub fn notify_delete(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::DELETE, fd, inode, device);
}

pub fn notify_move_from(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::MOVED_FROM, fd, inode, device);
}

pub fn notify_move_to(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::MOVED_TO, fd, inode, device);
}

pub fn notify_attrib(fd: i32, inode: u64, device: Option<u64>) {
    FANOTIFY_GROUPS.notify_event(FanEventMask::ATTRIB, fd, inode, device);
}

pub fn register_group(group: Arc<FanotifyGroup>) {
    FANOTIFY_GROUPS.register_group(group);
}
