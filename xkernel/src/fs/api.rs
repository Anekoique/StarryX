use alloc::sync::Arc;
use core::ffi::c_int;

use xerrno::{LinuxError, LinuxResult};
use xfs::{FS_CONTEXT, FileFlags, FsContext};
use xsync::RawMutex;
use xvfs::Location;

use xutils::ctypes::{AT_EMPTY_PATH, AT_FDCWD, AT_SYMLINK_NOFOLLOW};

use super::{
    fd::{Directory, File, get_file_like},
    file::FileLike,
};

pub fn with_fs<R>(
    dirfd: c_int,
    path: &str,
    f: impl FnOnce(&mut FsContext<RawMutex>) -> LinuxResult<R>,
) -> LinuxResult<R> {
    let mut fs = FS_CONTEXT.lock();
    if dirfd == AT_FDCWD || path.starts_with('/') {
        f(&mut fs)
    } else {
        let dir = Directory::from_fd(dirfd, FileFlags::empty(), FileFlags::empty())?
            .inner()
            .clone();
        f(&mut fs.with_current_dir(dir)?)
    }
}

/// Removes one non-directory name, completing the cache side of the unlink.
///
/// The cache is consulted only after the directory entry is gone, and only a
/// final link (`nlink == 0`) discards unowned pages, so a failed unlink or a
/// surviving hard link never loses dirty data.
pub fn remove_file(fs: &mut FsContext<RawMutex>, path: &str) -> LinuxResult<()> {
    let entry = fs.resolve_no_follow(path)?;
    let node = entry.is_file().then(|| entry.get_file_node());
    fs.remove_file(path)?;
    if let Some(node) = node {
        super::cache::complete_unlink(node.as_ref());
    }
    Ok(())
}

pub fn with_file<R>(
    dirfd: c_int,
    required: FileFlags,
    forbidden: FileFlags,
    f: impl FnOnce(&mut Arc<File>) -> LinuxResult<R>,
) -> LinuxResult<R> {
    f(&mut File::from_fd(dirfd, required, forbidden)?)
}

pub fn with_location<R>(
    dirfd: c_int,
    path: Option<&str>,
    flags: u32,
    f: impl FnOnce(&mut Location<RawMutex>) -> LinuxResult<R>,
) -> LinuxResult<R> {
    match path {
        Some("") | None => {
            if flags & AT_EMPTY_PATH == 0 {
                return Err(LinuxError::ENOENT);
            }
            let file_like = get_file_like(dirfd)?;
            let mut location = file_like.get_location().ok_or(LinuxError::EBADF)?;
            f(&mut location)
        }
        Some(path) => with_fs(dirfd, path, |fs| {
            let mut location = if flags & AT_SYMLINK_NOFOLLOW != 0 {
                fs.resolve_no_follow(path)?
            } else {
                fs.resolve(path)?
            };
            f(&mut location)
        }),
    }
}
