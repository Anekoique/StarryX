use alloc::vec;
use core::ffi::c_int;
use core::cmp::min;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axio::{Seek, SeekFrom};

use axuspace::{UserConstPtr, UserPtr, UserSpaceAccess, nullable};
use xcore::{
    fs::{FileLike, get_file_like},
    mm::PAGE_CACHE_MANAGER,
    task::with_uspace,
};

use crate::{
    ctypes::{__kernel_off_t, iovec},
    fs::{File, with_file},
};

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

/// Truncate a file to a specified length.
///
/// # Arguments
/// * `fd` - File descriptor
/// * `length` - New length for the file
pub fn sys_ftruncate(fd: c_int, length: __kernel_off_t) -> LinuxResult<isize> {
    trace!("sys_ftruncate <= {} {}", fd, length);
    with_file(fd, FileFlags::WRITE, FileFlags::empty(), |file| {
        if let Some(cache) = PAGE_CACHE_MANAGER.get_cache(file.inner().inode()?) {
            cache.set_size(length as _);
        }
        file.inner().access(FileFlags::WRITE)?.set_len(length as _)
    })
    .map(|_| 0)
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
        file.inner()
            .access(FileFlags::WRITE)?
            .set_len(offset as u64 + len as u64)
    })
    .map(|_| 0)
}

/// Synchronize a file's in-core state with storage device.
///
/// # Arguments
/// * `fd` - File descriptor
pub fn sys_fsync(fd: c_int) -> LinuxResult<isize> {
    trace!("sys_fsync <= {}", fd);
    with_file(fd, FileFlags::empty(), FileFlags::empty(), |file| {
        PAGE_CACHE_MANAGER.sync_file(file.inner().inode()?)?;
        file.inner().sync(false)
    })
    .map(|_| 0)
}

