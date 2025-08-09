use alloc::{sync::Arc, vec};
use core::ffi::{c_char, c_int};

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axio::{Seek, SeekFrom};

use xcore::{
    fs::{
        fd::{File, Pipe, get_file_like},
        file::FileLike,
        with_file, with_fs,
    },
    mm::PAGE_CACHE_MANAGER,
    resources::AX_FSIZE_LIMIT,
    task::with_uspace,
};
use xuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xutils::ctypes::{__kernel_off_t, AT_FDCWD, iovec};

/// Read data from the file indicated by `fd`.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `buf` - Buffer to read data into
/// * `len` - Length of data to read
pub fn sys_read(fd: i32, buf: UserPtr<u8>, len: usize) -> LinuxResult<isize> {
    let buf = with_uspace(|uspace| uspace.raw_slice(buf, len))?;
    debug!(
        "sys_read <= fd: {}, buf: {:p}, len: {}",
        fd,
        buf.as_ptr(),
        buf.len()
    );
    Ok(get_file_like(fd)?.read(buf)? as _)
}

fn readv_impl(
    iov: UserPtr<iovec>,
    iocnt: usize,
    mut f: impl FnMut(&mut [u8]) -> LinuxResult<usize>,
) -> LinuxResult<isize> {
    if !(0..=1024).contains(&iocnt) {
        return Err(LinuxError::EINVAL);
    }

    with_uspace(|uspace| {
        let iovs = uspace.raw_slice(iov, iocnt)?;
        let mut total = 0;

        for iov in iovs.iter().filter(|iov| iov.iov_len > 0) {
            let buf =
                uspace.raw_slice(UserPtr::<u8>::from(iov.iov_base as usize), iov.iov_len as _)?;

            let read = f(buf)?;
            total += read;

            if read < buf.len() {
                break;
            }
        }

        Ok(total as isize)
    })
}

fn writev_impl(
    iov: UserConstPtr<iovec>,
    iocnt: usize,
    mut f: impl FnMut(&[u8]) -> LinuxResult<usize>,
) -> LinuxResult<isize> {
    if !(0..=1024).contains(&iocnt) {
        return Err(LinuxError::EINVAL);
    }

    with_uspace(|uspace| {
        let iovs = uspace.read_slice(iov, iocnt)?;
        let mut total = 0;

        for iov in iovs.iter().filter(|iov| iov.iov_len > 0) {
            let buf = uspace.read_slice(
                UserConstPtr::<u8>::from(iov.iov_base as usize),
                iov.iov_len as _,
            )?;

            let written = f(buf)?;
            total += written;

            if written < buf.len() {
                break;
            }
        }

        Ok(total as isize)
    })
}

/// Read data from multiple buffers from the file indicated by `fd`.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `iov` - Array of iovec structures
/// * `iocnt` - Number of iovec structures
pub fn sys_readv(fd: i32, iov: UserPtr<iovec>, iocnt: usize) -> LinuxResult<isize> {
    debug!("sys_readv <= fd: {}, iov: {:?}, iocnt: {}", fd, iov, iocnt);
    let f = get_file_like(fd)?;
    readv_impl(iov, iocnt, |buf| f.read(buf))
}

/// Write data to the file indicated by `fd`.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `buf` - Buffer containing data to write
/// * `len` - Length of data to write
pub fn sys_write(fd: i32, buf: UserConstPtr<u8>, len: usize) -> LinuxResult<isize> {
    let buf = with_uspace(|uspace| uspace.read_slice(buf, len))?;
    debug!(
        "sys_write <= fd: {}, buf: {:p}, len: {}",
        fd,
        buf.as_ptr(),
        buf.len()
    );
    Ok(get_file_like(fd)?.write(buf)? as _)
}

/// Write data from multiple buffers to the file indicated by `fd`.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `iov` - Array of iovec structures
/// * `iocnt` - Number of iovec structures
pub fn sys_writev(fd: i32, iov: UserConstPtr<iovec>, iocnt: usize) -> LinuxResult<isize> {
    debug!("sys_writev <= fd: {}, iov: {:?}, iocnt: {}", fd, iov, iocnt);
    let f = get_file_like(fd)?;
    writev_impl(iov, iocnt, |buf| f.write(buf))
}

