//! User task management.

use core::{
    alloc::Layout,
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use alloc::{
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use axerrno::{LinuxError, LinuxResult};
use axhal::{
    arch::UspaceContext,
    irq::with_irqs_disabled,
    time::{NANOS_PER_MICROS, NANOS_PER_SEC, monotonic_time_nanos},
};
use axmm::{AddrSpace, kernel_aspace};
use axns::{AxNamespace, AxNamespaceIf};
use axprocess::{Pid, Process, ProcessGroup, Session, Thread};
use axsignal::{
    SignalInfo, Signo,
    api::{ProcessSignalManager, SignalActions, ThreadSignalManager},
};
use axsync::{Mutex, RawMutex};
use axtask::{AxTaskExtIf, TaskExtRef, TaskInner, WaitQueue, current};
use memory_addr::{VirtAddr, VirtAddrRange};
use spin::{Once, RwLock};
use weak_map::WeakMap;

use crate::{
    mm::{MmapRegion, VmaMapping},
    resources::Rlimits,
    task::FutexTable,
    time::TimeStat,
};

/// Create a new user task.
pub fn new_user_task(
    name: &str,
    uctx: UspaceContext,
    set_child_tid: Option<&'static mut Pid>,
) -> TaskInner {
    TaskInner::new(
        move || {
            let curr = axtask::current();
            if let Some(tid) = set_child_tid {
                *tid = curr.id().as_u64() as Pid;
            }

            let kstack_top = curr.kernel_stack_top().unwrap();
            info!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                uctx.ip(),
                uctx.sp(),
                kstack_top,
            );
            unsafe { uctx.enter_uspace(kstack_top) }
        },
        name.into(),
        axconfig::plat::KERNEL_STACK_SIZE,
    )
}

/// Task extended data for the monolithic kernel.
pub struct TaskExt {
    /// The time statistics
    pub time: RefCell<TimeStat>,
    /// The thread
    pub thread: Arc<Thread>,
}

impl TaskExt {
    /// Create a new [`TaskExt`].
    pub fn new(thread: Arc<Thread>) -> Self {
        Self {
            time: RefCell::new(TimeStat::new()),
            thread,
        }
    }

    /// Get the [`TaskExt`] from a [`TaskInner`] reference.
    /// This works with `&current()` due to `CurrentTask`'s `Deref` implementation.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the TaskInner and its TaskExt remain valid
    /// for the static lifetime. This is typically safe for tasks that are
    /// managed by the task scheduler.
    pub fn from_task(task: &TaskInner) -> &'static Self {
        unsafe { &*(task.task_ext() as *const Self) }
    }

    pub(crate) fn time_stat_from_kernel_to_user(&self, current_tick: usize) {
        with_irqs_disabled(|| {
            self.time.borrow_mut().switch_into_user_mode(current_tick);
        });
    }

    pub(crate) fn time_stat_from_user_to_kernel(&self, current_tick: usize) {
        with_irqs_disabled(|| {
            self.time.borrow_mut().switch_into_kernel_mode(current_tick);
        });
    }

    pub(crate) fn time_stat_switch_from_old_task(&self, current_tick: usize) {
        with_irqs_disabled(|| {
            self.time.borrow_mut().switch_from_old_task(current_tick);
        });
    }

    pub(crate) fn time_stat_switch_to_new_task(&self, current_tick: usize) {
        with_irqs_disabled(|| {
            self.time.borrow_mut().switch_to_new_task(current_tick);
        });
    }

    pub(crate) fn time_stat_update_real_timer(&self, current_tick: usize) {
        self.time.borrow_mut().update_real_timer(current_tick);
    }

    pub(crate) fn time_stat_output(&self) -> (usize, usize) {
        self.time.borrow().output()
    }

    /// Get the [`ThreadData`] associated with this task.
    pub fn thread_data(&self) -> &ThreadData {
        self.thread.data().unwrap()
    }

    /// Get the [`ProcessData`] associated with this task.
    pub fn process_data(&self) -> &ProcessData {
        self.thread.process().data().unwrap()
    }
}

