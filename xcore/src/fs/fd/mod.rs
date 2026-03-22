mod dummy;
mod epoll;
mod event;
mod fanotify;
mod file;
mod pid;
mod pipe;
mod timer;

pub use self::dummy::*;
pub use self::epoll::*;
pub use self::event::*;
pub use self::fanotify::*;
pub use self::file::*;
pub use self::pid::*;
pub use self::pipe::*;
pub use self::timer::*;

use alloc::{sync::Arc, vec::Vec};
use core::ffi::c_int;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::{FS_CONTEXT, FileFlags};
use axns::{ResArc, def_resource};
use axsync::Mutex;
use spin::RwLock;

use super::file::{FileLike, XFile};
use crate::task::with_xprocess;
use xutils::{
    collections::{bitmaps::Bitmap, flatten_objects::FlattenObjects},
    ctypes::RLIMIT_NOFILE,
};

pub const AX_FILE_LIMIT: usize = 1024;

def_resource! {
    pub static FD_TABLE: ResArc<FdTable> = ResArc::new();
}

/// File descriptor table that manages both file-like objects and their flags.
///
/// This structure provides a complete file descriptor management system including:
/// - Storage of file-like objects (files, directories, sockets, etc.)
/// - File descriptor flags (currently supports CLOEXEC flag)
/// - Thread-safe operations with internal mutability
/// - Support for file descriptor copying during process forking
///
/// The flags bitmap tracks per-fd flags like FD_CLOEXEC which determines
/// whether the file descriptor should be closed on exec().
pub struct FdTable {
    inner: RwLock<FlattenObjects<Arc<XFile>, AX_FILE_LIMIT>>,
    flags: RwLock<Bitmap<AX_FILE_LIMIT>>,
}

impl FdTable {
    /// Create a new FdTable instance
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(FlattenObjects::new()),
            flags: RwLock::new(Bitmap::new()),
        }
    }

    /// Returns the maximum number of file descriptors that can be held
    pub fn capacity(&self) -> usize {
        self.inner.read().capacity()
    }

    /// Returns the number of file descriptors that have been added
    pub fn count(&self) -> usize {
        self.inner.read().count()
    }

    /// Checks if the given fd is assigned
    pub fn is_assigned(&self, fd: usize) -> bool {
        self.inner.read().is_assigned(fd)
    }

    /// Get a file-like object by fd
    pub fn get(&self, fd: usize) -> Option<Arc<XFile>> {
        self.inner.read().get(fd).cloned()
    }

    /// Add a file-like object and return its fd
    pub fn add(&self, file: Arc<XFile>) -> Result<usize, LinuxError> {
        self.inner.write().add(file).map_err(|_| LinuxError::EMFILE)
    }

    /// Add a file-like object at a specific fd
    pub fn add_at(&self, fd: usize, file: Arc<XFile>) -> Result<usize, Arc<XFile>> {
        self.inner.write().add_at(fd, file)
    }

    /// Remove a file-like object by fd
    pub fn remove(&self, fd: usize) -> Option<Arc<XFile>> {
        self.inner.write().remove(fd)
    }

    /// Get all valid file descriptor IDs
    pub fn ids(&self) -> Vec<usize> {
        self.inner.read().ids().collect()
    }

    /// Return a copy of the entire FdTable.
    pub fn copy_inner(&self) -> FdTable {
        let table = self.inner.read();
        let mut new_table = FlattenObjects::new();
        let flags = self.flags.read();
        let mut new_flags = Bitmap::new();
        for id in table.ids() {
            let _ = new_table.add_at(id, table.get(id).unwrap().clone());
            new_flags.set(id, flags.get(id));
        }

        FdTable {
            inner: RwLock::new(new_table),
            flags: RwLock::new(new_flags),
        }
    }

    /// Clear all file descriptors
    pub fn clear(&self) {
        let mut table = self.inner.write();
        let ids = table.ids().collect::<Vec<_>>();
        for id in ids {
            let _ = table.remove(id);
        }
    }

    /// Add a file-like object with flags
    pub fn add_with_flags(&self, file: Arc<XFile>, cloexec: bool) -> Result<usize, LinuxError> {
        let fd = self.add(file)?;
        self.flags.write().set(fd, cloexec);
        debug!("files: {:?}", self.ids());
        Ok(fd)
    }

    /// Add a file-like object at a specific fd with flags
    pub fn add_at_with_flags(
        &self,
        fd: usize,
        file: Arc<XFile>,
        cloexec: bool,
    ) -> Result<(), LinuxError> {
        let result = self
            .inner
            .write()
            .add_at(fd, file)
            .map(|_| ())
            .map_err(|_| LinuxError::EBADF);
        if result.is_ok() {
            self.flags.write().set(fd, cloexec);
        }
        result
    }

    /// Add a file-like object with flags
    pub fn add_file_like(
        &self,
        fd: usize,
        file: Arc<dyn FileLike>,
        flags: FileFlags,
        cloexec: bool,
    ) -> Result<(), LinuxError> {
        let file = Arc::new(XFile::new(file, flags));
        self.add_at_with_flags(fd, file, cloexec)
    }

    /// Check if file descriptor has CLOEXEC flag
    pub fn has_cloexec(&self, fd: usize) -> bool {
        if self.is_assigned(fd) {
            self.flags.read().get(fd)
        } else {
            false
        }
    }

    /// Set CLOEXEC flag for a file descriptor
    pub fn set_cloexec(&self, fd: usize, cloexec: bool) {
        self.flags.write().set(fd, cloexec);
    }

    /// Close all file descriptors that have the CLOEXEC flag set
    /// This is called during exec() to close files marked for close-on-exec
    pub fn close_on_exec(&self) {
        let flags = self.flags.read();
        let ids_to_close: Vec<usize> = self
            .inner
            .read()
            .ids()
            .filter(|&fd| flags.get(fd))
            .collect();
        drop(flags);

        let mut inner = self.inner.write();
        let mut flags = self.flags.write();
        for fd in ids_to_close {
            inner.remove(fd);
            flags.set(fd, false);
        }
    }
}

