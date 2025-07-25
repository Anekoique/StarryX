mod api;
mod file;
mod iomux;
mod pipe;
mod stdio;

pub use self::api::*;
pub use self::file::*;
pub use self::iomux::*;
pub use self::pipe::*;
pub use self::stdio::*;

use axerrno::{LinuxError, LinuxResult};
use axfs_ng::FileFlags;

use xcore::fs::get_file_like;

use crate::net::Socket;

pub struct FileOps;

impl FileOps {
    pub fn accept(fd: i32) -> LinuxResult<Socket> {
        get_file_like(fd)?
            .validate(FileFlags::empty(), FileFlags::PATH)?
            .clone()
            .into_any()
            .downcast::<Socket>()
            .map_err(|_| LinuxError::ENOTSOCK)?
            .accept()
    }
}
