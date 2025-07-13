use alloc::sync::Arc;
use axerrno::LinuxResult;
use axfs_ng::FsContext;
use axfs_ng_vfs::{DeviceId, Filesystem, NodeType, VfsResult};
use axsync::{Mutex, RawMutex};
use rand::{RngCore, SeedableRng, rngs::SmallRng};

use super::{
    virt_file::{VirtDevice, VirtDeviceOps},
    virt_fs::{DirMaker, VirtDir, VirtFs},
};

/// The device ID for /dev/rtc0
pub const RTC0_DEVICE_ID: DeviceId = DeviceId::new(250, 0);

const RANDOM_SEED: &[u8; 32] = b"0123456789abcdef0123456789abcdef";

/// Initialize the device filesystem with common devices and mount /dev/shm
pub fn init_devfs() -> LinuxResult<Filesystem<RawMutex>> {
    let fs = VirtFs::new_with("devtmpfs".into(), 0x01021994, create_dev_root);
    let mp = axfs_ng_vfs::Mountpoint::new_root(&fs);

    // Mount /dev/shm as memory filesystem
    FsContext::new(mp.root_location())
        .resolve("/shm")?
        .mount(&super::tmp::init_tmpfs())?;

    Ok(fs)
}

/// Device operations enumeration for all supported device types
#[derive(Clone)]
enum DeviceOps {
    Null,
    Zero,
    Random(Arc<Mutex<SmallRng>>),
    Rtc,
}

impl VirtDeviceOps for DeviceOps {
    fn read_at(&self, buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        match self {
            Self::Null => Ok(0),
            Self::Zero => {
                buf.fill(0);
                Ok(buf.len())
            }
            Self::Random(rng) => {
                rng.lock().fill_bytes(buf);
                Ok(buf.len())
            }
            Self::Rtc => Ok(0),
        }
    }

    fn write_at(&self, buf: &[u8], _offset: u64) -> VfsResult<usize> {
        match self {
            Self::Null | Self::Random(_) => Ok(buf.len()),
            Self::Zero | Self::Rtc => Ok(0),
        }
    }
}

/// Macro to simplify device specification with name, major, minor, and operations
macro_rules! device_spec {
    ($name:literal, $major:expr, $minor:expr, $ops:expr) => {
        (
            $name,
            NodeType::CharacterDevice,
            DeviceId::new($major, $minor),
            $ops,
        )
    };
}

/// Helper function to add a device to the virtual directory builder
fn add_device(
    root: &mut super::virt_fs::VirtDirBuilder,
    fs: &Arc<VirtFs>,
    name: &str,
    node_type: NodeType,
    device_id: DeviceId,
    ops: DeviceOps,
) {
    root.add(name, VirtDevice::new(fs.clone(), node_type, device_id, ops));
}

/// Create the root directory structure for /dev filesystem
fn create_dev_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::builder(fs.clone());

    let devices = [
        device_spec!("null", 1, 3, DeviceOps::Null),
        device_spec!("zero", 1, 5, DeviceOps::Zero),
        device_spec!("rtc0", 250, 0, DeviceOps::Rtc),
    ];

    for (name, node_type, device_id, ops) in devices {
        add_device(&mut root, &fs, name, node_type, device_id, ops);
    }

    let random_devices = [
        ("random", DeviceId::new(1, 8)),
        ("urandom", DeviceId::new(1, 9)),
    ];

    for (name, device_id) in random_devices {
        let rng = Arc::new(Mutex::new(SmallRng::from_seed(*RANDOM_SEED)));
        add_device(
            &mut root,
            &fs,
            name,
            NodeType::CharacterDevice,
            device_id,
            DeviceOps::Random(rng),
        );
    }

    root.add("shm", VirtDir::builder(fs).build());

    root.build()
}
