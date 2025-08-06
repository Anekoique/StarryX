//! Interrupt management.
use alloc::{collections::btree_map::BTreeMap, vec::Vec};

use handler_table::HandlerTable;
use spin::RwLock;

use crate::arch::{disable_irqs, enable_irqs, irqs_enabled};
use crate::platform::irq::{MAX_IRQ_COUNT, dispatch_irq};
use crate::trap::{IRQ, register_trap_handler};

pub use crate::platform::irq::{register_handler, set_enable};

/// The type if an IRQ handler.
pub type IrqHandler = handler_table::Handler;

static IRQ_HANDLER_TABLE: HandlerTable<MAX_IRQ_COUNT> = HandlerTable::new();
static IRQ_STATISTIC: RwLock<BTreeMap<usize, usize>> = RwLock::new(BTreeMap::new());

#[allow(dead_code)]
pub(crate) fn increment_irq_count(irq_num: usize) {
    let mut irq_statistic = IRQ_STATISTIC.write();
    *irq_statistic.entry(irq_num).or_insert(0) += 1;
}

pub fn irq_stat() -> Vec<(usize, usize)> {
    let irq_statistic = IRQ_STATISTIC.read();
    irq_statistic
        .iter()
        .map(|(&irq, &count)| (irq, count))
        .collect()
}

/// Platform-independent IRQ dispatching.
#[allow(dead_code)]
pub(crate) fn dispatch_irq_common(irq_num: usize) {
    trace!("IRQ {}", irq_num);
    if !IRQ_HANDLER_TABLE.handle(irq_num) {
        warn!("Unhandled IRQ {}", irq_num);
    }
}

/// Platform-independent IRQ handler registration.
///
/// It also enables the IRQ if the registration succeeds. It returns `false` if
/// the registration failed.
#[allow(dead_code)]
pub(crate) fn register_handler_common(irq_num: usize, handler: IrqHandler) -> bool {
    if irq_num < MAX_IRQ_COUNT && IRQ_HANDLER_TABLE.register_handler(irq_num, handler) {
        set_enable(irq_num, true);
        return true;
    }
    warn!("register handler for IRQ {} failed", irq_num);
    false
}

#[register_trap_handler(IRQ)]
fn handler_irq(irq_num: usize) -> bool {
    let guard = kernel_guard::NoPreempt::new();
    dispatch_irq(irq_num);
    drop(guard); // rescheduling may occur when preemption is re-enabled.
    true
}

/// Execute a closure with interrupts disabled.
pub fn with_irqs_disabled<T>(f: impl FnOnce() -> T) -> T {
    let was_enabled = irqs_enabled();
    disable_irqs();

    // Execute the closure
    let result = f();

    // Restore interrupt state
    if was_enabled {
        enable_irqs();
    }

    result
}
