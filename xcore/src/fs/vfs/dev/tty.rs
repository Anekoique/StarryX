use core::any::Any;

use axio::{BufReader, Read};
use axerrno::AxResult;
use axfs_ng_vfs::VfsResult;
use axsync::Mutex;
use linux_raw_sys::general::{termios, winsize};

use super::super::virt_fs::VirtDeviceOps;

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

struct Stdin;

impl Read for Stdin{
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
    
/// Simple TTY device backed by the platform console with basic state.
pub struct Tty {
    reader: Mutex<BufReader<Stdin>>,
    pub win_size: Mutex<winsize>,
    pub termios: Mutex<termios>,
}

impl Tty {
    pub fn new() -> Self {
        Self {
            reader: Mutex::new(BufReader::new(Stdin)),
            win_size: Mutex::<winsize>::new(winsize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            }),
            termios: Mutex::<termios>::new(unsafe { core::mem::zeroed() }),
        }
    }

    pub fn get_winsize(&self) -> winsize {
        *self.win_size.lock()
    }

    pub fn set_winsize(&self, ws: winsize) {
        *self.win_size.lock() = ws;
    }

    pub fn get_termios(&self) -> termios {
        *self.termios.lock()
    }

    pub fn set_termios(&self, t: termios) {
        *self.termios.lock() = t;
    }
}

impl VirtDeviceOps for Tty {
    fn read_at(&self, buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        let read_len = self.reader.lock().read(buf)?;
        if buf.is_empty() || read_len > 0 {
            return Ok(read_len);
        }
        // try again until we get something
        loop {
            let read_len = self.reader.lock().read(buf)?;
            if read_len > 0 {
                return Ok(read_len);
            }
            axtask::yield_now();
        }
    }

    fn write_at(&self, buf: &[u8], _offset: u64) -> VfsResult<usize> {
        Ok(console_write_bytes(buf)?)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
