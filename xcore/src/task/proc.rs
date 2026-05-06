//! User task management.

use alloc::{
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    alloc::Layout,
    sync::atomic::{AtomicI32, AtomicU32, AtomicUsize, Ordering},
};

use axerrno::{LinuxError, LinuxResult};
use axhal::arch::UspaceContext;
use axns::{AxNamespace, AxNamespaceIf};
use axsync::Mutex;
use axtask::{AxTaskExtIf, TaskExtRef, TaskInner, WaitQueue, current};
use inherit_methods_macro::inherit_methods;
use memory_addr::{VirtAddr, VirtAddrRange};
use spin::{Once, RwLock};

use xprocess::{Pid, Process, ProcessGroup, Session, Thread};
use xsignal::{
    Signo,
    api::{ProcessSignalManager, SignalActions, ThreadSignalManager},
};
use xuspace::{UserPtr, UserSpaceAccess, nullable};
use xutils::{collections::weak_map::WeakMap, ctypes::SCHED_RR};
use xvma::MmapRegion;

use crate::{
    mm::{FileWrapper, XUserSpace},
    resources::Rlimits,
    task::{
        FutexKey, FutexTable, ProcessSignal, ThreadSignal, cred::ProcessCredentials, with_current,
    },
    time::{TimeStat, time_stat_switch_from_old_task},
};

pub fn new_user_task(
    name: &str,
    uctx: UspaceContext,
    tid_addr: Option<&'static mut Pid>,
) -> TaskInner {
    TaskInner::new(
        move || {
            with_current(|curr| {
                if let Some(tid_addr) = tid_addr {
                    nullable!(curr.task_ext().xprocess_ref().uspace().write(
                        UserPtr::<u32>::from(tid_addr as *mut _),
                        curr.id().as_u64() as Pid
                    ))
                    .map_err(|_| panic!("Failed to write tid to user space"))
                    .ok();
                }
                let kstack_top = curr.kernel_stack_top().unwrap();
                info!(
                    "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                    uctx.ip(),
                    uctx.sp(),
                    kstack_top,
                );
                unsafe { uctx.enter_uspace(kstack_top) }
            })
        },
        name.into(),
        crate::config::KERNEL_STACK_SIZE,
    )
}

axtask::def_task_ext!(XTaskExt);

#[repr(transparent)]
pub struct XTaskExt(Arc<Thread>);

impl XTaskExt {
    pub fn new(thread: Arc<Thread>) -> Self {
        Self(thread)
    }

    pub fn from_task(task: &TaskInner) -> &'static XTaskExt {
        unsafe { &*(task.task_ext() as *const Self) }
    }

    pub fn thread_ref(&self) -> &Arc<Thread> {
        &self.0
    }

    pub fn process_ref(&self) -> &Arc<Process> {
        self.0.process()
    }

    pub fn thread(&self) -> Arc<Thread> {
        self.0.clone()
    }

    pub fn process(&self) -> Arc<Process> {
        self.0.process().clone()
    }

    pub fn xthread_ref(&self) -> &XThread {
        self.0.data().unwrap()
    }

    pub fn xthread(&self) -> &'static XThread {
        unsafe { &*(self.0.data::<XThread>().unwrap() as *const XThread) }
    }

    pub fn xprocess_ref(&self) -> &XProcess {
        self.0.process().data().unwrap()
    }

    pub fn xprocess(&self) -> &'static XProcess {
        unsafe { &*(self.0.process().data::<XProcess>().unwrap() as *const XProcess) }
    }
}

pub struct XThread {
    pub time: RwLock<TimeStat>,
    pub clear_child_tid: AtomicUsize,
    pub robust_list_head: AtomicUsize,
    pub signal: ThreadSignal,
    pub oom_score_adj: AtomicI32,
    pub futex_bitset: AtomicU32,
    pub priority: AtomicI32,
    pub policy: AtomicU32,
}

impl XThread {
    pub fn new(proc: &XProcess) -> Self {
        Self {
            time: RwLock::new(TimeStat::new()),
            clear_child_tid: AtomicUsize::new(0),
            robust_list_head: AtomicUsize::new(0),
            signal: ThreadSignalManager::new(proc.signal.clone()),
            oom_score_adj: AtomicI32::new(200),
            futex_bitset: AtomicU32::new(0),
            priority: AtomicI32::new(0),
            policy: AtomicU32::new(SCHED_RR as _),
        }
    }

    pub fn from_thread(thread: &Arc<Thread>) -> &XThread {
        thread.data::<Self>().unwrap()
    }

