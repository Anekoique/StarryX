use alloc::{string::String, string::ToString, sync::Arc};

use axfs_ng_vfs::Filesystem;
use axprocess::Process;
use axsync::RawMutex;

use crate::{
    fs::{
        virt_file::{DirMaker, VirtDir, VirtDirOps, VirtFile},
        virt_fs::{VirtFs, VirtNodeOps},
    },
    task::{XProcess, get_process, processes, with_current},
};

/// Dummy memory information (Linux-style /proc/meminfo)
const DUMMY_MEMINFO: &str = r#"MemTotal:        8192000 kB
MemFree:         6144000 kB
MemAvailable:    6144000 kB
Buffers:               0 kB
Cached:          1024000 kB
SwapCached:            0 kB
Active:          1536000 kB
Inactive:         512000 kB
Active(anon):    1024000 kB
Inactive(anon):   256000 kB
Active(file):     512000 kB
Inactive(file):   256000 kB
Unevictable:           0 kB
Mlocked:               0 kB
SwapTotal:             0 kB
SwapFree:              0 kB
Dirty:                 0 kB
Writeback:             0 kB
AnonPages:       1024000 kB
Mapped:           512000 kB
Shmem:            256000 kB
KReclaimable:     128000 kB
Slab:             256000 kB
SReclaimable:     128000 kB
SUnreclaim:       128000 kB
KernelStack:       16000 kB
PageTables:        32000 kB
NFS_Unstable:          0 kB
Bounce:                0 kB
WritebackTmp:          0 kB
CommitLimit:     4096000 kB
Committed_AS:    2048000 kB
VmallocTotal:   34359738367 kB
VmallocUsed:           0 kB
VmallocChunk:          0 kB
Percpu:             1024 kB
HardwareCorrupted:     0 kB
AnonHugePages:         0 kB
ShmemHugePages:        0 kB
ShmemPmdMapped:        0 kB
FileHugePages:         0 kB
FilePmdMapped:         0 kB
HugePages_Total:       0
HugePages_Free:        0
HugePages_Rsvd:        0
HugePages_Surp:        0
Hugepagesize:       2048 kB
Hugetlb:               0 kB
DirectMap4k:     8388608 kB
DirectMap2M:           0 kB
DirectMap1G:           0 kB
"#;

/// Dummy CPU information (Linux-style /proc/cpuinfo)
const DUMMY_CPUINFO: &str = r#"processor	: 0
vendor_id	: StarryOS
cpu family	: 6
model		: 42
model name	: Virtual CPU @ 2.4GHz
stepping	: 1
microcode	: 0x1
cpu MHz		: 2400.000
cache size	: 256 KB
physical id	: 0
siblings	: 1
core id		: 0
cpu cores	: 1
apicid		: 0
initial apicid	: 0
fpu		: yes
fpu_exception	: yes
cpuid level	: 4
wp		: yes
flags		: fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ht syscall nx rdtscp lm rep_good nopl xtopology cpuid pni cx16 x2apic movbe popcnt aes xsave avx rdrand hypervisor lahf_lm abm 3dnowprefetch fsgsbase avx2 invpcid rdseed clflushopt
bogomips	: 4800.00
clflush size	: 64
cache_alignment	: 64
address sizes	: 40 bits physical, 48 bits virtual
power management:

"#;

/// Initialize the procfs filesystem
pub fn init_procfs() -> Filesystem<RawMutex> {
    VirtFs::new_with("procfs".into(), 0x9fa0, create_proc_root)
}

struct ProcOps(Arc<VirtFs>);

impl VirtDirOps for ProcOps {
    fn read_dir(&self) -> impl Iterator<Item = String> {
        processes().into_iter().map(|proc| proc.pid().to_string())
    }

    fn lookup(&self, name: &str) -> Option<VirtNodeOps> {
        let pid = name.parse::<u32>().ok()?;
        get_process(pid)
            .ok()
            .map(|proc| VirtNodeOps::Dir(create_proc_pid(self.0.clone(), proc)))
    }
}

/// Create the root /proc directory structure
fn create_proc_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::builder(fs.clone(), Some(Arc::new(ProcOps(fs.clone()))));

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
        VirtFile::new(fs.clone(), || "StarryOS version 1.0.0\n".to_string()),
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
        VirtFile::new(fs.clone(), || {
            "proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n\
                 devtmpfs /dev devtmpfs rw,nosuid,relatime 0 0\n\
                 tmpfs /tmp tmpfs rw,relatime 0 0\n"
                .to_string()
        }),
    )
    .add(
        "self",
        VirtFile::new_symlink(fs.clone(), || {
            with_current(|curr| curr.id().as_u64().to_string()) // 只返回 PID，不带 /proc/
        }),
    );
    root.build()
}

fn create_proc_pid(fs: Arc<VirtFs>, proc: Arc<Process>) -> DirMaker {
    let mut root = VirtDir::<()>::builder(fs.clone(), None);
    let xproc = XProcess::from_process_static(&proc);

    root.add(
        "exe",
        VirtFile::new_symlink(fs.clone(), || xproc.exe_path.read().to_string()),
    );

    root.build()
}