/// Synchronize a file's in-core data with storage device.
///
/// # Arguments
/// * `fd` - File descriptor
pub fn sys_fdatasync(fd: c_int) -> LinuxResult<isize> {
    debug!("sys_fdatasync <= {}", fd);
    with_file(fd, FileFlags::WRITE, FileFlags::empty(), |file| {
        file.inner().sync(true)
    })
    .map(|_| 0)
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
    File::from_fd(fd, FileFlags::READ, FileFlags::PATH)?
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

fn do_sendfile<F, D>(mut read: F, dest: &D) -> LinuxResult<usize>
where
    F: FnMut(&mut [u8]) -> LinuxResult<usize>,
    D: FileLike + ?Sized,
{
    let mut buf = vec![0; 0x4000];
    let mut total_written = 0;
    loop {
        let bytes_read = read(&mut buf)?;
        if bytes_read == 0 {
            break;
        }

        let bytes_written = dest.write(&buf[..bytes_read])?;
        if bytes_written < bytes_read {
            break;
        }
        total_written += bytes_written;
    }

    Ok(total_written)
}

/// Transfer data between file descriptors.
///
/// # Arguments
/// * `out_fd` - Output file descriptor
/// * `in_fd` - Input file descriptor
/// * `offset` - Pointer to offset in input file (NULL for current position)
/// * `len` - Maximum number of bytes to transfer
pub fn sys_sendfile(
    out_fd: c_int,
    in_fd: c_int,
    offset: UserPtr<u64>,
    len: usize,
) -> LinuxResult<isize> {
    trace!(
        "sys_sendfile <= out_fd: {}, in_fd: {}, offset: {}, len: {}",
        out_fd,
        in_fd,
        !offset.is_null(),
        len
    );

    with_file(out_fd, FileFlags::WRITE, FileFlags::PATH, |dest| {
        let offset = with_uspace(|uspace| nullable!(uspace.read(offset)))?;

        let result = match offset {
            Some(mut offset) => with_file(in_fd, FileFlags::READ, FileFlags::PATH, |src_file| {
                do_sendfile(
                    |buf| {
                        let bytes_read = src_file.read_at(buf, offset)?;
                        offset += bytes_read as u64;
                        Ok(bytes_read)
                    },
                    dest.as_ref(),
                )
            }),
            None => do_sendfile(|buf| get_file_like(in_fd)?.read(buf), dest.as_ref()),
        }?;

        PAGE_CACHE_MANAGER.sync_file(dest.inner().inode()?)?;
        Ok(result as isize)
    })
}

/// Copy a range of data from one file to another.
///
/// # Arguments
/// * `fd_in` - Input file descriptor
/// * `off_in` - Pointer to offset in input file (NULL for current position)
/// * `fd_out` - Output file descriptor
/// * `off_out` - Pointer to offset in output file (NULL for current position)
/// * `len` - Maximum number of bytes to copy
/// * `flags` - Flags for the operation (currently unused)
pub fn sys_copy_file_range(
    fd_in: i32,
    off_in: UserPtr<__kernel_off_t>,
    fd_out: i32,
    off_out: UserPtr<__kernel_off_t>,
    len: usize,
    _flags: u32,
) -> LinuxResult<isize> {
    debug!(
        "sys_copy_file_range <= fd_in: {}, off_in: {:?}, fd_out: {}, off_out: {:?}, len: {}",
        fd_in,
        !off_in.is_null(),
        fd_out,
        !off_out.is_null(),
        len
    );

    // 使用与 sendfile 完全相同的模式
    with_file(fd_out, FileFlags::WRITE, FileFlags::PATH, |dest| {
        with_uspace(|uspace| {
            let in_offset = if off_in.is_null() {
                None
            } else {
                Some(uspace.read(off_in)?)
            };

            let out_offset = if off_out.is_null() {
                None
            } else {
                Some(uspace.read(off_out)?)
            };

            debug!("copy_file_range: in_offset={:?}, out_offset={:?}, len={}", in_offset, out_offset, len);

            let result = match (in_offset, out_offset) {
                // 两个偏移都是用户提供的
                (Some(mut in_off), Some(mut out_off)) => {
                    let copied = with_file(fd_in, FileFlags::READ, FileFlags::PATH, |src_file| {
                        do_copy_explicit_offsets(src_file, dest, &mut in_off, &mut out_off, len)
                    })?;
                    uspace.write(off_in, in_off)?;
                    uspace.write(off_out, out_off)?;
                    debug!("copy_file_range: updated user offsets to in={}, out={}", in_off, out_off);
                    copied
                },
                // 输入使用用户偏移，输出使用文件位置  
                (Some(mut in_off), None) => {
                    let copied = with_file(fd_in, FileFlags::READ, FileFlags::PATH, |src_file| {
                        do_copy_in_explicit_out_current(src_file, dest, &mut in_off, len)
                    })?;
                    uspace.write(off_in, in_off)?;
                    debug!("copy_file_range: updated user in_offset to {}", in_off);
                    copied
                },
                // 输入使用文件位置，输出使用用户偏移
                (None, Some(mut out_off)) => {
                    let copied = do_copy_in_current_out_explicit(fd_in, dest, &mut out_off, len)?;
                    uspace.write(off_out, out_off)?;
                    debug!("copy_file_range: updated user out_offset to {}", out_off);
                    copied
                },
                // 两个都使用文件位置 - 最关键的情况，与 sendfile 完全相同
                (None, None) => {
                    do_copy_both_current(fd_in, dest, len)?
                },
            };

            debug!("copy_file_range: copied {} bytes", result);
            Ok(result as isize)
        })
    })
}

// 模仿 sys_sendfile 中处理 None offset 的方式
fn do_copy_both_current(
    fd_in: i32,
    dest: &File,
    len: usize,
) -> LinuxResult<usize> {
    debug!("do_copy_both_current: fd_in={}, len={}", fd_in, len);
    
    // 与 sys_sendfile 完全相同的模式：使用 get_file_like 获取输入文件
    let src = get_file_like(fd_in)?;
    
    let mut buf = vec![0u8; min(len, 0x4000)];
    let mut total_copied = 0;
    let mut remaining = len;

    while remaining > 0 {
        let to_read = min(remaining, buf.len());
        debug!("attempting to read {} bytes using current file position", to_read);
        
        let bytes_read = src.read(&mut buf[..to_read])?;
        debug!("read {} bytes from file", bytes_read);
        
        if bytes_read == 0 {
            debug!("EOF reached");
            break;
        }

        debug!("attempting to write {} bytes using current file position", bytes_read);
        let bytes_written = dest.write(&buf[..bytes_read])?;
        debug!("wrote {} bytes to file", bytes_written);
        
        total_copied += bytes_written;
        
        if bytes_written < bytes_read {
            debug!("partial write: {} < {}, stopping", bytes_written, bytes_read);
            break;
        }
        
        remaining -= bytes_written;
        debug!("copy progress: copied={}, remaining={}", total_copied, remaining);
    }

    debug!("do_copy_both_current: completed, total_copied={}", total_copied);
    Ok(total_copied)
}

fn do_copy_explicit_offsets(
    src_file: &File,
    dest_file: &File,
    in_off: &mut __kernel_off_t,
    out_off: &mut __kernel_off_t,
    len: usize,
) -> LinuxResult<usize> {
    debug!("do_copy_explicit_offsets: in_off={}, out_off={}, len={}", *in_off, *out_off, len);
    
    if len == 0 {
        debug!("do_copy_explicit_offsets: len=0, returning 0");
        return Ok(0);
    }

    let mut buf = vec![0u8; min(len, 0x4000)];
    let mut total_copied = 0;
    let mut remaining = len;

    while remaining > 0 {
        let to_read = min(remaining, buf.len());
        
        let bytes_read = src_file.read_at(&mut buf[..to_read], *in_off as u64)?;
        if bytes_read == 0 {
            break;
        }

        let bytes_written = dest_file.write_at(&buf[..bytes_read], *out_off as u64)?;
        
        *in_off += bytes_written as __kernel_off_t;
        *out_off += bytes_written as __kernel_off_t;
        total_copied += bytes_written;
        
        if bytes_written < bytes_read {
            break;
        }
        
        remaining -= bytes_written;
    }

    Ok(total_copied)
}

fn do_copy_in_explicit_out_current(
    src_file: &File,
    dest_file: &File,
    in_off: &mut __kernel_off_t,
    len: usize,
) -> LinuxResult<usize> {
    debug!("do_copy_in_explicit_out_current: in_off={}, len={}", *in_off, len);
    
    if len == 0 {
        return Ok(0);
    }

    let mut buf = vec![0u8; min(len, 0x4000)];
    let mut total_copied = 0;
    let mut remaining = len;

    while remaining > 0 {
        let to_read = min(remaining, buf.len());
        
        let bytes_read = src_file.read_at(&mut buf[..to_read], *in_off as u64)?;
        if bytes_read == 0 {
            break;
        }

        let bytes_written = dest_file.write(&buf[..bytes_read])?;
        
        *in_off += bytes_written as __kernel_off_t;
        total_copied += bytes_written;
        
        if bytes_written < bytes_read {
            break;
        }
        
        remaining -= bytes_written;
    }

    Ok(total_copied)
}

fn do_copy_in_current_out_explicit(
    fd_in: i32,
    dest_file: &File,
    out_off: &mut __kernel_off_t,
    len: usize,
) -> LinuxResult<usize> {
    debug!("do_copy_in_current_out_explicit: out_off={}, len={}", *out_off, len);
    
    if len == 0 {
        return Ok(0);
    }

    // 与 sendfile 相同：使用 get_file_like 获取使用当前位置的文件
    let src = get_file_like(fd_in)?;
    let mut buf = vec![0u8; min(len, 0x4000)];
    let mut total_copied = 0;
    let mut remaining = len;

    while remaining > 0 {
        let to_read = min(remaining, buf.len());
        
        let bytes_read = src.read(&mut buf[..to_read])?;
        if bytes_read == 0 {
            break;
        }

        let bytes_written = dest_file.write_at(&buf[..bytes_read], *out_off as u64)?;
        
        *out_off += bytes_written as __kernel_off_t;
        total_copied += bytes_written;
        
        if bytes_written < bytes_read {
            break;
        }
        
        remaining -= bytes_written;
    }

    Ok(total_copied)
}

pub fn sys_splice(
    fd_in: i32,
    off_in: UserPtr<__kernel_off_t>,
    fd_out: i32,
    off_out: UserPtr<__kernel_off_t>,
    len: usize,
    _flags: u32,
) -> LinuxResult<isize> {
    debug!(
        "sys_splice <= fd_in: {}, off_in: {:?}, fd_out: {}, off_out: {:?}, len: {}, flags: {}",
        fd_in,
        !off_in.is_null(),
        fd_out,
        !off_out.is_null(),
        len,
        _flags
    );
    
    with_uspace(|uspace| {
        // Validate offset parameters and check for negative values
        let in_offset = if off_in.is_null() {
            None
        } else {
            let offset = uspace.read(off_in)?;
            if offset < 0 {
                return Err(LinuxError::EINVAL);
            }
            Some(offset)
        };

        let out_offset = if off_out.is_null() {
            None
        } else {
            let offset = uspace.read(off_out)?;
            if offset < 0 {
                return Err(LinuxError::EINVAL);
            }
            Some(offset)
        };

        // Determine which one is the pipe and which is the file
        // If off_in is NULL, fd_in is a pipe
        // If off_out is NULL, fd_out is a pipe
        let result = match (in_offset, out_offset) {
            // fd_in is pipe, fd_out is file
            (None, Some(mut out_off)) => {
                debug!("splice: pipe to file, fd_in={}, fd_out={}, out_off={}", fd_in, fd_out, out_off);
                with_file(fd_out, FileFlags::WRITE, FileFlags::PATH, |dest_file| {
                    let copied = do_splice_pipe_to_file(fd_in, dest_file, &mut out_off, len)?;
                    uspace.write(off_out, out_off)?;
                    debug!("splice: pipe to file completed, copied={}", copied);
                    Ok(copied)
                })
            },
            // fd_in is file, fd_out is pipe
            (Some(mut in_off), None) => {
                debug!("splice: file to pipe, fd_in={}, fd_out={}, in_off={}", fd_in, fd_out, in_off);
                let copied = do_splice_file_to_pipe(fd_in, fd_out, &mut in_off, len)?;
                uspace.write(off_in, in_off)?;
                debug!("splice: file to pipe completed, copied={}", copied);
                Ok(copied)
            },
            // Invalid combinations - both should not be NULL or both should not be non-NULL
            _ => {
                debug!("splice: invalid offset combination");
                Err(LinuxError::EINVAL)
            },
        }?;

        debug!("sys_splice => result={}", result);
        Ok(result as isize)
    })
}

fn do_splice_file_to_pipe(
    fd_in: i32,
    fd_out: i32,
    in_off: &mut __kernel_off_t,
    len: usize,
) -> LinuxResult<usize> {
    let src_file = File::from_fd(fd_in, FileFlags::READ, FileFlags::PATH)?;
    let dest = get_file_like(fd_out)?;
    let mut buf = vec![0u8; min(len, 0x4000)];
    let mut total_copied = 0;
    let mut remaining = len;

    while remaining > 0 {
        let to_read = min(remaining, buf.len());
        let bytes_read = src_file.read_at(&mut buf[..to_read], *in_off as u64)?;
        
        if bytes_read == 0 {
            break; // EOF reached
        }

        // For pipes, we try to write all data and block if necessary
        let mut written = 0;
        while written < bytes_read {
            let bytes_written = dest.write(&buf[written..bytes_read])?;
            if bytes_written == 0 {
                break; // Pipe closed or error
            }
            written += bytes_written;
        }
        
        *in_off += bytes_read as __kernel_off_t;
        total_copied += written;
        
        if written < bytes_read {
            break; // Partial write to pipe
        }
        
        remaining -= written;
    }

    Ok(total_copied)
}

fn do_splice_pipe_to_file(
    fd_in: i32,
    dest_file: &File,
    out_off: &mut __kernel_off_t,
    len: usize,
) -> LinuxResult<usize> {
    debug!("do_splice_pipe_to_file: start, out_off={}, len={}", *out_off, len);
    
    let src = get_file_like(fd_in)?;
    let mut buf = vec![0u8; min(len, 0x4000)];
    let mut total_copied = 0;
    let mut remaining = len;

    while remaining > 0 {
        let to_read = min(remaining, buf.len());
        debug!("attempting to read {} bytes from pipe", to_read);
        
        // For pipes, read whatever is available, but handle potential blocking
        let bytes_read = match src.read(&mut buf[..to_read]) {
            Ok(n) => {
                debug!("read {} bytes from pipe", n);
                n
            },
            Err(e) => {
                debug!("pipe read failed: {:?}", e);
                return Err(e);
            }
        };
        
        if bytes_read == 0 {
            debug!("pipe read returned 0, pipe may be empty or closed");
            break; // Pipe closed or no more data
        }

        debug!("attempting to write {} bytes to file at offset {}", bytes_read, *out_off);
        let bytes_written = dest_file.write_at(&buf[..bytes_read], *out_off as u64)?;
        debug!("wrote {} bytes to file", bytes_written);
        
        *out_off += bytes_written as __kernel_off_t;
        total_copied += bytes_written;
        
        if bytes_written < bytes_read {
            debug!("partial write to file: {} < {}", bytes_written, bytes_read);
            break; // Partial write to file
        }
        
        remaining -= bytes_written;
        debug!("splice progress: copied={}, remaining={}", total_copied, remaining);
        
        // For splice pipe-to-file, if we got less data than requested from pipe,
        // it's normal and we should return what we have
        if bytes_read < to_read {
            debug!("read less than requested from pipe, this is normal");
            break;
        }
        
        // Also break if we've made some progress - splice doesn't need to transfer
        // all requested bytes in one call
        if total_copied > 0 && bytes_read < buf.len() {
            debug!("made progress and got partial read, returning");
            break;
        }
    }

    debug!("do_splice_pipe_to_file: completed, total_copied={}", total_copied);
    Ok(total_copied)
}