/// Reposition read/write file offset.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `offset` - Offset value
/// * `whence` - How to interpret the offset (SEEK_SET, SEEK_CUR, SEEK_END)
pub fn sys_lseek(fd: c_int, offset: __kernel_off_t, whence: c_int) -> LinuxResult<isize> {
    trace!("sys_lseek <= {} {} {}", fd, offset, whence);
    let pos = match whence {
        0 => SeekFrom::Start(offset as _),
        1 => SeekFrom::Current(offset as _),
        2 => SeekFrom::End(offset as _),
        _ => return Err(LinuxError::EINVAL),
    };
    let off = File::from_fd(fd, FileFlags::empty(), FileFlags::empty())?
        .inner()
        .seek(pos)?;
    Ok(off as _)
}

pub fn sys_truncate(path: UserConstPtr<c_char>, length: __kernel_off_t) -> LinuxResult<isize> {
    if length < 0 {
        return Err(LinuxError::EINVAL);
    } else if length > AX_FSIZE_LIMIT as _ {
        return Err(LinuxError::EFBIG);
    }
    let path = with_uspace(|uspace| uspace.read_str(path))?;
    trace!("sys_truncate <= {} {}", path, length);
    with_fs(AT_FDCWD, path, |fs| {
        PAGE_CACHE_MANAGER
            .get_cache(fs.write_file(path)?.access(FileFlags::WRITE)?.inode())
            .map(|inode| inode.evict_from_pos(length as _))
            .unwrap_or(Ok(()))
    })?;
    Ok(0)
}

/// Truncate a file to a specified length.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `length` - New length for the file
pub fn sys_ftruncate(fd: c_int, length: __kernel_off_t) -> LinuxResult<isize> {
    trace!("sys_ftruncate <= {} {}", fd, length);
    if length < 0 {
        return Err(LinuxError::EINVAL);
    }
    with_file(fd, FileFlags::WRITE, FileFlags::empty(), |file| {
        file.set_len(length as _)
    })?;
    Ok(0)
}

/// Allocate space in a file.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `mode` - Allocation mode (currently unused)
/// * `offset` - Offset to allocate from
/// * `len` - Length of the allocation
pub fn sys_fallocate(
    fd: c_int,
    mode: u32,
    offset: __kernel_off_t,
    len: __kernel_off_t,
) -> LinuxResult<isize> {
    trace!(
        "sys_fallocate <= fd: {}, mode: {}, offset: {}, len: {}",
        fd, mode, offset, len
    );
    if mode != 0 {
        return Ok(0);
    }
    with_file(fd, FileFlags::WRITE, FileFlags::empty(), |file| {
        file.set_len(offset as u64 + len as u64)
    })?;
    Ok(0)
}

/// Synchronize a file's in-core state with storage device.
///
/// # Arguments
/// * `fd` - File descriptor
pub fn sys_fsync(fd: c_int) -> LinuxResult<isize> {
    trace!("sys_fsync <= {}", fd);
    with_file(fd, FileFlags::empty(), FileFlags::empty(), |file| {
        file.sync(false)
    })?;
    Ok(0)
}

/// Synchronize a file's in-core data with storage device.
///
/// # Arguments
/// * `fd` - File descriptor
pub fn sys_fdatasync(fd: c_int) -> LinuxResult<isize> {
    debug!("sys_fdatasync <= {}", fd);
    with_file(fd, FileFlags::WRITE, FileFlags::empty(), |file| {
        file.sync(true)
    })?;
    Ok(0)
}

/// Read from a file descriptor at a given offset.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `buf` - Buffer to read data into
/// * `len` - Length of data to read
/// * `offset` - Offset to read from
pub fn sys_pread64(
    fd: c_int,
    buf: UserPtr<u8>,
    len: usize,
    offset: __kernel_off_t,
) -> LinuxResult<isize> {
    let buf = with_uspace(|uspace| uspace.raw_slice(buf, len))?;
    trace!("sys_pread64 <= {}", fd);
    if offset < 0 {
        return Err(LinuxError::EINVAL);
    }
    File::from_fd(fd, FileFlags::empty(), FileFlags::PATH)?
        .read_at(buf, offset as _)
        .map(|read| read as isize)
}

/// Write to a file descriptor at a given offset.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `buf` - Buffer containing data to write
/// * `len` - Length of data to write
/// * `offset` - Offset to write to
pub fn sys_pwrite64(
    fd: c_int,
    buf: UserConstPtr<u8>,
    len: usize,
    offset: __kernel_off_t,
) -> LinuxResult<isize> {
    let buf = with_uspace(|uspace| uspace.read_slice(buf, len))?;
    trace!("sys_pwrite64 <= {}", fd);
    File::from_fd(fd, FileFlags::WRITE, FileFlags::PATH)?
        .write_at(buf, offset as _)
        .map(|written| written as isize)
}

