use alloc::{
    format,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};

use axfs_ng_vfs::{
    DirEntry, DirEntrySink, DirNode, DirNodeOps, Filesystem, FilesystemOps, Metadata,
    MetadataUpdate, NodeOps, NodePermission, NodeType, Reference, VfsError, VfsResult,
};
use axsync::RawMutex;
use core::{str::FromStr, sync::atomic::Ordering};
use inherit_methods_macro::inherit_methods;

use crate::fs::{
    virt_file::{RwFile, VirtFile, VirtFileOperation},
    virt_fs::{DirMaker, VirtDir, VirtFs, VirtNode, VirtNodeOps},
};
use crate::task::{
    api::{with_current, with_thread, with_xprocess},
    proc::XThread,
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

/// Trait for getting process information - should be implemented by your process management system
pub trait ProcessInfo {
    fn get_all_pids() -> Vec<u64>;
    fn get_process_name(pid: u64) -> Option<String>;
    #[allow(dead_code)]
    fn get_process_exe_path(pid: u64) -> Option<String>;
    #[allow(dead_code)]
    fn get_process_cmdline(pid: u64) -> Option<String>;
    #[allow(dead_code)]
    fn get_process_status(pid: u64) -> Option<String>;
    fn process_exists(pid: u64) -> bool;
}

/// Default implementation for current process - you should replace this with actual process management
pub struct DefaultProcessInfo;

impl ProcessInfo for DefaultProcessInfo {
    fn get_all_pids() -> Vec<u64> {
        // Return current process ID and some dummy PIDs for demonstration
        vec![1, with_current(|curr| curr.id().as_u64())]
    }

    fn get_process_name(pid: u64) -> Option<String> {
        if Self::process_exists(pid) {
            Some(if pid == 1 {
                "init".to_string()
            } else {
                "starry_kernel".to_string()
            })
        } else {
            None
        }
    }

    fn get_process_exe_path(pid: u64) -> Option<String> {
        if Self::process_exists(pid) {
            Some(if pid == 1 {
                "/sbin/init".to_string()
            } else {
                get_current_exe()
            })
        } else {
            None
        }
    }

    fn get_process_cmdline(pid: u64) -> Option<String> {
        if Self::process_exists(pid) {
            Some(if pid == 1 {
                "init\0".to_string()
            } else {
                get_current_cmdline()
            })
        } else {
            None
        }
    }

    fn get_process_status(pid: u64) -> Option<String> {
        if Self::process_exists(pid) {
            Some(get_process_status_for_pid(pid))
        } else {
            None
        }
    }

    fn process_exists(pid: u64) -> bool {
        // Simple check - in real implementation, check your process table
        pid == 1 || pid == with_current(|curr| curr.id().as_u64())
    }
}

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
    get_process_status_for_pid(pid)
}

