use axhal::{
    mem::{MemoryAddr, VirtAddr},
    paging::MappingFlags,
    trap::{PAGE_FAULT, register_trap_handler},
};
use axsignal::{SignalInfo, Signo};
use axtask::current;
use linux_raw_sys::general::{RLIMIT_STACK, SI_KERNEL, SIGSEGV};
use starry_core::{
    mm::is_accessing_user_memory,
    task::{TaskExt, send_signal_process},
};

#[register_trap_handler(PAGE_FAULT)]
fn handle_page_fault(vaddr: VirtAddr, access_flags: MappingFlags, is_user: bool) -> bool {
    warn!(
        "Page fault at {:#x}, access_flags: {:#x?}",
        vaddr, access_flags
    );
    if !is_user && !is_accessing_user_memory() {
        return false;
    }

    let curr_ext = TaskExt::from_task(&current());
    let send_sigsegv = || {
        debug!("Sending SIGSEGV");
        let _ = send_signal_process(
            curr_ext.thread.process(),
            SignalInfo::new(Signo::from_repr(SIGSEGV as u8).unwrap(), SI_KERNEL as _),
        );
    };

    if (axconfig::plat::USER_STACK_TOP - axconfig::plat::USER_STACK_SIZE
        ..axconfig::plat::USER_STACK_TOP)
        .contains(&vaddr.as_usize())
    {
        // Stack extension, check rlimit
        let rlimit = &curr_ext.process_data().rlimits.read()[RLIMIT_STACK];
        let size = axconfig::plat::USER_STACK_TOP - vaddr.as_usize();
        if size as u64 > rlimit.current {
            debug!("Stack extension, check rlimit");
            send_sigsegv();
        }
    }

    if !curr_ext
        .process_data()
        .aspace
        .lock()
        .handle_page_fault(vaddr, access_flags)
    {
        warn!(
            "{} ({:?}): segmentation fault at {:#x}, sending SIGSEGV",
            current().id_name(),
            curr_ext.thread,
            vaddr
        );
        let _ = send_signal_process(
            curr_ext.thread.process(),
            SignalInfo::new(Signo::from_repr(SIGSEGV as u8).unwrap(), SI_KERNEL as _),
        );
    }

    curr_ext.process_data().get_buf(vaddr).ok().map(|data| {
        curr_ext
            .process_data()
            .aspace
            .lock()
            .write(vaddr.align_down_4k(), &data)
    });

    true
}