/// Get a file-like object by `fd`.
pub fn get_file_like(fd: c_int) -> LinuxResult<Arc<XFile>> {
    FD_TABLE.get(fd as usize).ok_or(LinuxError::EBADF)
}

/// Add a file to the file descriptor table.
pub fn add_file_like(f: Arc<dyn FileLike>, flags: FileFlags, cloexec: bool) -> LinuxResult<c_int> {
    // Check RLIMIT_NOFILE resource limit
    let fd_limit =
        with_xprocess(|xprocess| xprocess.rlimits.read()[RLIMIT_NOFILE].current as usize);

    // Check if we already have too many open files
    let fd_count = FD_TABLE.count();
    if fd_count >= fd_limit {
        return Err(LinuxError::EMFILE);
    }

    Ok(FD_TABLE.add_with_flags(Arc::new(XFile::new(f, flags)), cloexec)? as c_int)
}

/// Close a file by `fd`.
pub fn close_file_like(fd: c_int) -> LinuxResult {
    debug!("files: {:?}", FD_TABLE.ids());
    let f = FD_TABLE.remove(fd as usize).ok_or(LinuxError::EBADF)?;

    // Clear the file descriptor flags
    FD_TABLE.set_cloexec(fd as usize, false);

    drop(f);
    Ok(())
}

pub fn init_stdio() -> LinuxResult<()> {
    let fd_table = FdTable::new();
    let file = Arc::new(Mutex::new(
        FS_CONTEXT.lock().write_file("/dev/tty").unwrap(),
    ));
    fd_table.add_file_like(
        0,
        Arc::new(File::from_shared(file.clone())),
        FileFlags::READ,
        false,
    )?;
    fd_table.add_file_like(
        1,
        Arc::new(File::from_shared(file.clone())),
        FileFlags::WRITE,
        false,
    )?;
    fd_table.add_file_like(
        2,
        Arc::new(File::from_shared(file.clone())),
        FileFlags::READ | FileFlags::WRITE,
        false,
    )?;
    FD_TABLE.init_new(fd_table);
    Ok(())
}
