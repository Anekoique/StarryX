use core::ffi::c_int;

use alloc::{sync::Arc, vec::Vec};
use axerrno::{LinuxError, LinuxResult};
use axns::{ResArc, def_resource};
use axtask::{TaskExtRef, current};
use flatten_objects::FlattenObjects;
use linux_raw_sys::general::RLIMIT_NOFILE;
use spin::RwLock;

use super::{FileLike, stdio};

pub const AX_FILE_LIMIT: usize = 1024;

def_resource! {
    pub static FD_TABLE: ResArc<RwLock<FlattenObjects<Arc<dyn FileLike>, AX_FILE_LIMIT>>> = ResArc::new();
}

impl FD_TABLE {
    /// Return a copy of the inner table.
    pub fn copy_inner(&self) -> RwLock<FlattenObjects<Arc<dyn FileLike>, AX_FILE_LIMIT>> {
        let table = self.read();
        let mut new_table = FlattenObjects::new();
        for id in table.ids() {
            let _ = new_table.add_at(id, table.get(id).unwrap().clone());
        }
        RwLock::new(new_table)
    }

    pub fn clear(&self) {
        let mut table = self.write();
        let ids = table.ids().collect::<Vec<_>>();
        for id in ids {
            let _ = table.remove(id);
        }
    }
}

/// Get a file-like object by `fd`.
pub fn get_file_like(fd: c_int) -> LinuxResult<Arc<dyn FileLike>> {
    FD_TABLE
        .read()
        .get(fd as usize)
        .cloned()
        .ok_or(LinuxError::EBADF)
}

// /// Get a file-like object by `dirfd` and `path`.
// pub fn get_file_like_at(dirfd: c_int, path: &str) -> LinuxResult<Arc<dyn FileLike>> {
//     let dir = if path.starts_with('/') || dirfd == AT_FDCWD {
//         None
//     } else {
//         Some(Directory::from_fd(dirfd)?)
//     };
//
//     let mut opt = OpenOptions::new();
//     opt.read(true);
//     opt.write(true);
//     match dir.as_ref().map_or_else(
//         || axfs::fops::File::open(path, &opt),
//         |dir| dir.inner().open_file_at(path, &opt),
//     ) {
//         Err(AxError::IsADirectory) => {}
//         r => return Ok(Arc::new(File::new(r?, path.into()))),
//     }
//
//     Ok(Arc::new(Directory::new(
//         dir.map_or_else(
//             || axfs::fops::Directory::open_dir(path, &opt),
//             |dir| dir.inner().open_dir_at(path, &opt),
//         )?,
//         path.into(),
//     )))
// }

/// Add a file to the file descriptor table.
pub fn add_file_like(f: Arc<dyn FileLike>) -> LinuxResult<c_int> {
    let curr = current();
    // Check RLIMIT_NOFILE resource limit
    let rlimits = curr.task_ext().process_data().rlimits.read();
    let fd_limit = rlimits[RLIMIT_NOFILE].current as usize;

    // Check if we already have too many open files
    let fd_count = FD_TABLE.read().count();
    if fd_count >= fd_limit {
        return Err(LinuxError::EMFILE);
    }

    Ok(FD_TABLE.write().add(f).map_err(|_| LinuxError::EMFILE)? as c_int)
}

/// Close a file by `fd`.
pub fn close_file_like(fd: c_int) -> LinuxResult {
    let f = FD_TABLE
        .write()
        .remove(fd as usize)
        .ok_or(LinuxError::EBADF)?;
    debug!("close_file_like <= count: {}", Arc::strong_count(&f));
    Ok(())
}

#[ctor_bare::register_ctor]
fn init_stdio() {
    let mut fd_table = flatten_objects::FlattenObjects::new();
    fd_table
        .add_at(0, Arc::new(stdio::stdin()) as _)
        .unwrap_or_else(|_| panic!()); // stdin
    fd_table
        .add_at(1, Arc::new(stdio::stdout()) as _)
        .unwrap_or_else(|_| panic!()); // stdout
    fd_table
        .add_at(2, Arc::new(stdio::stdout()) as _)
        .unwrap_or_else(|_| panic!()); // stderr
    FD_TABLE.init_new(spin::RwLock::new(fd_table));
}
