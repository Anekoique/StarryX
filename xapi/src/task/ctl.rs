use axerrno::{LinuxError, LinuxResult};

use xuspace::{UserPtr, UserSpaceAccess};
use xutils::ctypes::{
    __user_cap_data_struct, __user_cap_header_struct, _LINUX_CAPABILITY_VERSION_1,
    _LINUX_CAPABILITY_VERSION_2, _LINUX_CAPABILITY_VERSION_3,
};

use xcore::task::{get_process, with_uspace};

fn validate_cap_header(header: &mut __user_cap_header_struct) -> LinuxResult<()> {
    match header.version {
        _LINUX_CAPABILITY_VERSION_1 | _LINUX_CAPABILITY_VERSION_2 | _LINUX_CAPABILITY_VERSION_3 => {
        }
        _ => {
            return Err(LinuxError::EINVAL);
        }
    }
    get_process(header.pid as u32).map(|_| ())
}

pub fn sys_capget(
    header: UserPtr<__user_cap_header_struct>,
    data: UserPtr<__user_cap_data_struct>,
) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        validate_cap_header(uspace.raw_ptr(header)?)?;
        uspace.write(
            data,
            __user_cap_data_struct {
                effective: u32::MIN,
                permitted: u32::MIN,
                inheritable: u32::MIN,
            },
        )?;
        Ok(0)
    })
}

pub fn sys_capset(
    header: UserPtr<__user_cap_header_struct>,
    data: UserPtr<__user_cap_data_struct>,
) -> LinuxResult<isize> {
    with_uspace(|uspace| {
        let _data = uspace.read(data)?;
        let header = uspace.raw_ptr(header)?;
        validate_cap_header(header)
            .inspect_err(|_| header.version = _LINUX_CAPABILITY_VERSION_3)
            .map(|_| 0)
    })
}
