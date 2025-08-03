use alloc::{
    borrow::Cow,
    format,
    string::{String, ToString},
    sync::Arc,
};

use axfs_ng_vfs::Filesystem;
use axprocess::Thread;
use axsync::RawMutex;

use crate::{
    fs::{
        virt_file::{DirMaker, VirtDir, VirtDirBuilder, VirtDirOps, VirtFile},
        virt_fs::{VirtFs, VirtNodeOps},
    },
    task::{XProcess, get_thread, processes, with_current},
};

use super::dummy::*;

/// Initialize the procfs filesystem
pub fn init_procfs() -> Filesystem<RawMutex> {
    VirtFs::new_with("procfs".into(), 0x9fa0, create_proc_root)
}

struct ProcPidOps(Arc<VirtFs>);

impl VirtDirOps for ProcPidOps {
    fn read_dir(&self) -> impl Iterator<Item = Cow<str>> {
        processes()
            .into_iter()
            .map(|proc| Cow::Owned(proc.pid().to_string()))
    }

    fn lookup(&self, name: &str) -> Option<VirtNodeOps> {
        let tid = name.parse::<u32>().ok()?;
        get_thread(tid)
            .ok()
            .map(|thread| VirtNodeOps::Dir(create_tid_root(self.0.clone(), thread)))
    }
}

/// Create the root /proc directory structure
fn create_proc_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::builder(fs.clone(), Some(Arc::new(ProcPidOps(fs.clone()))));
    let sys_root = create_sys_root(fs.clone());

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

fn create_tid_root(fs: Arc<VirtFs>, thread: Arc<Thread>) -> DirMaker {
    let mut root = VirtDir::<()>::builder(fs.clone(), None);
    let xproc = XProcess::from_thread_static(&thread);

    root.add(
        "exe",
        VirtFile::new_symlink(fs.clone(), || xproc.exe_path.read().to_string()),
    );

    root.build()
}

fn create_sys_root(fs: Arc<VirtFs>) -> VirtDirBuilder<()> {
    let mut root = VirtDir::<()>::builder(fs.clone(), None);
    let kernel_root = create_kernel_root(fs.clone());

    root.add("kernel", kernel_root.build());

    root
}

fn create_kernel_root(fs: Arc<VirtFs>) -> VirtDirBuilder<()> {
    let mut root = VirtDir::<()>::builder(fs.clone(), None);

    // Add kernel parameters commonly found in /proc/sys/kernel
    root.add(
        "pid_max",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_PID_MAX)),
    )
    .add(
        "threads-max",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_THREADS_MAX)),
    )
    .add(
        "hostname",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_HOSTNAME)),
    )
    .add(
        "domainname",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_DOMAINNAME)),
    )
    .add(
        "osrelease",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_OSRELEASE)),
    )
    .add(
        "printk",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_PRINTK)),
    )
    .add("random", create_random_root(fs.clone()).build())
    .add(
        "sysrq",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_SYSRQ)),
    )
    .add(
        "core_pattern",
        VirtFile::new(fs.clone(), || "core\n".to_string()),
    )
    .add(
        "core_uses_pid",
        VirtFile::new(fs.clone(), || "0\n".to_string()),
    )
    .add("panic", VirtFile::new(fs.clone(), || "0\n".to_string()))
    .add(
        "panic_on_oops",
        VirtFile::new(fs.clone(), || "0\n".to_string()),
    )
    .add(
        "shmmax",
        VirtFile::new(fs.clone(), || "33554432\n".to_string()),
    )
    .add(
        "shmall",
        VirtFile::new(fs.clone(), || "2097152\n".to_string()),
    )
    .add("shmmni", VirtFile::new(fs.clone(), || "4096\n".to_string()))
    .add(
        "sem",
        VirtFile::new(fs.clone(), || "250	32000	32	128\n".to_string()),
    )
    .add("msgmax", VirtFile::new(fs.clone(), || "8192\n".to_string()))
    .add(
        "msgmnb",
        VirtFile::new(fs.clone(), || "16384\n".to_string()),
    )
    .add(
        "msgmni",
        VirtFile::new(fs.clone(), || "32000\n".to_string()),
    )
    .add(
        "pid_max_limit",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_PID_MAX_LIMIT)),
    )
    .add(
        "overcommit_memory",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_OVERCOMMIT_MEMORY)),
    )
    .add(
        "hung_task_timeout_secs",
        VirtFile::new(fs.clone(), || {
            format!("{}\n", DEFAULT_HUNG_TASK_TIMEOUT_SECS)
        }),
    )
    .add(
        "sched_child_runs_first",
        VirtFile::new(fs.clone(), || {
            format!("{}\n", DEFAULT_SCHED_CHILD_RUNS_FIRST)
        }),
    );

    root
}

fn create_random_root(fs: Arc<VirtFs>) -> VirtDirBuilder<()> {
    let mut root = VirtDir::<()>::builder(fs.clone(), None);

    root.add(
        "poolsize",
        VirtFile::new(fs.clone(), || format!("{}\n", DEFAULT_RANDOM_POOLSIZE)),
    )
    .add(
        "entropy_avail",
        VirtFile::new(fs.clone(), || "3072\n".to_string()),
    )
    .add(
        "read_wakeup_threshold",
        VirtFile::new(fs.clone(), || "64\n".to_string()),
    )
    .add(
        "write_wakeup_threshold",
        VirtFile::new(fs.clone(), || "896\n".to_string()),
    )
    .add(
        "uuid",
        VirtFile::new(fs.clone(), || {
            "550e8400-e29b-41d4-a716-446655440000\n".to_string()
        }),
    )
    .add(
        "boot_id",
        VirtFile::new(fs.clone(), || {
            "550e8400-e29b-41d4-a716-446655440001\n".to_string()
        }),
    );

    root
}

fn irq_stat() -> String {
    let mut result = String::new();

    let irq_stats = axhal::irq::irq_stat();

    for (irq_num, count) in irq_stats {
        result.push_str(&format!("{}:        {}\n", irq_num, count));
    }

    result
}
