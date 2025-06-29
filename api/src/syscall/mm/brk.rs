use axerrno::LinuxResult;
use axtask::current;
use starry_core::task::TaskExt;

/// Change data segment size.
///
/// # Arguments
/// * `addr` - New end address of the heap (0 to query current end)
pub fn sys_brk(addr: usize) -> LinuxResult<isize> {
    let process_data = TaskExt::from_task(&current()).process_data();
    let mut return_val: isize = process_data.get_heap_top() as isize;
    let heap_bottom = process_data.get_heap_bottom() as usize;
    if addr != 0 && addr >= heap_bottom && addr <= heap_bottom + axconfig::plat::USER_HEAP_SIZE {
        process_data.set_heap_top(addr);
        return_val = addr as isize;
    }
    Ok(return_val)
}
