use core::fmt;

use alloc::sync::Arc;
use axfs_ng_vfs::{FileNode, FileNodeOps, Location, Metadata, VfsError, VfsResult};
use axio::SeekFrom;
use lock_api::RawMutex;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct FileFlags: u32 {
        const READ = 1;
        const WRITE = 2;
        const EXEC = 4;
        const APPEND = 8;
        const DIRECT = 16;
        const PATH = 32;
    }
}

/// Results returned by [`OpenOptions::open`].
pub enum OpenResult<M> {
    File(FsFile<M>),
    Dir(Location<M>),
}
impl<M> OpenResult<M> {
    pub fn into_file(self) -> VfsResult<FsFile<M>> {
        match self {
            Self::File(file) => Ok(file),
            Self::Dir(_) => Err(VfsError::EISDIR),
        }
    }

    pub fn into_dir(self) -> VfsResult<Location<M>> {
        match self {
            Self::Dir(dir) => Ok(dir),
            Self::File(_) => Err(VfsError::ENOTDIR),
        }
    }

    pub fn into_location(self) -> Location<M> {
        match self {
            Self::File(file) => file.inner,
            Self::Dir(dir) => dir,
        }
    }
}

/// Options and flags which can be used to configure how a file is opened.
#[derive(Clone)]
pub struct OpenOptions {
    // generic
    read: bool,
    write: bool,
    execute: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
    directory: bool,
    direct: bool,
    path: bool,
    user: Option<(u32, u32)>,
    // system-specific
    custom_flags: i32,
    mode: u32,
}
impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    pub fn new() -> Self {
        Self {
            // generic
            read: false,
            write: false,
            execute: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
            directory: false,
            direct: false,
            path: false,
            user: None,
            // system-specific
            custom_flags: 0,
            mode: 0o666,
        }
    }

    /// Sets the option for read access.
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    /// Sets the option for write access.
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    /// Sets the option for execute access.
    pub fn execute(&mut self, execute: bool) -> &mut Self {
        self.execute = execute;
        self
    }

    /// Sets the option for the append mode.
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.append = append;
        self
    }

    /// Sets the option for truncating a previous file.
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.truncate = truncate;
        self
    }

    /// Sets the option to create a new file, or open it if it already exists.
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }

    /// Sets the option to create a new file, failing if it already exists.
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.create_new = create_new;
        self
    }

    /// Sets the option to open directory instead.
    pub fn directory(&mut self, directory: bool) -> &mut Self {
        self.directory = directory;
        self
    }

    /// Sets the option to use direct I/O.
    pub fn direct(&mut self, direct: bool) -> &mut Self {
        self.direct = direct;
        self
    }

    /// Sets the option to use path.
    pub fn path(&mut self, path: bool) -> &mut Self {
        self.path = path;
        self
    }

    /// Sets the user and group id to open the file with.
    pub fn user(&mut self, uid: u32, gid: u32) -> &mut Self {
        self.user = Some((uid, gid));
        self
    }

    /// Pass custom flags to the flags argument of open.
    pub fn custom_flags(&mut self, flags: i32) -> &mut Self {
        self.custom_flags = flags;
        self
    }

    /// Sets the mode bits that a new file will be created with.
    pub fn mode(&mut self, mode: u32) -> &mut Self {
        self.mode = mode;
        self
    }

    // Getter methods for internal use

    /// Gets the read option.
    pub fn get_read(&self) -> bool {
        self.read
    }

    /// Gets the write option.
    pub fn get_write(&self) -> bool {
        self.write
    }

    /// Gets the execute option.
    pub fn get_execute(&self) -> bool {
        self.execute
    }

    /// Gets the append option.
    pub fn get_append(&self) -> bool {
        self.append
    }

    /// Gets the truncate option.
    pub fn get_truncate(&self) -> bool {
        self.truncate
    }

    /// Gets the create option.
    pub fn get_create(&self) -> bool {
        self.create
    }

    /// Gets the create_new option.
    pub fn get_create_new(&self) -> bool {
        self.create_new
    }

    /// Gets the directory option.
    pub fn get_directory(&self) -> bool {
        self.directory
    }

    /// Gets the user option.
    pub fn get_user(&self) -> Option<(u32, u32)> {
        self.user
    }

    /// Gets the custom flags.
    pub fn get_custom_flags(&self) -> i32 {
        self.custom_flags
    }

    /// Gets the mode.
    pub fn get_mode(&self) -> u32 {
        self.mode
    }

    pub fn to_flags(&self) -> VfsResult<FileFlags> {
        if self.append && !self.write {
            return Err(VfsError::EINVAL);
        }

        let mut flags = FileFlags::empty();

        if self.read {
            flags |= FileFlags::READ;
        }
        if self.write {
            flags |= FileFlags::WRITE;
        }
        if self.execute {
            flags |= FileFlags::EXEC;
        }
        if self.append {
            flags |= FileFlags::APPEND;
        }
        if self.direct {
            flags |= FileFlags::DIRECT;
        }
        if self.path {
            flags |= FileFlags::PATH;
        }

        if !(flags.intersects(FileFlags::READ | FileFlags::WRITE)) {
            return Err(VfsError::EINVAL);
        }

        Ok(flags)
    }

    pub(crate) fn is_valid(&self) -> bool {
        if !self.read && !self.write && !self.append {
            return true;
        }
        match (self.write, self.append) {
            (true, false) => {}
            (false, false) => {
                if self.truncate || self.create || self.create_new {
                    return false;
                }
            }
            (_, true) => {
                if self.truncate && !self.create_new {
                    return false;
                }
            }
        }
        true
    }
}
impl fmt::Debug for OpenOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let OpenOptions {
            read,
            write,
            execute,
            append,
            truncate,
            create,
            create_new,
            directory,
            direct,
            path,
            user,
            custom_flags,
            mode,
        } = self;
        f.debug_struct("OpenOptions")
            .field("read", read)
            .field("write", write)
            .field("execute", execute)
            .field("append", append)
            .field("truncate", truncate)
            .field("create", create)
            .field("create_new", create_new)
            .field("directory", directory)
            .field("direct", direct)
            .field("path", path)
            .field("user", user)
            .field("custom_flags", custom_flags)
            .field("mode", mode)
            .finish()
    }
}

