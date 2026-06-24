use axhal::{
    mem::{MemoryAddr, PAGE_SIZE_4K, VirtAddr, virt_to_phys},
    paging::MappingFlags,
    trap::{PAGE_FAULT, register_trap_handler},
};
use axtask::current;
use linux_raw_sys::general::{RLIMIT_STACK, SI_KERNEL, SIGSEGV};

use xcore::task::{XTaskExt, send_signal_process};
use xsignal::{SignalInfo, Signo};
use xuspace::is_accessing_user_memory;

#[register_trap_handler(PAGE_FAULT)]
fn handle_page_fault(vaddr: VirtAddr, access_flags: MappingFlags, is_user: bool) -> bool {
    // warn!(
    //     "Page fault at {:#x}, access_flags: {:#x?}",
    //     vaddr, access_flags
    // );
    if !is_user && !is_accessing_user_memory() {
        return false;
    }

    let xtask = XTaskExt::from_task(&current());
    let xprocess = xtask.xprocess();
    let send_sigsegv = || {
        debug!("Sending SIGSEGV");
        let _ = send_signal_process(
            xtask.thread_ref().process(),
            SignalInfo::new(Signo::from_repr(SIGSEGV as u8).unwrap(), SI_KERNEL as _),
        );
    };

    if (xcore::config::USER_STACK_TOP - xcore::config::USER_STACK_SIZE
        ..xcore::config::USER_STACK_TOP)
        .contains(&vaddr.as_usize())
    {
        // Stack extension, check rlimit
        let rlimit = &xprocess.rlimits.read()[RLIMIT_STACK];
        let size = xcore::config::USER_STACK_TOP - vaddr.as_usize();
        if size as u64 > rlimit.current {
            debug!("Stack extension, check rlimit");
            send_sigsegv();
        }
    }

    if !xprocess
        .uspace()
        .aspace
        .lock()
        .handle_page_fault(vaddr, access_flags)
    {
        warn!(
            "{} ({:?}): segmentation fault at VirtAddr: ({:#x}), PhysAddr: ({:#x}), sending SIGSEGV",
            current().id_name(),
            xtask.thread_ref(),
            vaddr,
            virt_to_phys(vaddr),
        );

        let _ = send_signal_process(
            xtask.thread_ref().process(),
            SignalInfo::new(Signo::from_repr(SIGSEGV as u8).unwrap(), SI_KERNEL as _),
        );
    }

    xprocess
        .uspace()
        .populate_file_pages(vaddr.align_down_4k(), PAGE_SIZE_4K)
        .map_err(|_| send_sigsegv())
        .expect("Failed to populate file pages");

    true
}
