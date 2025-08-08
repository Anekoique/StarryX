/// Dummy memory information (Linux-style /proc/meminfo)
pub const DUMMY_MEMINFO: &str = r#"MemTotal:        8192000 kB
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
pub const DUMMY_CPUINFO: &str = r#"processor	: 0
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

pub const DUMMY_MOUNTINFO: &str = r#"proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
devtmpfs /dev devtmpfs rw,nosuid,relatime 0 0
tmpfs /tmp tmpfs rw,relatime 0 0
"#;

pub const DUMMY_MAPS: &str = r#"00400000-00401000 r-xp 00000000 08:01 1000000000000000 /bin/sh
00401000-00402000 r--p 00000000 08:01 1000000000000000 /bin/sh
"#;

/// Default PID maximum value (32768 is common on Linux)
pub const DEFAULT_PID_MAX: u32 = 32768;

/// Maximum number of threads
pub const DEFAULT_THREADS_MAX: u32 = 4096;

/// Kernel hostname
pub const DEFAULT_HOSTNAME: &str = "StarryX";

/// Kernel domain name
pub const DEFAULT_DOMAINNAME: &str = "localdomain";

/// Kernel version
pub const DEFAULT_OSRELEASE: &str = "6.1.0-starry";

/// Kernel message control
pub const DEFAULT_PRINTK: &str = "7	4	1	7";

/// Random pool control
pub const DEFAULT_RANDOM_POOLSIZE: &str = "4096";

/// SysRq control (disabled for security)
pub const DEFAULT_SYSRQ: &str = "0";

/// Default maximum number of PIDs that can be allocated
pub const DEFAULT_PID_MAX_LIMIT: u32 = 4194304;

/// Default overcommit handling
pub const DEFAULT_OVERCOMMIT_MEMORY: &str = "1";

/// Default hung task timeout
pub const DEFAULT_HUNG_TASK_TIMEOUT_SECS: &str = "120";

/// Default sched child runs first
pub const DEFAULT_SCHED_CHILD_RUNS_FIRST: &str = "0";
