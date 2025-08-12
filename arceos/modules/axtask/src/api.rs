//! Task APIs for multi-task configuration.

use alloc::{
    string::String,
    sync::{Arc, Weak},
};

use kernel_guard::NoPreemptIrqSave;
#[cfg(feature = "multitask")]
use kspin::SpinNoIrq;
#[cfg(feature = "multitask")]
use weak_map::WeakMap;

pub(crate) use crate::run_queue::{current_run_queue, migrate_current, select_run_queue};

#[doc(cfg(feature = "multitask"))]
pub use crate::task::{CurrentTask, TaskId, TaskInner};
#[doc(cfg(feature = "multitask"))]
pub use crate::task_ext::{AxTaskExtIf, TaskExtMut, TaskExtRef};
#[doc(cfg(feature = "multitask"))]
pub use crate::wait_queue::WaitQueue;

/// The reference type of a task.
pub type AxTaskRef = Arc<AxTask>;

/// The weak reference type of a task.
pub type WeakAxTaskRef = Weak<AxTask>;

pub use crate::task::TaskState;

/// The wrapper type for [`cpumask::CpuMask`] with SMP configuration.
pub type AxCpuMask = cpumask::CpuMask<{ axconfig::SMP }>;

/// Global task table for managing all tasks by their TaskId
#[cfg(feature = "multitask")]
static TASK_TABLE: SpinNoIrq<WeakMap<u64, WeakAxTaskRef>> = SpinNoIrq::new(WeakMap::new());

cfg_if::cfg_if! {
    if #[cfg(feature = "sched_rr")] {
        const MAX_TIME_SLICE: usize = 5;
        pub(crate) type AxTask = axsched::RRTask<TaskInner, MAX_TIME_SLICE>;
        pub(crate) type Scheduler = axsched::RRScheduler<TaskInner, MAX_TIME_SLICE>;
    } else if #[cfg(feature = "sched_cfs")] {
        pub(crate) type AxTask = axsched::CFSTask<TaskInner>;
        pub(crate) type Scheduler = axsched::CFScheduler<TaskInner>;
    } else {
        // If no scheduler features are set, use FIFO as the default.
        pub(crate) type AxTask = axsched::FifoTask<TaskInner>;
        pub(crate) type Scheduler = axsched::FifoScheduler<TaskInner>;
    }
}

#[cfg(feature = "preempt")]
struct KernelGuardIfImpl;

#[cfg(feature = "preempt")]
#[crate_interface::impl_interface]
impl kernel_guard::KernelGuardIf for KernelGuardIfImpl {
    fn disable_preempt() {
        if let Some(curr) = current_may_uninit() {
            curr.disable_preempt();
        }
    }

    fn enable_preempt() {
        if let Some(curr) = current_may_uninit() {
            curr.enable_preempt(true);
        }
    }
}

/// Gets the current task, or returns [`None`] if the current task is not
/// initialized.
pub fn current_may_uninit() -> Option<CurrentTask> {
    CurrentTask::try_get()
}

/// Gets the current task.
///
/// # Panics
///
/// Panics if the current task is not initialized.
pub fn current() -> CurrentTask {
    CurrentTask::get()
}

/// Initializes the task scheduler (for the primary CPU).
pub fn init_scheduler() {
    info!("Initialize scheduling...");

    crate::run_queue::init();
    #[cfg(feature = "irq")]
    crate::timers::init();
}

/// Initializes the task scheduler for secondary CPUs.
pub fn init_scheduler_secondary() {
    crate::run_queue::init_secondary();
    #[cfg(feature = "irq")]
    crate::timers::init();
}

/// Handles periodic timer ticks for the task manager.
///
/// For example, advance scheduler states, checks timed events, etc.
#[cfg(feature = "irq")]
#[doc(cfg(feature = "irq"))]
pub fn on_timer_tick() {
    use kernel_guard::NoOp;
    crate::timers::check_events();
    // Since irq and preemption are both disabled here,
    // we can get current run queue with the default `kernel_guard::NoOp`.
    current_run_queue::<NoOp>().scheduler_timer_tick();
}

/// Adds the given task to the run queue, returns the task reference.
pub fn spawn_task(task: TaskInner) -> AxTaskRef {
    let task_ref = task.into_arc();

    // Register the task in the global task table
    #[cfg(feature = "multitask")]
    register_task(&task_ref);

    select_run_queue::<NoPreemptIrqSave>(&task_ref).add_task(task_ref.clone());
    task_ref
}

