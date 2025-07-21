use axerrno::{LinuxError, LinuxResult};
use axuspace::{UserPtr, UserSpaceAccess};
use xcore::task::{get_process, with_uspace};

use crate::ctypes::{__user_cap_data_struct, __user_cap_header_struct};

fn validate_cap_header(header: &mut __user_cap_header_struct) -> LinuxResult<()> {
    if header.version != 0x20080522 {
        header.version = 0x20080522;
        return Err(LinuxError::EINVAL);
    }
    get_process(header.pid as u32).map(|_| ())
}

pub fn sys_capget(
    header: UserPtr<__user_cap_header_struct>,
    data: UserPtr<__user_cap_data_struct>,
) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        validate_cap_header(uspace.raw_ptr(header)?)?;
        uspace.write(data, __user_cap_data_struct {
            effective: u32::MAX,
            permitted: u32::MAX,
            inheritable: u32::MAX,
        })?;
        Ok(0)
    })
}

pub fn sys_capset(
    header: UserPtr<__user_cap_header_struct>,
    _data: UserPtr<__user_cap_data_struct>,
) -> LinuxResult<isize> {
    with_uspace(|uspace| validate_cap_header(uspace.raw_ptr(header)?).map(|_| 0))
}
