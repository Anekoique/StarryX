use alloc::boxed::Box;
use core::{any::Any, ffi::c_void};

use axerrno::{AxResult, LinuxError};
use axfs_ng_vfs::VfsResult;
use axio::{BufReader, Read};
use axsync::Mutex;

use xuspace::{UserPtr, UserSpaceAccess};
use xutils::ctypes::{
    ECHO, ICANON, TCGETS, TCSETS, TCSETSF, TCSETSW, TIOCGWINSZ, TIOCSWINSZ, termios, winsize,
};

use super::super::virt_fs::VirtDeviceOps;
use crate::task::with_uspace;

fn console_read_bytes(buf: &mut [u8]) -> AxResult<usize> {
    let len = axhal::console::read_bytes(buf);
    Ok(len)
}

fn console_write_bytes(buf: &[u8]) -> AxResult<usize> {
    axhal::console::write_bytes(buf);
    Ok(buf.len())
}

struct Stdin;

impl Read for Stdin {
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

struct StdinRaw;

impl Read for StdinRaw {
    // Raw character read with immediate return
    fn read(&mut self, buf: &mut [u8]) -> AxResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let len = console_read_bytes(buf[0..1].as_mut())?;
        Ok(len)
    }
}

enum TtyReader {
    Canonical(Box<BufReader<Stdin>>),
    Raw(StdinRaw),
}

/// Simple TTY device backed by the platform console with basic state.
pub struct Tty {
    reader: Mutex<TtyReader>,
    pub win_size: Mutex<winsize>,
    pub termios: Mutex<termios>,
}

impl Tty {
    pub fn new() -> Self {
        Self {
            reader: Mutex::new(TtyReader::Canonical(Box::new(BufReader::new(Stdin)))),
            win_size: Mutex::<winsize>::new(winsize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            }),
            termios: Mutex::<termios>::new(Self::default_termios()),
        }
    }

    fn default_termios() -> termios {
        use xutils::ctypes::*;
        termios {
            c_iflag: ICRNL | IXON | BRKINT | IMAXBEL,
            c_oflag: OPOST | ONLCR,
            c_cflag: B38400 | CS8 | CREAD | HUPCL,
            c_lflag: ISIG | ICANON | ECHO | ECHOE | ECHOK | ECHOCTL | ECHOKE | IEXTEN,
            c_line: 0,
            c_cc: {
                let mut cc = [0; 19];
                cc[VINTR as usize] = 3; // ^C
                cc[VQUIT as usize] = 28; // ^\
                cc[VERASE as usize] = 127; // DEL
                cc[VKILL as usize] = 21; // ^U
                cc[VEOF as usize] = 4; // ^D
                cc[VTIME as usize] = 0; // No timeout for canonical mode
                cc[VMIN as usize] = 1; // Read at least 1 char for non-canonical
                cc[VSTART as usize] = 17; // ^Q
                cc[VSTOP as usize] = 19; // ^S
                cc[VSUSP as usize] = 26; // ^Z
                cc[VEOL as usize] = 0;
                cc[VREPRINT as usize] = 18; // ^R
                cc[VDISCARD as usize] = 15; // ^O
                cc[VWERASE as usize] = 23; // ^W
                cc[VLNEXT as usize] = 22; // ^V
                cc[VEOL2 as usize] = 0;
                cc
            },
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
        let old_termios = *self.termios.lock();
        *self.termios.lock() = t;

        if (old_termios.c_lflag & ICANON) != (t.c_lflag & ICANON) {
            let mut reader = self.reader.lock();
            if t.c_lflag & ICANON != 0 {
                *reader = TtyReader::Canonical(Box::new(BufReader::new(Stdin)));
            } else {
                *reader = TtyReader::Raw(StdinRaw);
            }
        }
    }
}

impl VirtDeviceOps for Tty {
    fn read_at(&self, buf: &mut [u8], _offset: u64) -> VfsResult<usize> {
        let termios = self.get_termios();
        let is_canonical = termios.c_lflag & ICANON != 0;
        let should_echo = termios.c_lflag & ECHO != 0;

        let read_len = {
            let mut reader = self.reader.lock();
            match &mut *reader {
                TtyReader::Canonical(canonical_reader) => canonical_reader.read(buf)?,
                TtyReader::Raw(raw_reader) => raw_reader.read(buf)?,
            }
        };

        // Handle echoing for raw mode - ECHO IMMEDIATELY
        if read_len > 0 && should_echo && !is_canonical {
            // Echo the characters back immediately for raw mode
            console_write_bytes(&buf[..read_len])?;
        }

        if buf.is_empty() || read_len > 0 {
            return Ok(read_len);
        }

        // try again until we get something
        loop {
            let read_len = {
                let mut reader = self.reader.lock();
                match &mut *reader {
                    TtyReader::Canonical(canonical_reader) => canonical_reader.read(buf)?,
                    TtyReader::Raw(raw_reader) => raw_reader.read(buf)?,
                }
            };

            // Handle echoing for raw mode - ECHO IMMEDIATELY
            if read_len > 0 && should_echo && !is_canonical {
                console_write_bytes(&buf[..read_len])?;
            }

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

    fn ioctl(&self, op: usize, argp: UserPtr<c_void>) -> VfsResult<isize> {
        debug!("TTY ioctl: op={}, argp={:?}", op, argp);
        with_uspace(|uspace| {
            match op as u32 {
                TIOCGWINSZ => {
                    let ws = self.get_winsize();
                    uspace.write(argp.cast::<winsize>(), ws)?;
                }
                TIOCSWINSZ => {
                    let ws = uspace.read(argp.cast::<winsize>())?;
                    self.set_winsize(ws);
                }
                TCGETS => {
                    let t = self.get_termios();
                    debug!("TTY ioctl: TCGETS: {:?}", t);
                    uspace.write(argp.cast::<termios>(), t)?;
                }
                TCSETS | TCSETSW | TCSETSF => {
                    let t = uspace.read(argp.cast::<termios>())?;
                    debug!("TTY ioctl: TCSETS: {:?}", t);
                    self.set_termios(t);
                }
                _ => return Err(LinuxError::ENOTTY),
            }
            Ok(0)
        })
    }
}
