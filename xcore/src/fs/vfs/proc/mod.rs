mod dummy;
mod pid;
mod sys;

use alloc::{
    format,
    string::{String, ToString},
    sync::Arc,
};

use axfs_ng_vfs::Filesystem;
use axsync::RawMutex;

use self::dummy::*;
use super::{
    virt_file::{DirMaker, VirtDir, VirtFile},
    virt_fs::VirtFs,
};
use crate::task::with_current;

/// Initialize the procfs filesystem
pub fn init_procfs() -> Filesystem<RawMutex> {
    VirtFs::new_with("procfs".into(), 0x9fa0, create_proc_root)
}

/// Create the root /proc directory structure
fn create_proc_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::builder(fs.clone(), Some(Arc::new(pid::ProcPidOps(fs.clone()))));
    let sys_root = sys::create_sys_root(fs.clone());

    // Add standard /proc entries
    root.add(
        "meminfo",
        VirtFile::new(fs.clone(), || DUMMY_MEMINFO.to_string()),
    )
    .add(
        "cpuinfo",
        VirtFile::new(fs.clone(), || DUMMY_CPUINFO.to_string()),
    )
    .add(
        "version",
        VirtFile::new(fs.clone(), || "StarryX version 1.0.0\n".to_string()),
    )
    .add(
        "uptime",
        VirtFile::new(fs.clone(), || "1234.56 1200.34\n".to_string()),
    )
    .add(
        "loadavg",
        VirtFile::new(fs.clone(), || "0.00 0.00 0.00 1/64 1\n".to_string()),
    )
    .add(
        "mounts",
        VirtFile::new(fs.clone(), || DUMMY_MOUNTINFO.to_string()),
    )
    .add("interrupts", VirtFile::new(fs.clone(), irq_stat))
    .add(
        "self",
        VirtFile::new_symlink(fs.clone(), || {
            with_current(|curr| curr.id().as_u64().to_string())
        }),
    )
    .add("sys", sys_root.build());
    root.build()
}

fn irq_stat() -> String {
    let mut result = String::new();

    let irq_stats = axhal::irq::irq_stat();

    for (irq_num, count) in irq_stats {
        result.push_str(&format!("{}:        {}\n", irq_num, count));
    }

    result
}
