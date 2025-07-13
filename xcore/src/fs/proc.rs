use alloc::{format, string::String, string::ToString, sync::Arc};
use axfs_ng_vfs::Filesystem;
use axsync::RawMutex;

use super::{
    virt_file::VirtFile,
    virt_fs::{DirMaker, VirtDir, VirtFs},
};
use crate::task::api::{with_current, with_xprocess};

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

/// Create a virtual file that reads data dynamically
fn create_dynamic_file(fs: Arc<VirtFs>, content_fn: fn() -> String) -> Arc<VirtFile> {
    VirtFile::new(fs, content_fn)
}

/// Create a static virtual file
fn create_static_file(fs: Arc<VirtFs>, content: &'static str) -> Arc<VirtFile> {
    VirtFile::new(fs, move || content)
}

/// Get current process executable path
fn get_current_exe() -> String {
    with_xprocess(|proc| proc.exe_path.read().clone())
}

/// Get current process command line  
fn get_current_cmdline() -> String {
    "starry_kernel\0".to_string()
}

/// Get current process name
fn get_current_name() -> String {
    "starry_kernel".to_string()
}

/// Get current process status
fn get_current_status() -> String {
    let pid = with_current(|curr| curr.id().as_u64());
    let name = get_current_name();

    format!(
        "Name:\t{}\n\
         State:\tR (running)\n\
         Tgid:\t{}\n\
         Ngid:\t0\n\
         Pid:\t{}\n\
         PPid:\t1\n\
         TracerPid:\t0\n\
         Uid:\t0\t0\t0\t0\n\
         Gid:\t0\t0\t0\t0\n\
         FDSize:\t256\n\
         Groups:\t\n\
         VmPeak:\t    8192 kB\n\
         VmSize:\t    8192 kB\n\
         VmLck:\t       0 kB\n\
         VmPin:\t       0 kB\n\
         VmHWM:\t    1024 kB\n\
         VmRSS:\t    1024 kB\n\
         VmData:\t    2048 kB\n\
         VmStk:\t     132 kB\n\
         VmExe:\t     512 kB\n\
         VmLib:\t    1024 kB\n\
         VmPTE:\t      32 kB\n\
         VmSwap:\t       0 kB\n\
         Threads:\t1\n\
         SigQ:\t0/32768\n\
         SigPnd:\t0000000000000000\n\
         ShdPnd:\t0000000000000000\n\
         SigBlk:\t0000000000000000\n\
         SigIgn:\t0000000000000000\n\
         SigCgt:\t0000000000000000\n\
         CapInh:\t0000000000000000\n\
         CapPrm:\tffffffffffffffff\n\
         CapEff:\tffffffffffffffff\n\
         CapBnd:\tffffffffffffffff\n\
         CapAmb:\t0000000000000000\n\
         Seccomp:\t0\n\
         Cpus_allowed:\t1\n\
         Cpus_allowed_list:\t0\n\
         Mems_allowed:\t1\n\
         Mems_allowed_list:\t0\n\
         voluntary_ctxt_switches:\t0\n\
         nonvoluntary_ctxt_switches:\t0\n",
        name, pid, pid
    )
}

/// Create the root /proc directory structure
fn create_proc_root(fs: Arc<VirtFs>) -> DirMaker {
    let mut root = VirtDir::builder(fs.clone());

    // Add standard /proc entries
    root.add("meminfo", create_static_file(fs.clone(), DUMMY_MEMINFO))
        .add("cpuinfo", create_static_file(fs.clone(), DUMMY_CPUINFO))
        .add(
            "version",
            create_static_file(fs.clone(), "StarryOS version 1.0.0\n"),
        )
        .add(
            "uptime",
            create_dynamic_file(fs.clone(), || "1234.56 1200.34\n".to_string()),
        )
        .add(
            "loadavg",
            create_static_file(fs.clone(), "0.00 0.00 0.00 1/64 1\n"),
        )
        .add(
            "mounts",
            create_static_file(
                fs.clone(),
                "proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n\
             devtmpfs /dev devtmpfs rw,nosuid,relatime 0 0\n\
             tmpfs /tmp tmpfs rw,relatime 0 0\n",
            ),
        );

    // Create /proc/self directory separately
    let mut self_dir = VirtDir::builder(fs.clone());
    self_dir
        .add("exe", VirtFile::new_symlink(fs.clone(), get_current_exe))
        .add(
            "cmdline",
            create_dynamic_file(fs.clone(), get_current_cmdline),
        )
        .add(
            "status",
            create_dynamic_file(fs.clone(), get_current_status),
        )
        .add("comm", create_dynamic_file(fs.clone(), get_current_name));

    // Add the built self directory to root
    root.add("self", self_dir.build());

    root.build()
}
