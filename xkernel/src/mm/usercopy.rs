// SPDX-License-Identifier: GPL-3.0-or-later OR Apache-2.0

//! Linux UAPI typed-copy boundary.
//!
//! `xuspace` only moves bytes. This private module explicitly decodes the
//! finite set of Linux ABI values accepted by the kernel, so arbitrary user
//! bytes are never materialized as an unconstrained Rust type.

use alloc::{vec, vec::Vec};
use core::alloc::Layout;

use memory_addr::VirtAddr;
use xerrno::{LinuxError, LinuxResult};
use xsignal::{SignalInfo, SignalSet, SignalStack};
use xuspace::{UserConstPtr, UserPtr, UserReadable, UserSpaceAccess};
use xutils::ctypes::{
    __kernel_fd_set, __kernel_fsid_t, __kernel_old_timeval, __user_cap_data_struct,
    __user_cap_header_struct, clone_args, epoll_event, iovec, itimerval, loop_info, new_utsname,
    rlimit, rlimit64, robust_list, robust_list_head, rusage, sigset_t, stat, statfs, statx,
    statx_timestamp,
    sys::{Tms, itimerspec, rtc_time, utimbuf},
    sysinfo, termios, timespec, timeval, winsize,
};

use crate::{
    ipc::{IpcPerm, MsgidDs, SemBuf, SemInfo, ShmInfo},
    syscall::iomux::PollFd,
};

use super::XUserSpace;

pub(crate) trait UserRead: Sized {
    fn decode(input: &[u8]) -> Self;
}

pub(crate) trait UserWrite: Sized {
    fn encode(&self, output: &mut [u8]);
}

/// Linux `struct kernel_sigaction` on RISC-V and LoongArch64.
///
/// Handler addresses remain integers at the user-copy boundary: arbitrary
/// user bytes are not valid Rust function pointers.
#[derive(Clone, Copy, Default)]
#[repr(C)]
pub(crate) struct UserSignalAction {
    pub handler: usize,
    pub flags: u64,
    pub mask: SignalSet,
}

fn decode_field<T: UserRead>(input: &[u8], offset: usize) -> T {
    T::decode(&input[offset..offset + core::mem::size_of::<T>()])
}

fn encode_field<T: UserWrite>(output: &mut [u8], offset: usize, value: &T) {
    value.encode(&mut output[offset..offset + core::mem::size_of::<T>()]);
}

macro_rules! impl_struct_read {
    ($ty:ty { $($field:ident : $field_ty:ty),* $(,)? }) => {
        impl UserRead for $ty {
            fn decode(input: &[u8]) -> Self {
                Self {
                    $($field: decode_field::<$field_ty>(
                        input,
                        core::mem::offset_of!(Self, $field),
                    )),*
                }
            }
        }
    };
}

macro_rules! impl_struct_write {
    ($ty:ty { $($field:ident),* $(,)? }) => {
        impl UserWrite for $ty {
            fn encode(&self, output: &mut [u8]) {
                output.fill(0);
                $(encode_field(
                    output,
                    core::mem::offset_of!(Self, $field),
                    &self.$field,
                );)*
            }
        }
    };
}

macro_rules! impl_integer_codec {
    ($($ty:ty),* $(,)?) => {
        $(
            impl UserRead for $ty {
                fn decode(input: &[u8]) -> Self {
                    Self::from_ne_bytes(input.try_into().expect("integer ABI size"))
                }
            }

            impl UserWrite for $ty {
                fn encode(&self, output: &mut [u8]) {
                    output.copy_from_slice(&self.to_ne_bytes());
                }
            }
        )*
    };
}

impl_integer_codec!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

impl<T, const N: usize> UserRead for [T; N]
where
    T: UserRead,
{
    fn decode(input: &[u8]) -> Self {
        let size = core::mem::size_of::<T>();
        core::array::from_fn(|index| T::decode(&input[index * size..(index + 1) * size]))
    }
}

impl<T, const N: usize> UserWrite for [T; N]
where
    T: UserWrite,
{
    fn encode(&self, output: &mut [u8]) {
        let size = core::mem::size_of::<T>();
        for (value, bytes) in self.iter().zip(output.chunks_exact_mut(size)) {
            value.encode(bytes);
        }
    }
}

impl<T> UserRead for UserPtr<T> {
    fn decode(input: &[u8]) -> Self {
        usize::decode(input).into()
    }
}

impl<T> UserRead for UserConstPtr<T> {
    fn decode(input: &[u8]) -> Self {
        usize::decode(input).into()
    }
}

impl<T> UserWrite for UserPtr<T> {
    fn encode(&self, output: &mut [u8]) {
        self.address().as_usize().encode(output);
    }
}

