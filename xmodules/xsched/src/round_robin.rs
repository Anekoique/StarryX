use core::{
    ops::Deref,
    sync::atomic::{AtomicIsize, Ordering},
};

use alloc::sync::Arc;

use intrusive_collections::{
    container_of, linked_list::LinkOps, offset_of, Adapter, DefaultLinkOps, DefaultPointerOps,
    LinkedList, LinkedListAtomicLink, LinkedListLink, PointerOps,
};

use crate::BaseScheduler;

/// A task wrapper for the [`RRScheduler`].
///
/// It add a time slice counter to use in round-robin scheduling.
pub struct RRTask<T, const MAX_TIME_SLICE: usize> {
    inner: T,
    time_slice: AtomicIsize,
    link: LinkedListAtomicLink,
}

// Copied from `intrusive_collections::intrusive_adapter` macro since it doesn't
// support const generics yet.

struct NodeAdapter<T, const MAX_TIME_SLICE: usize> {
    link_ops: LinkOps,
    pointer_ops: DefaultPointerOps<Arc<RRTask<T, MAX_TIME_SLICE>>>,
}

unsafe impl<T, const MAX_TIME_SLICE: usize> Send for NodeAdapter<T, MAX_TIME_SLICE> {}
unsafe impl<T, const MAX_TIME_SLICE: usize> Sync for NodeAdapter<T, MAX_TIME_SLICE> {}

impl<T, const MAX_TIME_SLICE: usize> NodeAdapter<T, MAX_TIME_SLICE> {
    pub const NEW: Self = NodeAdapter {
        link_ops: <LinkedListLink as DefaultLinkOps>::NEW,
        pointer_ops: DefaultPointerOps::new(),
    };
}
unsafe impl<T, const MAX_TIME_SLICE: usize> Adapter for NodeAdapter<T, MAX_TIME_SLICE> {
    type LinkOps = LinkOps;
    type PointerOps = DefaultPointerOps<Arc<RRTask<T, MAX_TIME_SLICE>>>;

    #[inline]
    unsafe fn get_value(
        &self,
        link: <Self::LinkOps as intrusive_collections::LinkOps>::LinkPtr,
    ) -> *const <Self::PointerOps as PointerOps>::Value {
        container_of!(link.as_ptr(), RRTask<T, MAX_TIME_SLICE>, link)
    }
    #[inline]
    unsafe fn get_link(
        &self,
        value: *const <Self::PointerOps as PointerOps>::Value,
    ) -> <Self::LinkOps as intrusive_collections::LinkOps>::LinkPtr {
        let ptr = (value as *const u8).add(offset_of!(RRTask<T, MAX_TIME_SLICE>, link));
        core::ptr::NonNull::new_unchecked(ptr as *mut _)
    }

    #[inline]
    fn link_ops(&self) -> &Self::LinkOps {
        &self.link_ops
    }
    #[inline]
    fn link_ops_mut(&mut self) -> &mut Self::LinkOps {
        &mut self.link_ops
    }
    #[inline]
    fn pointer_ops(&self) -> &Self::PointerOps {
        &self.pointer_ops
    }
}

// intrusive_adapter!(NodeAdapter<T, const MAX_TIME_SLICE: usize> = Arc<RRTask<T, MAX_TIME_SLICE>>: RRTask<T, MAX_TIME_SLICE> { link: LinkedListLink });

impl<T, const S: usize> RRTask<T, S> {
    /// Creates a new [`RRTask`] from the inner task struct.
    pub const fn new(inner: T) -> Self {
        Self {
            inner,
            time_slice: AtomicIsize::new(S as isize),
            link: LinkedListAtomicLink::new(),
        }
    }

    fn time_slice(&self) -> isize {
        self.time_slice.load(Ordering::Acquire)
    }

    fn reset_time_slice(&self) {
        self.time_slice.store(S as isize, Ordering::Release);
    }

    /// Returns a reference to the inner task struct.
    pub const fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T, const S: usize> Deref for RRTask<T, S> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A simple [Round-Robin] (RR) preemptive scheduler.
///
/// It's very similar to the [`FifoScheduler`], but every task has a time slice
/// counter that is decremented each time a timer tick occurs. When the current
/// task's time slice counter reaches zero, the task is preempted and needs to
/// be rescheduled.
///
/// Unlike [`FifoScheduler`], it uses [`VecDeque`] as the ready queue. So it may
/// take O(n) time to remove a task from the ready queue.
///
/// [Round-Robin]: https://en.wikipedia.org/wiki/Round-robin_scheduling
/// [`FifoScheduler`]: crate::FifoScheduler
pub struct RRScheduler<T, const MAX_TIME_SLICE: usize> {
    ready_queue: LinkedList<NodeAdapter<T, MAX_TIME_SLICE>>,
}

impl<T, const S: usize> RRScheduler<T, S> {
    /// Creates a new empty [`RRScheduler`].
    pub const fn new() -> Self {
        Self {
            ready_queue: LinkedList::new(NodeAdapter::NEW),
        }
    }
    /// get the name of scheduler
    pub fn scheduler_name() -> &'static str {
        "Round-robin"
    }
}

impl<T, const S: usize> BaseScheduler for RRScheduler<T, S> {
    type SchedItem = Arc<RRTask<T, S>>;

    fn init(&mut self) {}

    fn add_task(&mut self, task: Self::SchedItem) {
        self.ready_queue.push_back(task);
    }

    fn remove_task(&mut self, task: &Self::SchedItem) -> Option<Self::SchedItem> {
        let mut cursor = unsafe { self.ready_queue.cursor_mut_from_ptr(Arc::as_ptr(task)) };
        cursor.remove()
    }

    fn pick_next_task(&mut self) -> Option<Self::SchedItem> {
        self.ready_queue.pop_front()
    }

    fn put_prev_task(&mut self, prev: Self::SchedItem, preempt: bool) {
        if prev.time_slice() > 0 && preempt {
            self.ready_queue.push_front(prev)
        } else {
            prev.reset_time_slice();
            self.ready_queue.push_back(prev)
        }
    }

    fn task_tick(&mut self, current: &Self::SchedItem) -> bool {
        let old_slice = current.time_slice.fetch_sub(1, Ordering::Release);
        old_slice <= 1
    }

    fn set_priority(&mut self, _task: &Self::SchedItem, _prio: isize) -> bool {
        false
    }
}

impl<T, const S: usize> Default for RRScheduler<T, S> {
    fn default() -> Self {
        Self::new()
    }
}