/// Provides `std::fs::File`-like interface.
pub struct FsFile<M> {
    inner: Location<M>,
    pub(crate) flags: FileFlags,

    pub position: u64,
}
impl<M: RawMutex> FsFile<M> {
    pub(crate) fn new(inner: Location<M>, flags: FileFlags) -> Self {
        Self {
            inner,
            flags,
            position: 0,
        }
    }

    pub fn access(&self, cap: FileFlags) -> VfsResult<&FileNode<M>> {
        if self.flags.contains(cap) {
            self.inner.entry().as_file()
        } else {
            Err(VfsError::EBADF)
        }
    }

    pub fn inner(&self) -> &Location<M> {
        &self.inner
    }

    /// Attempts to sync OS-internal file content and metadata to disk.
    ///
    /// If `data_only` is `true`, only the file data is synced, not the metadata.
    pub fn sync(&self, data_only: bool) -> VfsResult<()> {
        self.access(FileFlags::WRITE)?.sync(data_only)
    }

    /// Get the file size.
    pub fn len(&self) -> VfsResult<u64> {
        self.access(FileFlags::READ)?.len()
    }

    /// Returns whether the file is empty.
    pub fn is_empty(&self) -> VfsResult<bool> {
        Ok(self.len()? == 0)
    }

    /// Get the file inode number.
    pub fn inode(&self) -> VfsResult<u64> {
        Ok(self.access(FileFlags::empty())?.inode())
    }

    pub fn get_file_node(&self) -> Arc<dyn FileNodeOps<M>> {
        self.inner.get_file_node()
    }

