use axhal::{
    mem::{MemoryAddr, VirtAddr},
    paging::MappingFlags,
    trap::{PAGE_FAULT, register_trap_handler},
};
use axsignal::{SignalInfo, Signo};
use axtask::{TaskExtRef, current};
use linux_raw_sys::general::{RLIMIT_STACK, SI_KERNEL, SIGSEGV};
use starry_core::{mm::is_accessing_user_memory, task::send_signal_process};

#[register_trap_handler(PAGE_FAULT)]
fn handle_page_fault(vaddr: VirtAddr, access_flags: MappingFlags, is_user: bool) -> bool {
    warn!(
        "Page fault at {:#x}, access_flags: {:#x?}",
        vaddr, access_flags
    );
    if !is_user && !is_accessing_user_memory() {
        return false;
    }

    let curr = current();
    let send_sigsegv = || {
        debug!("Sending SIGSEGV");
        let _ = send_signal_process(
            curr.task_ext().thread.process(),
            SignalInfo::new(Signo::from_repr(SIGSEGV as u8).unwrap(), SI_KERNEL as _),
        );
    };

    if (axconfig::plat::USER_STACK_TOP - axconfig::plat::USER_STACK_SIZE
        ..axconfig::plat::USER_STACK_TOP)
        .contains(&vaddr.as_usize())
    {
        // Stack extension, check rlimit
        let rlimit = &curr.task_ext().process_data().rlimits.read()[RLIMIT_STACK];
        let size = axconfig::plat::USER_STACK_TOP - vaddr.as_usize();
        if size as u64 > rlimit.current {
            send_sigsegv();
        }
    }

    // First check if we can find a region, return false if not found

    let buf = curr
        .task_ext()
        .process_data()
        .find_mmap_region_by_addr(vaddr)
        .and_then(|region| region.has_file())
        .map(|region| {
            region
                .check_file(vaddr)
                .and_then(|region| region.get_buf_by_addr(vaddr))
                .map_err(|_| send_sigsegv())
        });

    if !curr
        .task_ext()
        .process_data()
        .aspace
        .lock()
        .handle_page_fault(vaddr, access_flags)
    {
        warn!(
            "{} ({:?}): segmentation fault at {:#x}, sending SIGSEGV",
            curr.id_name(),
            curr.task_ext().thread,
            vaddr
        );
        let _ = send_signal_process(
            curr.task_ext().thread.process(),
            SignalInfo::new(Signo::from_repr(SIGSEGV as u8).unwrap(), SI_KERNEL as _),
        );
    }

    // Write buffer to address space if we have one
    if let Some(Ok(data)) = buf {
        let _ = curr
            .task_ext()
            .process_data()
            .aspace
            .lock()
            .write(vaddr.align_down_4k(), &data);
    }

    true
}
