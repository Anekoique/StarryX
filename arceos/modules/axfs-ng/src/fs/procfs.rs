
use crate::{Filesystem, FilesystemOps, StatFs, VfsResult, DirEntry, Node, NodeOps, VfsError, Inode};
use alloc::sync::Arc;
use axerrno::LinuxError;
use axlog::info;
use axtype::Path;
use axinterrupt;

pub struct ProcFs;

impl ProcFs {
    pub fn new() -> VfsResult<Self> {
        Ok(Self)
    }
}

impl<M: lock_api::RawMutex> FilesystemOps<M> for ProcFs {
    fn name(&self) -> &str {
        "procfs"
    }

    fn root_dir(&self) -> DirEntry<M> {
        let root_inode = ProcInterrupts::new();
        let root_node = Node::new(
            root_inode,
            self.name(),
            crate::VfsNodePerm::default_dir(),
        );
        DirEntry::new(None, root_node)
    }

    fn stat(&self) -> VfsResult<StatFs> {
        Ok(StatFs {
            fs_type: 0,
            block_size: 4096,
            blocks: 0,
            blocks_free: 0,
            blocks_available: 0,
            file_count: 0,
            free_file_count: 0,
            name_length: 255,
            fragment_size: 4096,
            mount_flags: 0,
        })
    }
}

pub struct ProcInterrupts;

impl ProcInterrupts {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl<M: lock_api::RawMutex> NodeOps<M> for ProcInterrupts {
    fn get_attr(&self) -> VfsResult<crate::VfsNodeAttr> {
        Ok(crate::VfsNodeAttr {
            inode_id: 0,
            dev_id: 0,
            rdev: 0,
            mode: crate::VfsNodePerm::default_file().bits(),
            nlink: 1,
            uid: 0,
            gid: 0,
            size: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
            blk_size: 0,
            blocks: 0,
        })
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        let content = axinterrupt::get_interrupt_counts();
        let content_bytes = content.as_bytes();
        let len = content_bytes.len();
        if offset >= len as u64 {
            return Ok(0);
        }
        let read_len = (len - offset as usize).min(buf.len());
        buf[..read_len].copy_from_slice(&content_bytes[offset as usize..offset as usize + read_len]);
        Ok(read_len)
    }

    fn write_at(&self, _offset: u64, _buf: &[u8]) -> VfsResult<usize> {
        Err(VfsError::PermissionDenied)
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}
