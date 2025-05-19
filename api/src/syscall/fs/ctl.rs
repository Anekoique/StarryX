use core::{
    ffi::{c_char, c_int, c_void},
    mem::offset_of,
};

use alloc::ffi::CString;
use axerrno::{LinuxError, LinuxResult};
use axfs::{
    api::{TimesMask, Timestamp},
    fops::DirEntry,
};
use axtask::{TaskExtRef, current};
use linux_raw_sys::general::{
    AT_FDCWD, AT_REMOVEDIR, DT_BLK, DT_CHR, DT_DIR, DT_FIFO, DT_LNK, DT_REG, DT_SOCK, DT_UNKNOWN,
    UTIME_NOW, UTIME_OMIT, linux_dirent64, timespec,
};

use crate::{
    fs::{
        Directory, FileLike, HARDLINK_MANAGER, get_file_like, get_file_like_at, handle_file_path,
    },
    ptr::{UserConstPtr, UserPtr, nullable},
    utils::time::wall_time,
};

/// The ioctl() system call manipulates the underlying device parameters
/// of special files.
///
/// # Arguments
/// * `fd` - The file descriptor
/// * `op` - The request code. It is of type unsigned long in glibc and BSD,
///   and of type int in musl and other UNIX systems.
/// * `argp` - The argument to the request. It is a pointer to a memory location
pub fn sys_ioctl(_fd: i32, _op: usize, _argp: UserPtr<c_void>) -> LinuxResult<isize> {
    warn!("Unimplemented syscall: SYS_IOCTL");
    Ok(0)
}

pub fn sys_chdir(path: UserConstPtr<c_char>) -> LinuxResult<isize> {
    let path = path.get_as_str()?;
    debug!("sys_chdir <= {:?}", path);

    axfs::api::set_current_dir(path)?;
    Ok(0)
}

pub fn sys_mkdir(path: UserConstPtr<c_char>, mode: u32) -> LinuxResult<isize> {
    sys_mkdirat(AT_FDCWD, path, mode)
}

pub fn sys_mkdirat(dirfd: i32, path: UserConstPtr<c_char>, mode: u32) -> LinuxResult<isize> {
    let path = path.get_as_str()?;
    debug!(
        "sys_mkdirat <= dirfd: {}, path: {}, mode: {}",
        dirfd, path, mode
    );

    if mode != 0 {
        warn!("directory mode not supported.");
    }

    let path = handle_file_path(dirfd, path)?;
    axfs::api::create_dir(path.as_str())?;

    Ok(0)
}

#[allow(dead_code)]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum FileType {
    Unknown = DT_UNKNOWN as u8,
    Fifo = DT_FIFO as u8,
    Chr = DT_CHR as u8,
    Dir = DT_DIR as u8,
    Blk = DT_BLK as u8,
    Reg = DT_REG as u8,
    Lnk = DT_LNK as u8,
    Socket = DT_SOCK as u8,
}

impl From<axfs::api::FileType> for FileType {
    fn from(ft: axfs::api::FileType) -> Self {
        match ft {
            ft if ft.is_dir() => FileType::Dir,
            ft if ft.is_file() => FileType::Reg,
            _ => FileType::Unknown,
        }
    }
}

// Directory buffer for getdents64 syscall
struct DirBuffer<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> DirBuffer<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, offset: 0 }
    }

    fn remaining_space(&self) -> usize {
        self.buf.len().saturating_sub(self.offset)
    }

    fn write_entry(&mut self, d_type: FileType, name: &[u8]) -> bool {
        const NAME_OFFSET: usize = offset_of!(linux_dirent64, d_name);

        let len = NAME_OFFSET + name.len() + 1;
        // alignment
        let len = len.next_multiple_of(align_of::<linux_dirent64>());
        if self.remaining_space() < len {
            return false;
        }

        unsafe {
            let entry_ptr = self.buf.as_mut_ptr().add(self.offset);
            entry_ptr.cast::<linux_dirent64>().write(linux_dirent64 {
                // FIXME: real inode number
                d_ino: 1,
                d_off: 0,
                d_reclen: len as _,
                d_type: d_type as _,
                d_name: Default::default(),
            });

            let name_ptr = entry_ptr.add(NAME_OFFSET);
            name_ptr.copy_from_nonoverlapping(name.as_ptr(), name.len());
            name_ptr.add(name.len()).write(0);
        }

        self.offset += len;
        true
    }
}