axtask::def_task_ext!(TaskExt);

/// Update the time statistics to reflect a switch from kernel mode to user mode.
pub fn time_stat_from_kernel_to_user() {
    TaskExt::from_task(&current()).time_stat_from_kernel_to_user(monotonic_time_nanos() as usize);
}

/// Update the time statistics to reflect a switch from user mode to kernel mode.
pub fn time_stat_from_user_to_kernel() {
    TaskExt::from_task(&current()).time_stat_from_user_to_kernel(monotonic_time_nanos() as usize);
}

/// Get the time statistics for the current task.
pub fn time_stat_output() -> (usize, usize, usize, usize, usize, usize) {
    let (utime_ns, stime_ns) = TaskExt::from_task(&current()).time_stat_output();
    (
        utime_ns,
        utime_ns / NANOS_PER_SEC as usize,
        utime_ns / NANOS_PER_MICROS as usize,
        stime_ns,
        stime_ns / NANOS_PER_SEC as usize,
        stime_ns / NANOS_PER_MICROS as usize,
    )
}

#[doc(hidden)]
pub struct WaitQueueWrapper(WaitQueue);
impl Default for WaitQueueWrapper {
    fn default() -> Self {
        Self(WaitQueue::new())
    }
}
impl axsignal::api::WaitQueue for WaitQueueWrapper {
    fn wait_timeout(&self, timeout: Option<Duration>) -> bool {
        if let Some(timeout) = timeout {
            self.0.wait_timeout(timeout)
        } else {
            self.0.wait();
            true
        }
    }

    fn notify_one(&self) -> bool {
        self.0.notify_one(false)
    }
}

/// Extended data for [`Thread`].
pub struct ThreadData {
    /// When the thread exits, the kernel clears the word at this address if it is not NULL.
    pub clear_child_tid: AtomicUsize,
    /// The head of the robust list
    pub robust_list_head: AtomicUsize,
    /// The thread-level signal manager
    pub signal: ThreadSignalManager<RawMutex, WaitQueueWrapper>,
}

impl ThreadData {
    /// Create a new [`ThreadData`].
    #[allow(clippy::new_without_default)]
    pub fn new(proc: &ProcessData) -> Self {
        Self {
            clear_child_tid: AtomicUsize::new(0),
            robust_list_head: AtomicUsize::new(0),
            signal: ThreadSignalManager::new(proc.signal.clone()),
        }
    }

    /// Get the clear child tid field.
    pub fn clear_child_tid(&self) -> usize {
        self.clear_child_tid.load(Ordering::Relaxed)
    }

    /// Set the clear child tid field.
    pub fn set_clear_child_tid(&self, clear_child_tid: usize) {
        self.clear_child_tid
            .store(clear_child_tid, Ordering::Relaxed);
    }
}

/// Extended data for [`Process`].
pub struct ProcessData {
    /// The executable path
    pub exe_path: RwLock<String>,
    /// The virtual memory address space.
    pub aspace: Arc<Mutex<AddrSpace>>,
    /// The resource namespace
    pub ns: AxNamespace,
    /// The user heap bottom
    heap_bottom: AtomicUsize,
    /// The user heap top
    heap_top: AtomicUsize,
    /// The child exit wait queue
    pub child_exit_wq: WaitQueue,
    /// The exit signal of the thread
    pub exit_signal: Option<Signo>,
    /// The process signal manager
    pub signal: Arc<ProcessSignalManager<RawMutex, WaitQueueWrapper>>,
    /// resource limits
    pub rlimits: RwLock<Rlimits>,
    /// The futex table
    pub futex_table: FutexTable,
    /// The VMA mapping for mmap regions
    pub vma_mapping: RwLock<VmaMapping>,
}

