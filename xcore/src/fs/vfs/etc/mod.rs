use alloc::{string::ToString, sync::Arc};

use axfs_ng_vfs::Filesystem;
use axsync::RawMutex;

use super::{
    virt_file::{DirMaker, VirtDir, VirtFile},
    virt_fs::VirtFs,
};

const PASSWD_CONTENT: &str = concat!(
    "root:x:0:0:root:/root:/bin/bash\n",
    "nobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin\n",
);
const PROTOCOLS_CONTENT: &str = concat!(
    "ip      0       IP\n",
    "icmp    1       ICMP\n",
    "tcp     6       TCP\n",
    "udp     17      UDP\n",
);

/// Initialize the /etc filesystem as a virtual filesystem.
pub fn init_etcfs() -> Filesystem<RawMutex> {
    VirtFs::new_with("etcfs".into(), 0x657463, create_etc_root) // magic number for 'etc'
}

/// Create the root /etc directory structure.
fn create_etc_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::<()>::builder(fs.clone(), None);
    root.add(
        "passwd",
        VirtFile::new(fs.clone(), || PASSWD_CONTENT.to_string()),
    )
    .add(
        "protocols",
        VirtFile::new(fs.clone(), || PROTOCOLS_CONTENT.to_string()),
    );
    root.build()
}