impl<T> UserWrite for UserConstPtr<T> {
    fn encode(&self, output: &mut [u8]) {
        self.address().as_usize().encode(output);
    }
}

impl UserRead for SignalSet {
    fn decode(input: &[u8]) -> Self {
        SignalSet::from_bits(u64::decode(input))
    }
}

impl UserWrite for SignalSet {
    fn encode(&self, output: &mut [u8]) {
        self.bits().encode(output);
    }
}

impl_struct_read!(UserSignalAction {
    handler: usize,
    flags: u64,
    mask: SignalSet,
});
impl_struct_write!(UserSignalAction {
    handler,
    flags,
    mask,
});

impl UserRead for SignalInfo {
    fn decode(input: &[u8]) -> Self {
        SignalInfo::from_bytes(input.try_into().expect("siginfo ABI size"))
    }
}

impl_struct_read!(SignalStack {
    sp: usize,
    flags: u32,
    size: usize,
});
impl_struct_write!(SignalStack { sp, flags, size });

impl UserRead for PollFd {
    fn decode(input: &[u8]) -> Self {
        Self {
            fd: decode_field(input, core::mem::offset_of!(Self, fd)),
            events: xutils::ctypes::fs::IoEvents::from_bits_retain(decode_field(
                input,
                core::mem::offset_of!(Self, events),
            )),
            revents: xutils::ctypes::fs::IoEvents::from_bits_retain(decode_field(
                input,
                core::mem::offset_of!(Self, revents),
            )),
        }
    }
}

impl UserWrite for PollFd {
    fn encode(&self, output: &mut [u8]) {
        output.fill(0);
        encode_field(output, core::mem::offset_of!(Self, fd), &self.fd);
        encode_field(
            output,
            core::mem::offset_of!(Self, events),
            &self.events.bits(),
        );
        encode_field(
            output,
            core::mem::offset_of!(Self, revents),
            &self.revents.bits(),
        );
    }
}

impl_struct_read!(__user_cap_header_struct {
    version: u32,
    pid: i32,
});
impl_struct_write!(__user_cap_header_struct { version, pid });
impl_struct_read!(__user_cap_data_struct {
    effective: u32,
    permitted: u32,
    inheritable: u32,
});
impl_struct_write!(__user_cap_data_struct {
    effective,
    permitted,
    inheritable,
});

impl_struct_read!(clone_args {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
});

impl_struct_read!(epoll_event {
    events: u32,
    data: u64,
});
impl_struct_write!(epoll_event { events, data });

impl_struct_read!(timespec {
    tv_sec: i64,
    tv_nsec: i64,
});
impl_struct_write!(timespec { tv_sec, tv_nsec });
impl_struct_read!(timeval {
    tv_sec: i64,
    tv_usec: i64,
});
impl_struct_write!(timeval { tv_sec, tv_usec });
impl_struct_read!(itimerval {
    it_interval: timeval,
    it_value: timeval,
});
impl_struct_write!(itimerval {
    it_interval,
    it_value,
});
impl_struct_read!(itimerspec {
    it_interval: timespec,
    it_value: timespec,
});
impl_struct_write!(itimerspec {
    it_interval,
    it_value,
});

impl_struct_read!(rlimit {
    rlim_cur: u64,
    rlim_max: u64,
});
impl_struct_write!(rlimit { rlim_cur, rlim_max });
impl_struct_read!(rlimit64 {
    rlim_cur: u64,
    rlim_max: u64,
});
impl_struct_write!(rlimit64 { rlim_cur, rlim_max });

impl_struct_read!(winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
});
impl_struct_write!(winsize {
    ws_row,
    ws_col,
    ws_xpixel,
    ws_ypixel,
});

impl_struct_read!(utimbuf {
    actime: i64,
    modtime: i64,
});

impl_struct_write!(Tms {
    tms_utime,
    tms_stime,
    tms_cutime,
    tms_cstime,
});
impl_struct_write!(rtc_time {
    tm_sec,
    tm_min,
    tm_hour,
    tm_mday,
    tm_mon,
    tm_year,
    tm_wday,
    tm_yday,
    tm_isdst,
});

impl_struct_read!(__kernel_fd_set {
    fds_bits: [u64; 16],
});
impl_struct_write!(__kernel_fd_set { fds_bits });

impl_struct_read!(sigset_t { sig: [u64; 1] });

impl_struct_write!(new_utsname {
    sysname,
    nodename,
    release,
    version,
    machine,
    domainname,
});