/// Read data into multiple buffers from a file descriptor at a given offset.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `iov` - Array of iovec structures
/// * `iocnt` - Number of iovec structures
/// * `offset` - Offset to read from
pub fn sys_preadv(
    fd: c_int,
    iov: UserPtr<iovec>,
    iocnt: usize,
    offset: __kernel_off_t,
) -> LinuxResult<isize> {
    sys_preadv2(fd, iov, iocnt, offset, 0)
}

/// Write data from multiple buffers to a file descriptor at a given offset.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `iov` - Array of iovec structures
/// * `iocnt` - Number of iovec structures
/// * `offset` - Offset to write to
pub fn sys_pwritev(
    fd: c_int,
    iov: UserConstPtr<iovec>,
    iocnt: usize,
    offset: __kernel_off_t,
) -> LinuxResult<isize> {
    sys_pwritev2(fd, iov, iocnt, offset, 0)
}

/// Read data into multiple buffers from a file descriptor at a given offset with flags.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `iov` - Array of iovec structures
/// * `iocnt` - Number of iovec structures
/// * `offset` - Offset to read from
/// * `flags` - Flags for the operation (currently unused)
pub fn sys_preadv2(
    fd: c_int,
    iov: UserPtr<iovec>,
    iocnt: usize,
    mut offset: __kernel_off_t,
    _flags: u32,
) -> LinuxResult<isize> {
    with_file(fd, FileFlags::READ, FileFlags::PATH, |file| {
        readv_impl(iov, iocnt, |buf| {
            let read = file.read_at(buf, offset as _)?;
            offset += read as __kernel_off_t;
            Ok(read)
        })
    })
}

/// Write data from multiple buffers to a file descriptor at a given offset with flags.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `iov` - Array of iovec structures
/// * `iocnt` - Number of iovec structures
/// * `offset` - Offset to write to
/// * `flags` - Flags for the operation (currently unused)
pub fn sys_pwritev2(
    fd: c_int,
    iov: UserConstPtr<iovec>,
    iocnt: usize,
    mut offset: __kernel_off_t,
    _flags: u32,
) -> LinuxResult<isize> {
    with_file(fd, FileFlags::WRITE, FileFlags::PATH, |file| {
        writev_impl(iov, iocnt, |buf| {
            let write = file.write_at(buf, offset as _)?;
            offset += write as __kernel_off_t;
            Ok(write)
        })
    })
}

enum SendFile<'a> {
    Direct(Arc<dyn FileLike>),
    Offset(Arc<File>, &'a mut i64),
}

impl SendFile<'_> {
    fn read(&mut self, buf: &mut [u8]) -> LinuxResult<usize> {
        match self {
            SendFile::Direct(file) => file.read(buf),
            SendFile::Offset(file, offset) => {
                let bytes_read = file.read_at(buf, **offset as _)?;
                **offset += bytes_read as i64;
                Ok(bytes_read)
            }
        }
    }

    fn write(&mut self, buf: &[u8]) -> LinuxResult<usize> {
        match self {
            SendFile::Direct(file) => file.write(buf),
            SendFile::Offset(file, offset) => {
                let bytes_written = file.write_at(buf, **offset as _)?;
                **offset += bytes_written as i64;
                Ok(bytes_written)
            }
        }
    }
}

fn do_send(mut src: SendFile<'_>, mut dst: SendFile<'_>, len: usize) -> LinuxResult<usize> {
    let mut buf = vec![0; 0x4000];
    let mut total_written = 0;
    let mut remaining = len;

    while remaining > 0 {
        let to_read = buf.len().min(remaining);
        let bytes_read = src.read(&mut buf[..to_read])?;
        if bytes_read == 0 {
            break;
        }

        let bytes_written = dst.write(&buf[..bytes_read])?;
        if bytes_written < bytes_read {
            break;
        }

        total_written += bytes_written;
        remaining -= bytes_written;
    }

    Ok(total_written)
}

pub fn sys_sendfile(
    out_fd: c_int,
    in_fd: c_int,
    offset: UserPtr<i64>,
    len: usize,
) -> LinuxResult<isize> {
    debug!(
        "sys_sendfile <= out_fd: {}, in_fd: {}, offset: {}, len: {}",
        out_fd,
        in_fd,
        !offset.is_null(),
        len
    );

    with_uspace(|uspace| {
        let off = nullable!(uspace.raw_ptr(offset))?;
        let src = if let Some(off) = off {
            if *off < 0 {
                return Err(LinuxError::EINVAL);
            }
            SendFile::Offset(File::from_fd(in_fd, FileFlags::READ, FileFlags::PATH)?, off)
        } else {
            SendFile::Direct(get_file_like(in_fd)?.file.clone())
        };

        let dst = SendFile::Direct(get_file_like(out_fd)?.file.clone());
        do_send(src, dst, len).map(|n| n as _)
    })
}