    pub fn from_thread_static(thread: &Arc<Thread>) -> &'static XThread {
        unsafe { &*(thread.data::<Self>().unwrap() as *const Self) }
    }

    pub fn clear_child_tid(&self) -> usize {
        self.clear_child_tid.load(Ordering::Relaxed)
    }

    pub fn set_clear_child_tid(&self, clear_child_tid: usize) {
        self.clear_child_tid
            .store(clear_child_tid, Ordering::Relaxed);
    }

    pub fn get_priority(&self) -> i32 {
        self.priority.load(Ordering::Relaxed)
    }

    pub fn set_priority(&self, priority: i32) {
        self.priority.store(priority, Ordering::Relaxed);
    }

    pub fn get_policy(&self) -> u32 {
        self.policy.load(Ordering::Relaxed)
    }

    pub fn set_policy(&self, policy: u32) {
        self.policy.store(policy, Ordering::Relaxed);
    }

    pub fn get_oom_score_adj(&self) -> i32 {
        self.oom_score_adj.load(Ordering::Relaxed)
    }

    pub fn set_oom_score_adj(&self, value: i32) -> LinuxResult<()> {
        if !(-1000..=1000).contains(&value) {
            return Err(LinuxError::EINVAL);
        }
        self.oom_score_adj.store(value, Ordering::Relaxed);
        Ok(())
    }
}

pub struct XProcess {
    pub exe_path: RwLock<String>,
    pub uspace: XUserSpace,
    pub ns: AxNamespace,
    pub child_exit_wq: WaitQueue,
    pub exit_signal: Option<Signo>,
    pub signal: Arc<ProcessSignal>,
    pub rlimits: RwLock<Rlimits>,
    pub futex_table: FutexTable,
    pub credentials: ProcessCredentials,
}

impl XProcess {
    pub fn new(
        exe_path: String,
        uspace: XUserSpace,
        signal_actions: Arc<Mutex<SignalActions>>,
        exit_signal: Option<Signo>,
        rlimits: Option<Rlimits>,
    ) -> Self {
        Self {
            exe_path: RwLock::new(exe_path),
            uspace,
            ns: AxNamespace::new_thread_local(),
            child_exit_wq: WaitQueue::new(),
            exit_signal,
            signal: Arc::new(ProcessSignalManager::new(signal_actions, 0)),
            rlimits: RwLock::new(rlimits.unwrap_or_default()),
            futex_table: FutexTable::new(),
            credentials: ProcessCredentials::default(),
        }
    }

    pub fn from_process(process: &Arc<Process>) -> &XProcess {
        process.data().unwrap()
    }

    pub fn from_process_static(process: &Arc<Process>) -> &'static XProcess {
        unsafe { &*(process.data::<XProcess>().unwrap() as *const XProcess) }
    }

    pub fn from_thread(thread: &Arc<Thread>) -> &XProcess {
        thread.process().data().unwrap()
    }

    pub fn from_thread_static(thread: &Arc<Thread>) -> &'static XProcess {
        unsafe { &*(thread.process().data::<XProcess>().unwrap() as *const XProcess) }
    }

    pub fn is_clone_child(&self) -> bool {
        self.exit_signal != Some(Signo::SIGCHLD)
    }

    pub fn uspace(&self) -> &XUserSpace {
        &self.uspace
    }

    pub fn futex_table_for(&self, key: &FutexKey) -> &FutexTable {
        match key {
            FutexKey::Private { .. } => &self.futex_table,
            FutexKey::Shared { .. } => &SHARED_FUTEX_TABLE,
        }
    }
}

#[inherit_methods(from = "self.uspace")]
impl XProcess {
    pub fn get_heap_bottom(&self) -> usize;
    pub fn set_heap_bottom(&self, bottom: usize);
    pub fn get_heap_top(&self) -> usize;
    pub fn set_heap_top(&self, top: usize);
    pub fn add_region(&self, region: MmapRegion<FileWrapper>) -> LinuxResult<()>;
    pub fn remove_overlapping_regions(
        &self,
        vaddr_range: VirtAddrRange,
    ) -> Vec<MmapRegion<FileWrapper>>;
    pub fn clear_regions(&self);
    pub fn populate_file_pages(&self, vaddr: VirtAddr, len: usize) -> LinuxResult<()>;
}