impl_struct_read!(loop_info {
    lo_number: i32,
    lo_device: u32,
    lo_inode: u64,
    lo_rdevice: u32,
    lo_offset: i32,
    lo_encrypt_type: i32,
    lo_encrypt_key_size: i32,
    lo_flags: i32,
    lo_name: [u8; 64],
    lo_encrypt_key: [u8; 32],
    lo_init: [u64; 2],
    reserved: [u8; 4],
});
impl_struct_write!(loop_info {
    lo_number,
    lo_device,
    lo_inode,
    lo_rdevice,
    lo_offset,
    lo_encrypt_type,
    lo_encrypt_key_size,
    lo_flags,
    lo_name,
    lo_encrypt_key,
    lo_init,
    reserved,
});

impl_struct_read!(termios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_line: u8,
    c_cc: [u8; 19],
});
impl_struct_write!(termios {
    c_iflag,
    c_oflag,
    c_cflag,
    c_lflag,
    c_line,
    c_cc,
});

impl UserRead for iovec {
    fn decode(input: &[u8]) -> Self {
        Self {
            iov_base: decode_field::<usize>(input, core::mem::offset_of!(Self, iov_base)) as *mut _,
            iov_len: decode_field(input, core::mem::offset_of!(Self, iov_len)),
        }
    }
}

impl_struct_write!(stat {
    st_dev,
    st_ino,
    st_mode,
    st_nlink,
    st_uid,
    st_gid,
    st_rdev,
    __pad1,
    st_size,
    st_blksize,
    __pad2,
    st_blocks,
    st_atime,
    st_atime_nsec,
    st_mtime,
    st_mtime_nsec,
    st_ctime,
    st_ctime_nsec,
    __unused4,
    __unused5,
});

impl_struct_write!(statx_timestamp {
    tv_sec,
    tv_nsec,
    __reserved,
});
impl_struct_write!(statx {
    stx_mask,
    stx_blksize,
    stx_attributes,
    stx_nlink,
    stx_uid,
    stx_gid,
    stx_mode,
    __spare0,
    stx_ino,
    stx_size,
    stx_blocks,
    stx_attributes_mask,
    stx_atime,
    stx_btime,
    stx_ctime,
    stx_mtime,
    stx_rdev_major,
    stx_rdev_minor,
    stx_dev_major,
    stx_dev_minor,
    stx_mnt_id,
    stx_dio_mem_align,
    stx_dio_offset_align,
    stx_subvol,
    stx_atomic_write_unit_min,
    stx_atomic_write_unit_max,
    stx_atomic_write_segments_max,
    stx_dio_read_offset_align,
    stx_atomic_write_unit_max_opt,
    __spare2,
    __spare3,
});

impl_struct_write!(__kernel_fsid_t { val });
impl_struct_write!(statfs {
    f_type,
    f_bsize,
    f_blocks,
    f_bfree,
    f_bavail,
    f_files,
    f_ffree,
    f_fsid,
    f_namelen,
    f_frsize,
    f_flags,
    f_spare,
});

impl_struct_write!(sysinfo {
    uptime,
    loads,
    totalram,
    freeram,
    sharedram,
    bufferram,
    totalswap,
    freeswap,
    procs,
    pad,
    totalhigh,
    freehigh,
    mem_unit,
});

impl_struct_write!(__kernel_old_timeval { tv_sec, tv_usec });
impl_struct_write!(rusage {
    ru_utime,
    ru_stime,
    ru_maxrss,
    ru_ixrss,
    ru_idrss,
    ru_isrss,
    ru_minflt,
    ru_majflt,
    ru_nswap,
    ru_inblock,
    ru_oublock,
    ru_msgsnd,
    ru_msgrcv,
    ru_nsignals,
    ru_nvcsw,
    ru_nivcsw,
});

impl_struct_read!(IpcPerm {
    key: i32,
    uid: u32,
    gid: u32,
    cuid: u32,
    cgid: u32,
    mode: u32,
    seq: u16,
    pad: u16,
    unused0: i64,
    unused1: i64,
});
impl_struct_write!(IpcPerm {
    key,
    uid,
    gid,
    cuid,
    cgid,
    mode,
    seq,
    pad,
    unused0,
    unused1,
});

impl_struct_read!(MsgidDs {
    msg_perm: IpcPerm,
    msg_stime: i64,
    msg_rtime: i64,
    msg_ctime: i64,
    msg_cbytes: u64,
    msg_qnum: u64,
    msg_qbytes: u64,
    msg_lspid: i32,
    msg_lrpid: i32,
});
impl_struct_write!(MsgidDs {
    msg_perm,
    msg_stime,
    msg_rtime,
    msg_ctime,
    msg_cbytes,
    msg_qnum,
    msg_qbytes,
    msg_lspid,
    msg_lrpid,
});

