use xerrno::{LinuxError, LinuxResult};

use xuspace::UserPtr;
use xutils::ctypes::{
    __user_cap_data_struct, __user_cap_header_struct, _LINUX_CAPABILITY_VERSION_1,
    _LINUX_CAPABILITY_VERSION_2, _LINUX_CAPABILITY_VERSION_3,
};

use crate::task::{get_process, with_process, with_uspace};

const PR_SET_CHILD_SUBREAPER: u32 = 36;
const PR_GET_CHILD_SUBREAPER: u32 = 37;

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
        let mut header_value = uspace.read(header)?;
        validate_cap_header(&mut header_value)?;
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
        let mut header_value = uspace.read(header)?;
        if let Err(error) = validate_cap_header(&mut header_value) {
            header_value.version = _LINUX_CAPABILITY_VERSION_3;
            uspace.write(header, header_value)?;
            return Err(error);
        }
        Ok(0)
    })
}

pub fn sys_prctl(
    option: u32,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> LinuxResult<isize> {
    match option {
        PR_SET_CHILD_SUBREAPER => {
            if arg2 > 1 || arg3 != 0 || arg4 != 0 || arg5 != 0 {
                return Err(LinuxError::EINVAL);
            }
            with_process(|process| process.set_child_subreaper(arg2 != 0));
            Ok(0)
        }
        PR_GET_CHILD_SUBREAPER => {
            if arg3 != 0 || arg4 != 0 || arg5 != 0 {
                return Err(LinuxError::EINVAL);
            }
            let enabled = with_process(|process| process.is_child_subreaper()) as i32;
            with_uspace(|uspace| {
                uspace.write(UserPtr::<i32>::from(arg2), enabled)?;
                Ok(0)
            })
        }
        _ => Ok(0),
    }
}