impl ProcessData {
    /// Create a new [`ProcessData`].
    pub fn new(
        exe_path: String,
        aspace: Arc<Mutex<AddrSpace>>,
        signal_actions: Arc<Mutex<SignalActions>>,
        exit_signal: Option<Signo>,
        rlimits: Option<Rlimits>,
        vma_mapping: RwLock<VmaMapping>,
    ) -> Self {
        Self {
            exe_path: RwLock::new(exe_path),
            aspace,
            ns: AxNamespace::new_thread_local(),
            heap_bottom: AtomicUsize::new(axconfig::plat::USER_HEAP_BASE),
            heap_top: AtomicUsize::new(axconfig::plat::USER_HEAP_BASE),

            child_exit_wq: WaitQueue::new(),
            exit_signal,

            signal: Arc::new(ProcessSignalManager::new(
                signal_actions,
                axconfig::plat::SIGNAL_TRAMPOLINE,
            )),

            rlimits: RwLock::new(rlimits.unwrap_or_default()),

            futex_table: FutexTable::new(),

            vma_mapping,
        }
    }

    /// Get the bottom address of the user heap.
    pub fn get_heap_bottom(&self) -> usize {
        self.heap_bottom.load(Ordering::Acquire)
    }

    /// Set the bottom address of the user heap.
    pub fn set_heap_bottom(&self, bottom: usize) {
        self.heap_bottom.store(bottom, Ordering::Release)
    }

    /// Get the top address of the user heap.
    pub fn get_heap_top(&self) -> usize {
        self.heap_top.load(Ordering::Acquire)
    }

    /// Set the top address of the user heap.
    pub fn set_heap_top(&self, top: usize) {
        self.heap_top.store(top, Ordering::Release)
    }

    /// Linux manual: A "clone" child is one which delivers no signal, or a
    /// signal other than SIGCHLD to its parent upon termination.
    pub fn is_clone_child(&self) -> bool {
        self.exit_signal != Some(Signo::SIGCHLD)
    }

    /// Add a new memory mapping region
    pub fn add_region(&self, region: MmapRegion) -> LinuxResult<()> {
        self.vma_mapping.write().add_region(region)
    }

    /// Remove all regions that overlap with the given address range
    pub fn remove_overlapping_regions(&self, vaddr_range: VirtAddrRange) -> Vec<MmapRegion> {
        self.vma_mapping
            .write()
            .remove_overlapping_regions(vaddr_range)
    }

    /// Clear all mmap mappings
    pub fn clear_regions(&self) {
        self.vma_mapping.write().clear()
    }

    /// Populate file-backed pages in the address space
    pub fn populate_file_pages(&self, vaddr: VirtAddr, len: usize) -> LinuxResult<()> {
        self.vma_mapping.read().populate_file_pages(vaddr, len)
    }
}

impl Drop for ProcessData {
    fn drop(&mut self) {
        if !cfg!(target_arch = "aarch64") && !cfg!(target_arch = "loongarch64") {
            // See [`crate::new_user_aspace`]
            let kernel = kernel_aspace().lock();
            self.aspace
                .lock()
                .clear_mappings(VirtAddrRange::from_start_size(kernel.base(), kernel.size()));
        }
    }
}

struct AxTaskExtImpl;
#[crate_interface::impl_interface]
impl AxTaskExtIf for AxTaskExtImpl {
    fn switch_to_task() {
        TaskExt::from_task(&current())
            .time_stat_switch_to_new_task(monotonic_time_nanos() as usize);
    }

    fn switch_from_task() {
        TaskExt::from_task(&current())
            .time_stat_switch_from_old_task(monotonic_time_nanos() as usize);
    }

    fn update_real_timer() {
        TaskExt::from_task(&current()).time_stat_update_real_timer(monotonic_time_nanos() as usize);
    }
}