pub fn sys_getdents64(fd: i32, buf: UserPtr<u8>, len: usize) -> LinuxResult<isize> {
    let buf = buf.get_as_mut_slice(len)?;
    debug!(
        "sys_getdents64 <= fd: {}, buf: {:p}, len: {}",
        fd,
        buf.as_ptr(),
        buf.len()
    );

    let mut buffer = DirBuffer::new(buf);

    let dir = Directory::from_fd(fd)?;

    let mut last_dirent = dir.last_dirent();
    if let Some(ent) = last_dirent.take() {
        if !buffer.write_entry(ent.entry_type().into(), ent.name_as_bytes()) {
            *last_dirent = Some(ent);
            return Err(LinuxError::EINVAL);
        }
    }

    let mut inner = dir.inner();
    loop {
        let mut dirents = [DirEntry::default()];
        let cnt = inner.read_dir(&mut dirents)?;
        if cnt == 0 {
            break;
        }

        let [ent] = dirents;
        if !buffer.write_entry(ent.entry_type().into(), ent.name_as_bytes()) {
            *last_dirent = Some(ent);
            break;
        }
    }

    if last_dirent.is_some() && buffer.offset == 0 {
        return Err(LinuxError::EINVAL);
    }
    Ok(buffer.offset as _)
}

/// create a link from new_path to old_path
/// old_path: old file path
/// new_path: new file path
/// flags: link flags
/// return value: return 0 when success, else return -1.
pub fn sys_linkat(
    old_dirfd: c_int,
    old_path: UserConstPtr<c_char>,
    new_dirfd: c_int,
    new_path: UserConstPtr<c_char>,
    flags: i32,
) -> LinuxResult<isize> {
    let old_path = old_path.get_as_str()?;
    let new_path = new_path.get_as_str()?;
    debug!(
        "sys_linkat <= old_dirfd: {}, old_path: {}, new_dirfd: {}, new_path: {}, flags: {}",
        old_dirfd, old_path, new_dirfd, new_path, flags
    );

    if flags != 0 {
        warn!("Unsupported flags: {flags}");
    }

    // handle old path
    let old_path = handle_file_path(old_dirfd, old_path)?;
    // handle new path
    let new_path = handle_file_path(new_dirfd, new_path)?;

    HARDLINK_MANAGER.create_link(&new_path, &old_path)?;

    Ok(0)
}

pub fn sys_link(
    old_path: UserConstPtr<c_char>,
    new_path: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    sys_linkat(AT_FDCWD, old_path, AT_FDCWD, new_path, 0)
}

/// remove link of specific file (can be used to delete file)
/// dir_fd: the directory of link to be removed
/// path: the name of link to be removed
/// flags: can be 0 or AT_REMOVEDIR
/// return 0 when success, else return -1
pub fn sys_unlinkat(dirfd: c_int, path: UserConstPtr<c_char>, flags: u32) -> LinuxResult<isize> {
    let path = path.get_as_str()?;
    debug!(
        "sys_unlinkat <= dirfd: {}, path: {}, flags: {}",
        dirfd, path, flags
    );

    let path = handle_file_path(dirfd, path)?;

    if flags == AT_REMOVEDIR {
        axfs::api::remove_dir(path.as_str())?;
    } else {
        let metadata = axfs::api::metadata(path.as_str())?;
        if metadata.is_dir() {
            return Err(LinuxError::EISDIR);
        } else {
            debug!("unlink file: {:?}", path);
            HARDLINK_MANAGER
                .remove_link(&path)
                .ok_or(LinuxError::ENOENT)?;
        }
    }
    Ok(0)
}

pub fn sys_rmdir(path: UserConstPtr<c_char>) -> LinuxResult<isize> {
    sys_unlinkat(AT_FDCWD, path, AT_REMOVEDIR)
}

pub fn sys_unlink(path: UserConstPtr<c_char>) -> LinuxResult<isize> {
    sys_unlinkat(AT_FDCWD, path, 0)
}

pub fn sys_getcwd(buf: UserPtr<u8>, size: usize) -> LinuxResult<isize> {
    let buf = nullable!(buf.get_as_mut_slice(size))?;

    let Some(buf) = buf else {
        return Ok(0);
    };

    let cwd = CString::new(axfs::api::current_dir()?).map_err(|_| LinuxError::EINVAL)?;
    let cwd = cwd.as_bytes_with_nul();

    if cwd.len() <= buf.len() {
        buf[..cwd.len()].copy_from_slice(cwd);
        Ok(buf.as_ptr() as _)
    } else {
        Err(LinuxError::ERANGE)
    }
}

pub fn sys_readlinkat(
    dirfd: c_int,
    path: UserConstPtr<c_char>,
    buf: UserPtr<u8>,
    size: usize,
) -> LinuxResult<isize> {
    let path = path.get_as_str()?;
    debug!(
        "sys_readlinkat <= dirfd: {}, path: {}, size: {}",
        dirfd, path, size
    );

    let path = handle_file_path(dirfd, path)?;

    // Get the target path for the symlink
    let link = if path.as_str() == "/proc/self/exe" {
        let curr = current();
        let exe_path = curr.task_ext().process_data().exe_path.read();
        debug!("exe_path: {:?}", exe_path);
        exe_path.clone()
    } else {
        let real_path = HARDLINK_MANAGER.real_path(path.as_str());
        if real_path == path.as_str() {
            return Err(LinuxError::EINVAL);
        }
        real_path
    };

    // Copy the link target path to the user buffer
    if let Some(buf) = nullable!(buf.get_as_mut_slice(size))? {
        let bytes = link.as_bytes();
        let len = size.min(bytes.len());
        buf[..len].copy_from_slice(&bytes[..len]);
        Ok(len as isize)
    } else {
        Ok(link.len() as isize)
    }
}