/// Get process status for a specific PID
fn get_process_status_for_pid(pid: u64) -> String {
    let name = DefaultProcessInfo::get_process_name(pid).unwrap_or_else(|| "unknown".to_string());

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

/// Dynamic directory that shows all process PIDs
pub struct DynamicProcRoot {
    node: VirtNode,
    this: axfs_ng_vfs::WeakDirEntry<RawMutex>,
    fs: Arc<VirtFs>,
}

impl DynamicProcRoot {
    fn new(fs: Arc<VirtFs>, this: axfs_ng_vfs::WeakDirEntry<RawMutex>) -> Arc<Self> {
        let node = VirtNode::new(
            fs.clone(),
            axfs_ng_vfs::NodeType::Directory,
            axfs_ng_vfs::NodePermission::from_bits_truncate(0o755),
        );

        Arc::new(Self { node, this, fs })
    }

    /// Get static entries (non-PID entries)
    fn get_static_entries(&self) -> Vec<(&'static str, VirtNodeOps)> {
        vec![
            (
                "meminfo",
                VirtNodeOps::from(create_static_file(self.fs.clone(), DUMMY_MEMINFO)),
            ),
            (
                "cpuinfo",
                VirtNodeOps::from(create_static_file(self.fs.clone(), DUMMY_CPUINFO)),
            ),
            (
                "version",
                VirtNodeOps::from(create_static_file(
                    self.fs.clone(),
                    "StarryOS version 1.0.0\n",
                )),
            ),
            (
                "uptime",
                VirtNodeOps::from(create_dynamic_file(self.fs.clone(), || {
                    "1234.56 1200.34\n".to_string()
                })),
            ),
            (
                "loadavg",
                VirtNodeOps::from(create_static_file(
                    self.fs.clone(),
                    "0.00 0.00 0.00 1/64 1\n",
                )),
            ),
            (
                "mounts",
                VirtNodeOps::from(create_static_file(
                    self.fs.clone(),
                    "proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n\
                 devtmpfs /dev devtmpfs rw,nosuid,relatime 0 0\n\
                 tmpfs /tmp tmpfs rw,relatime 0 0\n",
                )),
            ),
            (
                "self",
                VirtNodeOps::from(create_proc_pid_dir(self.fs.clone(), true)),
            ),
        ]
    }

    /// Check if a name is a PID (numeric string)
    fn is_pid_name(name: &str) -> bool {
        name.chars().all(|c| c.is_ascii_digit())
    }
}

#[inherit_methods(from = "self.node")]
impl NodeOps<RawMutex> for DynamicProcRoot {
    fn inode(&self) -> u64;
    fn metadata(&self) -> VfsResult<Metadata>;
    fn update_metadata(&self, update: MetadataUpdate) -> VfsResult<()>;
    fn filesystem(&self) -> &dyn FilesystemOps<RawMutex>;
    fn sync(&self, data_only: bool) -> VfsResult<()>;
    fn into_any(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }
    fn len(&self) -> VfsResult<u64> {
        Ok(0)
    }
}

impl DirNodeOps<RawMutex> for DynamicProcRoot {
    fn read_dir(&self, offset: u64, sink: &mut dyn DirEntrySink) -> VfsResult<usize> {
        use axfs_ng_vfs::path::{DOT, DOTDOT};

        let this_entry = self.this.upgrade().unwrap();

        // Collect all entries: special entries + static entries + PID entries
        let mut all_entries = Vec::new();

        // Add . and ..
        all_entries.push((
            DOT.to_string(),
            this_entry.metadata()?.inode,
            NodeType::Directory,
        ));
        all_entries.push((
            DOTDOT.to_string(),
            this_entry
                .parent()
                .map_or_else(|| this_entry.metadata(), |parent| parent.metadata())?
                .inode,
            NodeType::Directory,
        ));

        // Add static entries
        for (name, _) in self.get_static_entries() {
            // Get metadata from the actual file
            match self.lookup(name) {
                Ok(entry) => {
                    let metadata = entry.metadata()?;
                    all_entries.push((name.to_string(), metadata.inode, metadata.node_type));
                }
                Err(_) => continue,
            }
        }

        // Add PID entries
        for pid in DefaultProcessInfo::get_all_pids() {
            let pid_name = pid.to_string();

            match self.lookup(&pid_name) {
                Ok(entry) => {
                    let metadata = entry.metadata()?;
                    all_entries.push((pid_name, metadata.inode, metadata.node_type));
                }
                Err(_) => continue,
            }
        }

        // Skip to offset and send entries
        let entries_to_send = all_entries.into_iter().enumerate().skip(offset as usize);

        let mut count = 0;
        for (i, (name, inode, node_type)) in entries_to_send {
            if !sink.accept(&name, inode, node_type, i as u64 + 1) {
                break;
            }
            count += 1;
        }

        Ok(count)
    }

    fn lookup(&self, name: &str) -> VfsResult<DirEntry<RawMutex>> {
        // Check static entries first
        for (static_name, ops) in self.get_static_entries() {
            if static_name == name {
                let reference = Reference::new(self.this.upgrade(), name.to_string());
                return Ok(match ops {
                    VirtNodeOps::Dir(maker) => {
                        DirEntry::new_dir(|this| DirNode::new(maker(this)), reference)
                    }
                    VirtNodeOps::File(ops) => {
                        let node_type = ops.metadata()?.node_type;
                        DirEntry::new_file(axfs_ng_vfs::FileNode::new(ops), node_type, reference)
                    }
                });
            }
        }

        // Check if it's a PID
        if Self::is_pid_name(name) {
            if let Ok(pid) = u64::from_str(name) {
                if DefaultProcessInfo::process_exists(pid) {
                    let reference = Reference::new(self.this.upgrade(), name.to_string());
                    let maker = create_proc_pid_dir(self.fs.clone(), false);
                    return Ok(DirEntry::new_dir(
                        |this| DirNode::new(maker(this)),
                        reference,
                    ));
                }
            }
        }

        Err(VfsError::ENOENT)
    }

    fn create(
        &self,
        _name: &str,
        _node_type: NodeType,
        _permission: NodePermission,
    ) -> VfsResult<DirEntry<RawMutex>> {
        Err(VfsError::EROFS)
    }

    fn link(&self, _name: &str, _node: &DirEntry<RawMutex>) -> VfsResult<DirEntry<RawMutex>> {
        Err(VfsError::EROFS)
    }

    fn unlink(&self, _name: &str) -> VfsResult<()> {
        Err(VfsError::EROFS)
    }

    fn rename(
        &self,
        _src_name: &str,
        _dst_dir: &axfs_ng_vfs::DirNode<RawMutex>,
        _dst_name: &str,
    ) -> VfsResult<()> {
        Err(VfsError::EROFS)
    }
}

/// Create the root /proc directory structure with dynamic PID support
fn create_proc_root(fs: Arc<VirtFs>) -> DirMaker {
    Arc::new(move |this| DynamicProcRoot::new(fs.clone(), this))
}

/// Create the /proc/[pid] directory structure
fn create_proc_pid_dir(fs: Arc<VirtFs>, is_self: bool) -> DirMaker {
    let mut pid_dir = VirtDir::builder(fs.clone());

    // 公共文件
    pid_dir
        .add(
            "status",
            create_dynamic_file(fs.clone(), get_current_status),
        )
        .add(
            "oom_score_adj",
            VirtFile::new(
                fs.clone(),
                RwFile::new(move |req| {
                    let thread = with_thread(|thread| thread.clone());
                    let Some(thr_data) = thread.data::<XThread>() else {
                        return Err(VfsError::EBADF);
                    };
                    match req {
                        VirtFileOperation::Read => Ok(Some(
                            thr_data
                                .oom_score_adj
                                .load(Ordering::SeqCst)
                                .to_string()
                                .into_bytes(),
                        )),
                        VirtFileOperation::Write(data) => {
                            if !data.is_empty() {
                                let value = core::str::from_utf8(data)
                                    .ok()
                                    .and_then(|s| s.trim().parse::<i32>().ok())
                                    .ok_or(VfsError::EINVAL)?;
                                thr_data.oom_score_adj.store(value, Ordering::SeqCst);
                            }
                            Ok(None)
                        }
                    }
                }),
            ),
        )
        .add("maps", create_static_file(fs.clone(), "0\n"))
        .add("task", create_static_file(fs.clone(), "0\n"))
        .add("stat", create_static_file(fs.clone(), "0\n"))
        .add(
            "statm",
            create_static_file(fs.clone(), "1024 512 256 128 0 896 0\n"),
        )
        .add(
            "cmdline",
            create_dynamic_file(fs.clone(), get_current_cmdline),
        )
        .add("comm", create_dynamic_file(fs.clone(), get_current_name))
        .add(
            "environ",
            create_static_file(fs.clone(), "PATH=/bin:/usr/bin\0HOME=/root\0TERM=xterm\0\0"),
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

    // /proc/self 额外文件
    if is_self {
        pid_dir.add("exe", VirtFile::new_symlink(fs.clone(), get_current_exe));
    }

    pid_dir.build()
}