struct AxNamespaceImpl;
#[crate_interface::impl_interface]
impl AxNamespaceIf for AxNamespaceImpl {
    fn current_namespace_base() -> *mut u8 {
        // Namespace for kernel task
        static KERNEL_NS_BASE: Once<usize> = Once::new();
        let current = axtask::current();
        // Safety: We only check whether the task extended data is null and do not access it.
        if unsafe { current.task_ext_ptr() }.is_null() {
            return *(KERNEL_NS_BASE.call_once(|| {
                let global_ns = AxNamespace::global();
                let layout = Layout::from_size_align(global_ns.size(), 64).unwrap();
                // Safety: The global namespace is a static readonly variable and will not be dropped.
                let dst = unsafe { alloc::alloc::alloc(layout) };
                let src = global_ns.base();
                unsafe { core::ptr::copy_nonoverlapping(src, dst, global_ns.size()) };
                dst as usize
            })) as *mut u8;
        }
        current.task_ext().process_data().ns.base()
    }
}

static THREAD_TABLE: RwLock<WeakMap<Pid, Weak<Thread>>> = RwLock::new(WeakMap::new());
static PROCESS_TABLE: RwLock<WeakMap<Pid, Weak<Process>>> = RwLock::new(WeakMap::new());
static PROCESS_GROUP_TABLE: RwLock<WeakMap<Pid, Weak<ProcessGroup>>> = RwLock::new(WeakMap::new());
static SESSION_TABLE: RwLock<WeakMap<Pid, Weak<Session>>> = RwLock::new(WeakMap::new());

/// Add the thread and possibly its process, process group and session to the
/// corresponding tables.
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

/// Lists all processes.
pub fn processes() -> Vec<Arc<Process>> {
    PROCESS_TABLE.read().values().collect()
}

/// Finds the thread with the given TID.
pub fn get_thread(tid: Pid) -> LinuxResult<Arc<Thread>> {
    if tid == 0 {
        Ok(current().task_ext().thread.clone())
    } else {
        THREAD_TABLE.read().get(&tid).ok_or(LinuxError::ESRCH)
    }
}
/// Finds the process with the given PID.
pub fn get_process(pid: Pid) -> LinuxResult<Arc<Process>> {
    if pid == 0 {
        Ok(current().task_ext().thread.process().clone())
    } else {
        PROCESS_TABLE.read().get(&pid).ok_or(LinuxError::ESRCH)
    }
}
/// Finds the process group with the given PGID.
pub fn get_process_group(pgid: Pid) -> LinuxResult<Arc<ProcessGroup>> {
    PROCESS_GROUP_TABLE
        .read()
        .get(&pgid)
        .ok_or(LinuxError::ESRCH)
}
/// Finds the session with the given SID.
pub fn get_session(sid: Pid) -> LinuxResult<Arc<Session>> {
    SESSION_TABLE.read().get(&sid).ok_or(LinuxError::ESRCH)
}

/// Send a signal to a thread.
pub fn send_signal_thread(thr: &Thread, sig: SignalInfo) -> LinuxResult<()> {
    info!("Send signal {:?} to thread {}", sig.signo(), thr.tid());
    let Some(thr) = thr.data::<ThreadData>() else {
        return Err(LinuxError::EPERM);
    };
    thr.signal.send_signal(sig);
    Ok(())
}

/// Send a signal to a process.
pub fn send_signal_process(proc: &Process, sig: SignalInfo) -> LinuxResult<()> {
    info!("Send signal {:?} to process {}", sig.signo(), proc.pid());
    let Some(proc) = proc.data::<ProcessData>() else {
        return Err(LinuxError::EPERM);
    };
    proc.signal.send_signal(sig);
    Ok(())
}

/// Send a signal to a process group.
pub fn send_signal_process_group(pg: &ProcessGroup, sig: SignalInfo) -> usize {
    info!(
        "Send signal {:?} to process group {}",
        sig.signo(),
        pg.pgid()
    );
    let mut count = 0;
    for proc in pg.processes() {
        count += send_signal_process(&proc, sig.clone()).is_ok() as usize;
    }
    count
}
