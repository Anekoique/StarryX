use crate::mem::{PhysAddr, virt_to_phys};

/// Starts the given secondary CPU with its boot stack.
pub fn start_secondary_cpu(hartid: usize, stack_top: PhysAddr) {
    debug!("enter start_secondary_cpu");
    debug!("run _start_secondary");
    unsafe extern "C" {
        fn _start_secondary();
    }
    debug!("run sbi_rt::probe_extension(sbi_rt::Hsm).is_unavailable()");
    if sbi_rt::probe_extension(sbi_rt::Hsm).is_unavailable() {
        warn!("HSM SBI extension is not supported for current SEE.");
        return;
    }
    debug!("run virt_to_phys");
    let entry = virt_to_phys(va!(_start_secondary as *const () as usize));
    debug!("run hart_start");
    sbi_rt::hart_start(hartid, entry.as_usize(), stack_top.as_usize());
    debug!("exit start_secondary_cpu");
}