#[inherit_methods(from = "self.credentials")]
impl XProcess {
    pub fn uid(&self) -> u32;
    pub fn set_uid(&self, uid: u32);
    pub fn gid(&self) -> u32;
    pub fn set_gid(&self, gid: u32);
    pub fn fsuid(&self) -> u32;
    pub fn set_fsuid(&self, fsuid: u32);
    pub fn fsgid(&self) -> u32;
    pub fn set_fsgid(&self, fsgid: u32);
    pub fn euid(&self) -> u32;
    pub fn set_euid(&self, euid: u32);
    pub fn egid(&self) -> u32;
    pub fn set_egid(&self, egid: u32);
    pub fn suid(&self) -> u32;
    pub fn set_suid(&self, suid: u32);
    pub fn sgid(&self) -> u32;
    pub fn set_sgid(&self, sgid: u32);
    pub fn sup_group(&self) -> Arc<Mutex<Vec<u32>>>;
    pub fn set_sup_group(&self, sup_group: Vec<u32>);
}

struct AxTaskExtImpl;
#[crate_interface::impl_interface]
impl AxTaskExtIf for AxTaskExtImpl {
    fn switch_to_task() {}

    fn switch_from_task() {
        time_stat_switch_from_old_task();
    }

    fn update_real_timer() {}
}

struct AxNamespaceImpl;
#[crate_interface::impl_interface]
impl AxNamespaceIf for AxNamespaceImpl {
    fn current_namespace_base() -> *mut u8 {
        static KERNEL_NS_BASE: Once<usize> = Once::new();
        let current = axtask::current();
        if unsafe { current.task_ext_ptr() }.is_null() {
            return *(KERNEL_NS_BASE.call_once(|| {
                let global_ns = AxNamespace::global();
                let layout = Layout::from_size_align(global_ns.size(), 64).unwrap();
                let dst = unsafe { alloc::alloc::alloc(layout) };
                let src = global_ns.base();
                unsafe { core::ptr::copy_nonoverlapping(src, dst, global_ns.size()) };
                dst as usize
            })) as *mut u8;
        }
        current.task_ext().xprocess_ref().ns.base()
    }
}

static SHARED_FUTEX_TABLE: FutexTable = FutexTable::new();
static THREAD_TABLE: RwLock<WeakMap<Pid, Weak<Thread>>> = RwLock::new(WeakMap::new());
static PROCESS_TABLE: RwLock<WeakMap<Pid, Weak<Process>>> = RwLock::new(WeakMap::new());
static PROCESS_GROUP_TABLE: RwLock<WeakMap<Pid, Weak<ProcessGroup>>> = RwLock::new(WeakMap::new());
static SESSION_TABLE: RwLock<WeakMap<Pid, Weak<Session>>> = RwLock::new(WeakMap::new());

pub fn add_thread_to_table(thread: &Arc<Thread>) {
    let mut thread_table = THREAD_TABLE.write();
    thread_table.insert(thread.tid(), thread);

    let mut process_table = PROCESS_TABLE.write();
    let process = thread.process();
    if process_table.contains_key(&process.pid()) {
        return;
    }
    process_table.insert(process.pid(), process);

    let mut process_group_table = PROCESS_GROUP_TABLE.write();
    let process_group = process.group();
    if process_group_table.contains_key(&process_group.pgid()) {
        return;
    }
    process_group_table.insert(process_group.pgid(), &process_group);

    let mut session_table = SESSION_TABLE.write();
    let session = process_group.session();
    if session_table.contains_key(&session.sid()) {
        return;
    }
    session_table.insert(session.sid(), &session);
}

pub fn processes() -> Vec<Arc<Process>> {
    PROCESS_TABLE.read().values().collect()
}

pub fn get_thread(tid: Pid) -> LinuxResult<Arc<Thread>> {
    if tid == u32::MAX {
        return Err(LinuxError::EINVAL);
    }
    if tid == 0 {
        Ok(current().task_ext().thread())
    } else {
        THREAD_TABLE.read().get(&tid).ok_or(LinuxError::ESRCH)
    }
}

pub fn get_process(pid: Pid) -> LinuxResult<Arc<Process>> {
    if pid == u32::MAX {
        return Err(LinuxError::EINVAL);
    }
    if pid == 0 {
        Ok(current().task_ext().process())
    } else {
        PROCESS_TABLE.read().get(&pid).ok_or(LinuxError::ESRCH)
    }
}

pub fn get_process_group(pgid: Pid) -> LinuxResult<Arc<ProcessGroup>> {
    PROCESS_GROUP_TABLE
        .read()
        .get(&pgid)
        .ok_or(LinuxError::ESRCH)
}

pub fn get_session(sid: Pid) -> LinuxResult<Arc<Session>> {
    SESSION_TABLE.read().get(&sid).ok_or(LinuxError::ESRCH)
}