pub fn sys_readlink(
    path: UserConstPtr<c_char>,
    buf: UserPtr<u8>,
    size: usize,
) -> LinuxResult<isize> {
    sys_readlinkat(AT_FDCWD, path, buf, size)
}

pub fn sys_utimensat(
    dirfd: i32,
    path: UserConstPtr<c_char>,
    times: UserConstPtr<timespec>,
    _flags: i32,
) -> LinuxResult<isize> {
    let path = nullable!(path.get_as_str())?;

    let file = if path.is_none_or(|s| s.is_empty()) {
        get_file_like(dirfd)?
    } else {
        get_file_like_at(dirfd, path.unwrap_or_default())?
    };

    let current_time = wall_time();
    let now_sec = current_time.as_secs();
    let now_nsec = current_time.subsec_nanos();

    let mut mask = TimesMask::ALL - TimesMask::CTIME - TimesMask::CTIME_NSEC;
    // Process user-specified times or use default values
    let (atime_sec, atime_nsec, mtime_sec, mtime_nsec) = if let Some(times) =
        nullable!(times.get_as_slice(2))?
    {
        let (a_sec, a_nsec) = match times[0].tv_nsec as _ {
            UTIME_OMIT => {
                mask -= TimesMask::ATIME | TimesMask::ATIME_NSEC;
                (0, 0)
            }
            UTIME_NOW => (now_sec, now_nsec),
            _ => (times[0].tv_sec as u64, times[0].tv_nsec as u32),
        };

        let (m_sec, m_nsec) = match times[1].tv_nsec as _ {
            UTIME_OMIT => {
                mask -= TimesMask::MTIME | TimesMask::MTIME_NSEC;
                (0, 0)
            }
            UTIME_NOW => (now_sec, now_nsec),
            _ => (times[1].tv_sec as u64, times[1].tv_nsec as u32),
        };

        (a_sec, a_nsec, m_sec, m_nsec)
    } else {
        // If no times specified, update both atime and mtime to current time
        mask = TimesMask::ATIME | TimesMask::MTIME | TimesMask::ATIME_NSEC | TimesMask::MTIME_NSEC;
        (now_sec, now_nsec, now_sec, now_nsec)
    };

    // Create timestamp object and apply it
    let new_times = Timestamp::new(
        atime_sec,
        atime_nsec as _,
        mtime_sec,
        mtime_nsec as _,
        now_sec,
        now_nsec as _,
    );

    file.set_times(new_times, mask)?;

    Ok(0)
}

pub fn sys_rename(
    old_path: UserConstPtr<c_char>,
    new_path: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    sys_renameat2(AT_FDCWD, old_path, AT_FDCWD, new_path, 0)
}

pub fn sys_renameat(
    old_dirfd: c_int,
    old_path: UserConstPtr<c_char>,
    new_dirfd: c_int,
    new_path: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    sys_renameat2(old_dirfd, old_path, new_dirfd, new_path, 0)
}

pub fn sys_renameat2(
    old_dirfd: c_int,
    old_path: UserConstPtr<c_char>,
    new_dirfd: c_int,
    new_path: UserConstPtr<c_char>,
    flags: u32,
) -> LinuxResult<isize> {
    let old_path = old_path.get_as_str()?;
    let new_path = new_path.get_as_str()?;
    debug!(
        "sys_renameat2 <= old_dirfd: {}, old_path: {}, new_dirfd: {}, new_path: {}, flags: {}",
        old_dirfd, old_path, new_dirfd, new_path, flags
    );

    let old_path = handle_file_path(old_dirfd, old_path)?;
    let new_path = handle_file_path(new_dirfd, new_path)?;

    // fixme: flags
    axfs::api::rename(old_path.as_str(), new_path.as_str())?;

    Ok(0)
}

pub fn sys_symlink(
    old_path: UserConstPtr<c_char>,
    new_path: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    sys_symlinkat(old_path, AT_FDCWD, new_path)
}

pub fn sys_symlinkat(
    _old_path: UserConstPtr<c_char>,
    _new_dirfd: c_int,
    _new_path: UserConstPtr<c_char>,
) -> LinuxResult<isize> {
    warn!("Unimplemented syscall: SYS_SYMLINKAT");
    Ok(0)
}
