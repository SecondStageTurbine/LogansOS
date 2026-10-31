// SPDX-License-Identifier: MPL-2.0
//! OT-61 C0.5 — the `neoplutarch` arm of `std::sys::stdio`.
//!
//! Overlaid into `library/std/src/sys/stdio/neoplutarch.rs` by
//! `userland/neoplutarch-std-overlay/apply-overlay.sh`. Routes `std`'s
//! `Stdout` AND `Stderr` to `SYS_WRITE` on the console descriptor (fd 1; there
//! is no separate stderr fd on LogansOS, see `neoplutarch_rt::CONSOLE_FD`)
//! through `crate::sys::neoplutarch_rt`, so real `std::println!` /
//! `eprintln!` print on-target. Replaces the `_` arm
//! (`unsupported`), which silently drops every write.
//!
//! No silent-wrong behaviour:
//! - a failed write is a real `io::Error` carrying the LogansOS errno (decoded
//!   by `sys/io/error/neoplutarch.rs`);
//! - `is_ebadf` is true ONLY for `EBADF` (std turns an EBADF on a standard
//!   stream into success, so a catch-all `true` would swallow every error);
//! - `Stdin` has no ambient input in C0.5, so `read` is `Err(Unsupported)`,
//!   not a fake EOF;
//! - `panic_output` is stderr, so panic messages reach the serial log before
//!   `abort_internal` exits 134.

use crate::io;
use crate::sys::neoplutarch_rt;

const STDOUT_FD: u64 = neoplutarch_rt::CONSOLE_FD;
const STDERR_FD: u64 = neoplutarch_rt::CONSOLE_FD;
/// LogansOS `EBADF` (`kernel/src/errno.rs`: -9).
const EBADF: i32 = 9;

fn write_fd(fd: u64, buf: &[u8]) -> io::Result<usize> {
    let r = neoplutarch_rt::write(fd, buf);
    if r < 0 { Err(io::Error::from_raw_os_error((-r) as i32)) } else { Ok(r as usize) }
}

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

impl Stdin {
    pub const fn new() -> Stdin {
        Stdin
    }
}

impl io::Read for Stdin {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        // No ambient stdin in C0.5 (input is a capability, a later rung).
        Err(io::const_error!(io::ErrorKind::Unsupported, "neoplutarch: no stdin in C0.5"))
    }
}

impl Stdout {
    pub const fn new() -> Stdout {
        Stdout
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write_fd(STDOUT_FD, buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Stderr {
    pub const fn new() -> Stderr {
        Stderr
    }
}

impl io::Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write_fd(STDERR_FD, buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub const STDIN_BUF_SIZE: usize = 0;

pub fn is_ebadf(err: &io::Error) -> bool {
    err.raw_os_error() == Some(EBADF)
}

pub fn panic_output() -> Option<impl io::Write> {
    Some(Stderr::new())
}
