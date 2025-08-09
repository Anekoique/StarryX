use axerrno::LinuxResult;

use xcore::task::with_xprocess;

/// Change data segment size.
///
/// # Arguments
/// * `addr` - New end address of the heap (0 to query current end)
pub fn sys_brk(addr: usize) -> LinuxResult<isize> {
    with_xprocess(|xprocess| {
        let mut return_val: isize = xprocess.get_heap_top() as isize;
        let heap_bottom = xprocess.get_heap_bottom();
        if addr != 0 && addr >= heap_bottom && addr <= heap_bottom + xcore::config::USER_HEAP_SIZE {
            xprocess.set_heap_top(addr);
            return_val = addr as isize;
        }
        Ok(return_val)
    })
}