    /// Truncates or extends the underlying file, updating the size of this file to become `size`.
    pub fn set_len(&self, size: u64) -> VfsResult<()> {
        self.access(FileFlags::WRITE)?.set_len(size)
    }

    /// Queries metadata about the underlying file.
    pub fn metadata(&self) -> VfsResult<Metadata> {
        self.access(FileFlags::READ)?;
        self.inner.metadata()
    }

    /// Reads a number of bytes starting from a given offset.
    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        self.access(FileFlags::READ)?.read_at(buf, offset)
    }

    /// Writes a number of bytes starting from a given offset.
    pub fn write_at(&mut self, buf: &[u8], offset: u64) -> VfsResult<usize> {
        self.access(FileFlags::WRITE)?.write_at(buf, offset)
    }

    /// Get the current position of the file.
    pub fn set_position(&mut self, position: u64) {
        self.position = position;
    }

    /// Get the file flags.
    pub fn get_flags(&self) -> FileFlags {
        self.flags
    }
}

impl<M: RawMutex> FsFile<M> {
    /// Writes a number of bytes starting from the current position.
    pub fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        if self.flags.contains(FileFlags::APPEND) {
            let (written, offset) = self.access(FileFlags::WRITE)?.append(buf)?;
            self.position = offset;
            Ok(written)
        } else {
            let n = self.write_at(buf, self.position)?;
            self.position += n as u64;
            Ok(n)
        }
    }
}

fn vfs_error_to_axio(err: VfsError) -> axio::Error {
    match err {
        VfsError::EEXIST => axio::Error::AlreadyExists,
        VfsError::ENOTEMPTY => axio::Error::DirectoryNotEmpty,
        VfsError::EINVAL => axio::Error::InvalidInput,
        VfsError::EISDIR => axio::Error::IsADirectory,
        VfsError::ENOMEM => axio::Error::NoMemory,
        VfsError::ENOTDIR => axio::Error::NotADirectory,
        VfsError::ENOENT => axio::Error::NotFound,
        VfsError::EBADF => axio::Error::PermissionDenied,
        VfsError::ENOSPC => axio::Error::StorageFull,
        VfsError::ENOSYS | VfsError::EOPNOTSUPP => axio::Error::Unsupported,
        _ => axio::Error::Io,
    }
}

impl<M: RawMutex> axio::Read for FsFile<M> {
    fn read(&mut self, buf: &mut [u8]) -> axio::Result<usize> {
        self.read_at(buf, self.position)
            .inspect(|n| {
                self.position += *n as u64;
            })
            .map_err(vfs_error_to_axio)
    }
}
impl<M: RawMutex> axio::Write for FsFile<M> {
    fn write(&mut self, buf: &[u8]) -> axio::Result<usize> {
        if self.flags.contains(FileFlags::APPEND) {
            self.access(FileFlags::WRITE)
                .map_err(vfs_error_to_axio)?
                .append(buf)
                .map(|(written, offset)| {
                    self.position = offset;
                    written
                })
        } else {
            self.write_at(buf, self.position).inspect(|n| {
                self.position += *n as u64;
            })
        }
        .map_err(vfs_error_to_axio)
    }

    fn flush(&mut self) -> axio::Result {
        Ok(())
    }
}
impl<M: RawMutex> axio::Seek for FsFile<M> {
    fn seek(&mut self, pos: SeekFrom) -> axio::Result<u64> {
        let new_pos = (|| {
            Ok(match pos {
                SeekFrom::Start(pos) => pos,
                SeekFrom::End(off) => {
                    let size = self.access(FileFlags::empty())?.len()?;
                    size.checked_add_signed(off).ok_or(VfsError::EINVAL)?
                }
                SeekFrom::Current(off) => self
                    .position
                    .checked_add_signed(off)
                    .ok_or(VfsError::EINVAL)?,
            })
        })()
        .map_err(vfs_error_to_axio)?;
        self.position = new_pos;
        Ok(new_pos)
    }
}
