use alloc::{format, string::ToString, sync::Arc};

use super::{
    super::{
        virt_file::{VirtDir, VirtDirBuilder, VirtFile},
        virt_fs::VirtFs,
    },
    dummy::*,
};

pub fn create_sys_root(fs: Arc<VirtFs>) -> VirtDirBuilder<()> {
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
