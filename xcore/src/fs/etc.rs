use crate::alloc::string::ToString;
use alloc::sync::Arc;
use axfs_ng_vfs::Filesystem;
use axsync::RawMutex;

use super::{
    virt_file::VirtFile,
    virt_fs::{DirMaker, VirtDir, VirtFs},
};

const PASSWD_CONTENT: &str = "nobody:x:0:0::/musl:/bin/sh\n";

/// Initialize the /etc filesystem as a virtual filesystem.
pub fn init_etcfs() -> Filesystem<RawMutex> {
    VirtFs::new_with("etcfs".into(), 0x657463, create_etc_root) // magic number for 'etc'
}

/// Create a static virtual file.
/// The closure returns a `String` which can be converted into `Vec<u8>`.
fn create_static_file(fs: Arc<VirtFs>, content: &'static str) -> Arc<VirtFile> {
    VirtFile::new(fs, move || content.to_string())
}

/// Create the root /etc directory structure.
fn create_etc_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::builder(fs.clone());
    root.add("passwd", create_static_file(fs.clone(), PASSWD_CONTENT));
    root.build()
}