impl_struct_read!(SemBuf {
    sem_num: u16,
    sem_op: i16,
    sem_flg: u16,
});
impl_struct_read!(SemInfo {
    sem_perm: IpcPerm,
    sem_otime: i64,
    sem_ctime: i64,
    sem_nsems: u16,
    pad: u16,
    unused0: i64,
    unused1: i64,
});
impl_struct_write!(SemInfo {
    sem_perm,
    sem_otime,
    sem_ctime,
    sem_nsems,
    pad,
    unused0,
    unused1,
});

impl_struct_read!(ShmInfo {
    shm_perm: IpcPerm,
    shm_segsz: u64,
    shm_atime: i64,
    shm_dtime: i64,
    shm_ctime: i64,
    shm_cpid: i32,
    shm_lpid: i32,
    shm_nattch: u16,
});
impl_struct_write!(ShmInfo {
    shm_perm,
    shm_segsz,
    shm_atime,
    shm_dtime,
    shm_ctime,
    shm_cpid,
    shm_lpid,
    shm_nattch,
});

impl UserRead for robust_list {
    fn decode(input: &[u8]) -> Self {
        Self {
            next: usize::decode(input) as *mut _,
        }
    }
}

impl UserRead for robust_list_head {
    fn decode(input: &[u8]) -> Self {
        Self {
            list: decode_field(input, core::mem::offset_of!(Self, list)),
            futex_offset: decode_field(input, core::mem::offset_of!(Self, futex_offset)),
            list_op_pending: decode_field::<usize>(
                input,
                core::mem::offset_of!(Self, list_op_pending),
            ) as *mut _,
        }
    }
}

impl XUserSpace {
    pub(crate) fn read<P, T>(&self, ptr: P) -> LinuxResult<T>
    where
        P: UserReadable<T>,
        T: UserRead,
    {
        check_layout::<T>(ptr.address())?;
        let mut bytes = vec![0; core::mem::size_of::<T>()];
        self.copy_from_user(ptr.address(), &mut bytes)?;
        Ok(T::decode(&bytes))
    }

    pub(crate) fn read_slice<P, T>(&self, ptr: P, len: usize) -> LinuxResult<Vec<T>>
    where
        P: UserReadable<T>,
        T: UserRead,
    {
        let layout = Layout::array::<T>(len).map_err(|_| LinuxError::EINVAL)?;
        if len == 0 {
            return Ok(Vec::new());
        }
        check_layout::<T>(ptr.address())?;
        let mut bytes = vec![0; layout.size()];
        self.copy_from_user(ptr.address(), &mut bytes)?;

        let mut values = Vec::with_capacity(len);
        for input in bytes.chunks_exact(core::mem::size_of::<T>()) {
            values.push(T::decode(input));
        }
        Ok(values)
    }

    pub(crate) fn read_slice_to<P, T>(&self, ptr: P, output: &mut [T]) -> LinuxResult<()>
    where
        P: UserReadable<T>,
        T: UserRead,
    {
        let layout = Layout::array::<T>(output.len()).map_err(|_| LinuxError::EINVAL)?;
        if output.is_empty() {
            return Ok(());
        }
        check_layout::<T>(ptr.address())?;
        let mut bytes = vec![0; layout.size()];
        self.copy_from_user(ptr.address(), &mut bytes)?;

        for (value, input) in output
            .iter_mut()
            .zip(bytes.chunks_exact(core::mem::size_of::<T>()))
        {
            *value = T::decode(input);
        }
        Ok(())
    }

    pub(crate) fn write<T: UserWrite>(&self, ptr: UserPtr<T>, value: T) -> LinuxResult<()> {
        check_layout::<T>(ptr.address())?;
        let mut bytes = vec![0; core::mem::size_of::<T>()];
        value.encode(&mut bytes);
        self.copy_to_user(ptr.address(), &bytes)
    }

    pub(crate) fn write_slice<T: UserWrite>(
        &self,
        ptr: UserPtr<T>,
        values: &[T],
    ) -> LinuxResult<()> {
        let layout = Layout::array::<T>(values.len()).map_err(|_| LinuxError::EINVAL)?;
        check_layout::<T>(ptr.address())?;
        let mut bytes = vec![0; layout.size()];
        let size = core::mem::size_of::<T>();
        for (value, output) in values.iter().zip(bytes.chunks_exact_mut(size)) {
            value.encode(output);
        }
        self.copy_to_user(ptr.address(), &bytes)
    }
}

fn check_layout<T>(address: VirtAddr) -> LinuxResult<()> {
    if address.as_usize() & (core::mem::align_of::<T>() - 1) != 0 {
        return Err(LinuxError::EFAULT);
    }
    Ok(())
}
