use alloc::vec::Vec;
use axfs_vfs::{
    TimesMask, VfsNodeAttr, VfsNodeOps, VfsNodeTimes, VfsResult, impl_vfs_non_dir_default,
};
use spin::RwLock;

/// The file node in the RAM filesystem.
///
/// It implements [`axfs_vfs::VfsNodeOps`].
pub struct FileNode {
    content: RwLock<Vec<u8>>,
    timestamp: RwLock<VfsNodeTimes>,
}

impl FileNode {
    pub(super) fn new() -> Self {
        Self {
            content: RwLock::new(Vec::new()),
            timestamp: RwLock::new(VfsNodeTimes::default()),
        }
    }
}

impl VfsNodeOps for FileNode {
    fn get_attr(&self) -> VfsResult<VfsNodeAttr> {
        Ok(VfsNodeAttr::new_file(
            self.content.read().len() as _,
            0,
            self.timestamp.read().clone(),
        ))
    }

    fn truncate(&self, size: u64) -> VfsResult {
        let mut content = self.content.write();
        if size < content.len() as u64 {
            content.truncate(size as _);
        } else {
            content.resize(size as _, 0);
        }
        Ok(())
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> VfsResult<usize> {
        let content = self.content.read();
        let start = content.len().min(offset as usize);
        let end = content.len().min(offset as usize + buf.len());
        let src = &content[start..end];
        buf[..src.len()].copy_from_slice(src);
        Ok(src.len())
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        let offset = offset as usize;
        let mut content = self.content.write();
        if offset + buf.len() > content.len() {
            content.resize(offset + buf.len(), 0);
        }
        let dst = &mut content[offset..offset + buf.len()];
        dst.copy_from_slice(&buf[..dst.len()]);
        Ok(buf.len())
    }

    fn set_times(&self, times: VfsNodeTimes, mask: TimesMask) -> VfsResult {
        let mut ts = self.timestamp.write();
        ts.set_times(&times, mask);
        Ok(())
    }

    impl_vfs_non_dir_default! {}
}
