use alloc::sync::Arc;
use core::any::Any;

use axerrno::{AxResult, LinuxError, LinuxResult};
use axfs_ng::FileFlags;
use axio::{BufReader, PollState, prelude::*};
use axsync::Mutex;

use xcore::fs::{FD_TABLE, FdTable, FileLike, Kstat, XFile};

use crate::{ctypes::S_IFCHR, task::check_fatal_signals};

fn console_read_bytes(buf: &mut [u8]) -> AxResult<usize> {
    let len = axhal::console::read_bytes(buf);
    for c in &mut buf[..len] {
        if *c == b'\r' {
            *c = b'\n';
        }
    }
    Ok(len)
}

fn console_write_bytes(buf: &[u8]) -> AxResult<usize> {
    axhal::console::write_bytes(buf);
    Ok(buf.len())
}

struct StdinRaw;
struct StdoutRaw;

impl Read for StdinRaw {
    // Non-blocking read, returns number of bytes read.
    fn read(&mut self, buf: &mut [u8]) -> AxResult<usize> {
        let mut read_len = 0;
        while read_len < buf.len() {
            let len = console_read_bytes(buf[read_len..].as_mut())?;
            if len == 0 {
                break;
            }
            read_len += len;
        }
        Ok(read_len)
    }
}

impl Write for StdoutRaw {
    fn write(&mut self, buf: &[u8]) -> AxResult<usize> {
        console_write_bytes(buf)
    }

    fn flush(&mut self) -> AxResult {
        Ok(())
    }
}

pub struct Stdin {
    inner: &'static Mutex<BufReader<StdinRaw>>,
}

impl Stdin {
    // Block until at least one byte is read.
    fn read_blocked(&self, buf: &mut [u8]) -> AxResult<usize> {
        let read_len = self.inner.lock().read(buf)?;
        if buf.is_empty() || read_len > 0 {
            return Ok(read_len);
        }
        // try again until we get something
        loop {
            let read_len = self.inner.lock().read(buf)?;
            if read_len > 0 {
                return Ok(read_len);
            }
            check_fatal_signals();
            axtask::yield_now();
        }
    }
}

impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> AxResult<usize> {
        self.read_blocked(buf)
    }
}

pub struct Stdout {
    inner: &'static Mutex<StdoutRaw>,
}

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> AxResult<usize> {
        self.inner.lock().write(buf)
    }

    fn flush(&mut self) -> AxResult {
        self.inner.lock().flush()
    }
}

/// Constructs a new handle to the standard input of the current process.
pub fn stdin() -> Stdin {
    static INSTANCE: Mutex<BufReader<StdinRaw>> = Mutex::new(BufReader::new(StdinRaw));
    Stdin { inner: &INSTANCE }
}

/// Constructs a new handle to the standard output of the current process.
pub fn stdout() -> Stdout {
    static INSTANCE: Mutex<StdoutRaw> = Mutex::new(StdoutRaw);
    Stdout { inner: &INSTANCE }
}

impl FileLike for Stdin {
    fn read(&self, buf: &mut [u8]) -> LinuxResult<usize> {
        Ok(self.read_blocked(buf)?)
    }

    fn write(&self, _buf: &[u8]) -> LinuxResult<usize> {
        Err(LinuxError::EPERM)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        Ok(Kstat {
            mode: S_IFCHR | 0o444u32, // r--r--r--
            ..Default::default()
        })
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        Ok(PollState {
            readable: true,
            writable: true,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) {}
}

impl FileLike for Stdout {
    fn read(&self, _buf: &mut [u8]) -> LinuxResult<usize> {
        Err(LinuxError::EPERM)
    }

    fn write(&self, buf: &[u8]) -> LinuxResult<usize> {
        Ok(self.inner.lock().write(buf)?)
    }

    fn stat(&self) -> LinuxResult<Kstat> {
        Ok(Kstat {
            mode: S_IFCHR | 0o220u32, // -w--w----
            ..Default::default()
        })
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }

    fn poll(&self) -> LinuxResult<PollState> {
        Ok(PollState {
            readable: true,
            writable: true,
        })
    }

    fn set_nonblocking(&self, _nonblocking: bool) {}
}

#[ctor_bare::register_ctor]
fn init_stdio() {
    let fd_table = FdTable::new();
    fd_table
        .add_at(0, Arc::new(XFile::new(Arc::new(stdin()), FileFlags::READ)))
        .unwrap_or_else(|_| panic!()); // stdin
    fd_table
        .add_at(
            1,
            Arc::new(XFile::new(Arc::new(stdout()), FileFlags::WRITE)),
        )
        .unwrap_or_else(|_| panic!()); // stdout
    fd_table
        .add_at(
            2,
            Arc::new(XFile::new(
                Arc::new(stdout()),
                FileFlags::READ | FileFlags::WRITE,
            )),
        )
        .unwrap_or_else(|_| panic!()); // stderr
    FD_TABLE.init_new(fd_table);
}