pub fn sys_splice(
    fd_in: c_int,
    off_in: UserPtr<i64>,
    fd_out: c_int,
    off_out: UserPtr<i64>,
    mut len: usize,
    _flags: u32,
) -> LinuxResult<isize> {
    debug!(
        "sys_splice <= fd_in: {}, off_in: {}, fd_out: {}, off_out: {}, len: {}, flags: {}",
        fd_in,
        !off_in.is_null(),
        fd_out,
        !off_out.is_null(),
        len,
        _flags
    );

    if !(Pipe::from_fd(fd_in, FileFlags::READ, FileFlags::PATH).is_ok()
        || Pipe::from_fd(fd_out, FileFlags::WRITE, FileFlags::PATH).is_ok())
    {
        return Err(LinuxError::EINVAL);
    }
    with_uspace(|uspace| {
        let off = nullable!(uspace.raw_ptr(off_in))?;
        let src = if let Some(off) = off {
            if *off < 0 {
                return Err(LinuxError::EINVAL);
            }
            SendFile::Offset(File::from_fd(fd_in, FileFlags::READ, FileFlags::PATH)?, off)
        } else {
            if let Ok(src) = Pipe::from_fd(fd_in, FileFlags::READ, FileFlags::PATH) {
                if !src.readable() {
                    return Err(LinuxError::EBADF);
                }
                if !src.poll()?.readable {
                    return Err(LinuxError::EINVAL);
                }
                len = len.min(src.available_read());
            }
            SendFile::Direct(get_file_like(fd_in)?.file.clone())
        };

        let off = nullable!(uspace.raw_ptr(off_out))?;
        let dst = if let Some(off) = off {
            if *off < 0 {
                return Err(LinuxError::EINVAL);
            }
            SendFile::Offset(
                File::from_fd(fd_out, FileFlags::WRITE, FileFlags::PATH)?,
                off,
            )
        } else {
            if let Ok(src) = Pipe::from_fd(fd_in, FileFlags::WRITE, FileFlags::PATH)
                && !src.writable()
            {
                return Err(LinuxError::EBADF);
            }
            SendFile::Direct(get_file_like(fd_out)?.file.clone())
        };

        do_send(src, dst, len).map(|n| n as _)
    })
}

/// Copy a range of data from one file to another.
///
/// # Arguments
/// * `in_fd` - Input file descriptor
/// * `in_off_ptr` - Pointer to offset in input file
/// * `out_fd` - Output file descriptor
/// * `out_off_ptr` - Pointer to offset in output file
/// * `len` - Length of data to copy
/// * `flags` - Flags for the operation (currently unused)
pub fn sys_copy_file_range(
    in_fd: c_int,
    in_off: UserPtr<u64>,
    out_fd: c_int,
    out_off: UserPtr<u64>,
    len: u64,
    _flags: usize,
) -> LinuxResult<isize> {
    trace!(
        "sys_copy_file_range <= in_fd: {}, in_off: {:?}, out_fd: {}, out_off: {:?}, len: {}, flags: {}",
        in_fd, in_off, out_fd, out_off, len, _flags
    );

    let in_file = File::from_fd(in_fd, FileFlags::READ, FileFlags::PATH)?;
    let out_file = File::from_fd(out_fd, FileFlags::WRITE, FileFlags::PATH)?;
    let mut buf = vec![0; len as _];

    with_uspace(|uspace| {
        let new_len = if let Some(off) = nullable!(uspace.read(in_off))? {
            if off > in_file.len()? {
                return Ok(0);
            }
            let r = in_file.read_at(&mut buf, off as _)?;
            uspace.write(in_off, off + r as u64).map(|_| r)
        } else {
            let position = in_file.inner().position;
            if position > in_file.len()? {
                return Ok(0);
            }
            in_file.read(&mut buf)
        }?;

        buf.truncate(new_len);
        let result = if let Some(off) = nullable!(uspace.read(out_off))? {
            let r = out_file.write_at(&buf, off as _)?;
            uspace.write(out_off, off + r as u64).map(|_| r)
        } else {
            out_file.write(&buf)
        }?;

        Ok(result as isize)
    })
}