/// Spawns a new task with the given parameters.
///
/// Returns the task reference.
pub fn spawn_raw<F>(f: F, name: String, stack_size: usize) -> AxTaskRef
where
    F: FnOnce() + Send + 'static,
{
    spawn_task(TaskInner::new(f, name, stack_size))
}

/// Spawns a new task with the default parameters.
///
/// The default task name is an empty string. The default task stack size is
/// [`axconfig::TASK_STACK_SIZE`].
///
/// Returns the task reference.
pub fn spawn<F>(f: F) -> AxTaskRef
where
    F: FnOnce() + Send + 'static,
{
    spawn_raw(f, "".into(), axconfig::TASK_STACK_SIZE)
}

/// Set the priority for current task.
///
/// The range of the priority is dependent on the underlying scheduler. For
/// example, in the [CFS] scheduler, the priority is the nice value, ranging from
/// -20 to 19.
///
/// Returns `true` if the priority is set successfully.
///
/// [CFS]: https://en.wikipedia.org/wiki/Completely_Fair_Scheduler
pub fn set_priority(prio: isize) -> bool {
    current_run_queue::<NoPreemptIrqSave>().set_current_priority(prio)
}

/// Set the affinity for the current task.
/// [`AxCpuMask`] is used to specify the CPU affinity.
/// Returns `true` if the affinity is set successfully.
pub fn set_affinity(task: &AxTaskRef, cpumask: AxCpuMask) -> bool {
    if cpumask.is_empty() {
        false
    } else {
        task.set_cpumask(cpumask);
        // After setting the affinity, we need to check if the task needs migration
        #[cfg(feature = "smp")]
        migrate_current(task.clone());

        true
    }
}

/// Current task gives up the CPU time voluntarily, and switches to another
/// ready task.
pub fn yield_now() {
    current_run_queue::<NoPreemptIrqSave>().yield_current()
}

/// Current task is going to sleep for the given duration.
///
/// If the feature `irq` is not enabled, it uses busy-wait instead.
pub fn sleep(dur: core::time::Duration) {
    sleep_until(axhal::time::wall_time() + dur);
}

/// Current task is going to sleep, it will be woken up at the given deadline.
///
/// If the feature `irq` is not enabled, it uses busy-wait instead.
pub fn sleep_until(deadline: axhal::time::TimeValue) {
    #[cfg(feature = "irq")]
    current_run_queue::<NoPreemptIrqSave>().sleep_until(deadline);
    #[cfg(not(feature = "irq"))]
    axhal::time::busy_wait_until(deadline);
}

/// Exits the current task.
pub fn exit(exit_code: i32) -> ! {
    current_run_queue::<NoPreemptIrqSave>().exit_current(exit_code)
}

/// The idle task routine.
///
/// It runs an infinite loop that keeps calling [`yield_now()`].
pub fn run_idle() -> ! {
    loop {
        yield_now();
        debug!("idle task: waiting for IRQs...");
        #[cfg(feature = "irq")]
        axhal::arch::wait_for_irqs();
    }
}

/// Register a task in the global task table.
///
/// This function should be called when a task is created and needs to be
/// tracked in the global task table.
#[cfg(feature = "multitask")]
pub fn register_task(task_ref: &AxTaskRef) {
    let mut table = TASK_TABLE.lock();
    table.insert(task_ref.id().as_u64(), task_ref);
    debug!("Task registered: {}", task_ref.id_name());
}

/// Get a weak task reference by its TaskId.
///
/// Returns `Some(WeakAxTaskRef)` if the task exists and is still alive (weak can still
/// fail to upgrade later), or `None` if the task doesn't exist.
///
/// This is similar to the `current()` function but works for any task ID.
#[cfg(feature = "multitask")]
pub fn get_task_by_id(task_id: TaskId) -> Option<WeakAxTaskRef> {
    let table = TASK_TABLE.lock();
    table
        .get(&task_id.as_u64())
        .map(|task_ref| Arc::downgrade(&task_ref))
}

/// Execute a function with a task reference.
///
/// Returns `Some(R)` if the task exists and is still alive,
/// `None` if the task doesn't exist or has been dropped.
pub fn with_task<R>(id: TaskId, f: impl FnOnce(&AxTaskRef) -> R) -> Option<R> {
    if id.as_u64() == 0 {
        Some(f(current().as_task_ref()))
    } else {
        get_task_by_id(id)
            .and_then(|weak_task| weak_task.upgrade())
            .map(|task| f(&task))
    }
}
