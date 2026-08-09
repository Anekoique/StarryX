#[cfg(feature = "fat")]
pub mod fat;

#[cfg(feature = "ext4")]
pub mod ext4;

use cfg_if::cfg_if;
use lock_api::RawMutex;
use xdriver::XBlockDevice;
use xvfs::{Filesystem, VfsResult};

pub fn new_default<M: RawMutex + Send + Sync + 'static>(
    dev: XBlockDevice,
) -> VfsResult<Filesystem<M>> {
    cfg_if! {
        if #[cfg(feature = "ext4")] {
            ext4::Ext4Filesystem::mount(dev)
        } else if #[cfg(feature = "fat")] {
            Ok(fat::FatFilesystem::mount(dev))
        } else {
            panic!("No filesystem feature enabled");
        }
    }
}
