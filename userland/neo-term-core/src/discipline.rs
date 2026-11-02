// SPDX-License-Identifier: MPL-2.0
//! The line discipline (THINK L1a §4): the terminal's, in userland.
//!
//! - **Cooked** (the default every child starts in): printable bytes go into
//!   a line buffer ([`LINE_MAX`] bytes; past that, a bell and the byte is
//!   dropped) and are echoed; Backspace removes the last UTF-8 scalar;
//!   Ctrl+U clears the line; Enter appends LF and delivers the line; Ctrl+D
//!   on an empty line closes stdin (EOF), on a non-empty line delivers it
//!   without LF; Ctrl+C discards the line and delivers `03`. Multi-byte key
//!   sequences (arrows and the like) are not line editing in L1a: dropped.
//! - **Raw**: each key's bytes go straight to the child.
//! - **Echo** off (cooked, for a password prompt): the same, without echo.
//!
//! The discipline only decides bytes; writing them is the terminal's, with
//! its never-block rule (§3.3).

use crate::page::Mode;
use alloc::vec::Vec;

pub const LINE_MAX: usize = 4096;

/// What one key (or a paste) produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    /// Bytes for the child's stdin.
    pub to_child: Vec<u8>,
    /// Bytes for the terminal's own parser (the echo).
    pub echo: Vec<u8>,
    /// Ring the (visual) bell.
    pub bell: bool,
    /// Close the stdin pipe's write end (EOF).
    pub close: bool,
}

const ERASE_ONE: &[u8] = b"\x08 \x08";

#[derive(Clone, Debug)]
pub struct Discipline {
    mode: Mode,
    line: Vec<u8>,
    closed: bool,
}

impl Default for Discipline {
    fn default() -> Self {
        Self::new()
    }
}

impl Discipline {
    /// A new child's discipline: cooked with echo, an empty line.
    pub fn new() -> Self {
        Self {
            mode: Mode::COOKED_ECHO,
            line: Vec::new(),
            closed: false,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The line typed so far (cooked).
    pub fn pending(&self) -> &[u8] {
        &self.line
    }

    /// Switch mode (a child's request, §3.2). Going raw delivers whatever
    /// partial line was typed, so no typed byte is lost.
    pub fn set_mode(&mut self, mode: Mode) -> Effect {
        let mut fx = Effect::default();
        if self.mode.cooked && !mode.cooked && !self.line.is_empty() && !self.closed {
            fx.to_child = core::mem::take(&mut self.line);
        }
        self.mode = mode;
        fx
    }

    /// One key press, as the encoder's bytes.
    pub fn key(&mut self, bytes: &[u8]) -> Effect {
        let mut fx = Effect::default();
        if self.closed || bytes.is_empty() {
            return fx;
        }
        if !self.mode.cooked {
            fx.to_child.extend_from_slice(bytes);
            if self.mode.echo {
                fx.echo.extend_from_slice(bytes);
            }
            return fx;
        }
        if bytes.len() > 1 && bytes[0] == 0x1b {
            return fx;
        }
        if bytes.len() == 1 {
            match bytes[0] {
                b'\r' | b'\n' => {
                    self.line.push(b'\n');
                    fx.to_child = core::mem::take(&mut self.line);
                    self.echo(&mut fx, b"\r\n");
                    return fx;
                }
                0x7f | 0x08 => {
                    if self.pop_scalar() {
                        self.echo(&mut fx, ERASE_ONE);
                    }
                    return fx;
                }
                0x15 => {
                    while self.pop_scalar() {
                        self.echo(&mut fx, ERASE_ONE);
                    }
                    return fx;
                }
                0x03 => {
                    self.line.clear();
                    fx.to_child.push(0x03);
                    self.echo(&mut fx, b"^C\r\n");
                    return fx;
                }
                0x04 => {
                    if self.line.is_empty() {
                        self.closed = true;
                        fx.close = true;
                    } else {
                        fx.to_child = core::mem::take(&mut self.line);
                    }
                    return fx;
                }
                b'\t' => {}
                c if c < 0x20 => return fx,
                _ => {}
            }
        }
        self.insert(bytes, &mut fx);
        fx
    }

    /// Text that is not keystrokes (a sanitized paste, §6.1): inserted
    /// literally in cooked mode, an LF ending a line as Enter would; sent as
    /// is in raw mode.
    pub fn text(&mut self, text: &[u8]) -> Effect {
        let mut fx = Effect::default();
        if self.closed {
            return fx;
        }
        if !self.mode.cooked {
            fx.to_child.extend_from_slice(text);
            if self.mode.echo {
                fx.echo.extend_from_slice(text);
            }
            return fx;
        }
        for piece in text.split_inclusive(|&b| b == b'\n') {
            let (body, ends) = match piece.split_last() {
                Some((b'\n', body)) => (body, true),
                _ => (piece, false),
            };
            self.insert(body, &mut fx);
            if ends {
                self.line.push(b'\n');
                fx.to_child.append(&mut self.line);
                self.echo(&mut fx, b"\r\n");
            }
        }
        fx
    }

    fn insert(&mut self, bytes: &[u8], fx: &mut Effect) {
        if self.line.len() + bytes.len() > LINE_MAX {
            fx.bell = true;
            return;
        }
        self.line.extend_from_slice(bytes);
        self.echo(fx, bytes);
    }

    fn echo(&self, fx: &mut Effect, bytes: &[u8]) {
        if self.mode.echo {
            fx.echo.extend_from_slice(bytes);
        }
    }

    /// Remove the last UTF-8 scalar; `false` if the line was empty.
    fn pop_scalar(&mut self) -> bool {
        let Some(mut i) = self.line.len().checked_sub(1) else {
            return false;
        };
        while i > 0 && self.line[i] & 0xC0 == 0x80 {
            i -= 1;
        }
        self.line.truncate(i);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(d: &mut Discipline, s: &[u8]) -> Effect {
        let mut all = Effect::default();
        for &b in s {
            let fx = d.key(&[b]);
            all.to_child.extend(fx.to_child);
            all.echo.extend(fx.echo);
            all.bell |= fx.bell;
            all.close |= fx.close;
        }
        all
    }

    #[test]
    fn a_new_child_is_cooked_with_echo() {
        assert_eq!(Discipline::new().mode(), Mode::COOKED_ECHO);
    }

    #[test]
    fn cooked_delivers_at_enter_and_echoes() {
        let mut d = Discipline::new();
        let fx = typed(&mut d, b"ls");
        assert!(fx.to_child.is_empty());
        assert_eq!(fx.echo, b"ls");
        let fx = d.key(b"\r");
        assert_eq!(fx.to_child, b"ls\n");
        assert_eq!(fx.echo, b"\r\n");
        assert!(d.pending().is_empty());
    }

    #[test]
    fn backspace_removes_one_scalar() {
        let mut d = Discipline::new();
        typed(&mut d, "aé".as_bytes());
        let fx = d.key(&[0x7f]);
        assert_eq!(fx.echo, ERASE_ONE);
        assert_eq!(d.pending(), b"a");
        d.key(&[0x7f]);
        assert_eq!(d.key(&[0x7f]), Effect::default()); // empty: nothing
        assert_eq!(d.key(b"\r").to_child, b"\n");
    }

    #[test]
    fn ctrl_u_clears_the_line() {
        let mut d = Discipline::new();
        typed(&mut d, b"abc");
        let fx = d.key(&[0x15]);
        assert_eq!(fx.echo, [ERASE_ONE, ERASE_ONE, ERASE_ONE].concat());
        assert!(d.pending().is_empty());
    }

    #[test]
    fn ctrl_c_discards_and_sends_etx() {
        let mut d = Discipline::new();
        typed(&mut d, b"oops");
        let fx = d.key(&[0x03]);
        assert_eq!(fx.to_child, [0x03]);
        assert_eq!(fx.echo, b"^C\r\n");
        assert!(d.pending().is_empty());
    }

    #[test]
    fn ctrl_d_is_eof_on_an_empty_line_and_flush_otherwise() {
        let mut d = Discipline::new();
        typed(&mut d, b"ab");
        let fx = d.key(&[0x04]);
        assert_eq!(fx.to_child, b"ab");
        assert!(!fx.close);
        let fx = d.key(&[0x04]);
        assert!(fx.close);
        // After EOF nothing more is sent.
        assert_eq!(typed(&mut d, b"x\r"), Effect::default());
    }

    #[test]
    fn echo_off_hides_but_still_edits() {
        let mut d = Discipline::new();
        d.set_mode(Mode {
            cooked: true,
            echo: false,
        });
        let fx = typed(&mut d, b"secret\r");
        assert_eq!(fx.to_child, b"secret\n");
        assert!(fx.echo.is_empty());
    }

    #[test]
    fn raw_passes_every_key_through_unechoed() {
        let mut d = Discipline::new();
        d.set_mode(Mode::RAW);
        assert_eq!(d.key(b"a").to_child, b"a");
        assert_eq!(d.key(b"\x1b[A").to_child, b"\x1b[A");
        assert_eq!(d.key(&[0x03]).to_child, [0x03]);
        assert_eq!(d.key(&[0x04]).to_child, [0x04]);
        assert!(d.key(b"a").echo.is_empty());
    }

    #[test]
    fn going_raw_delivers_the_partial_line() {
        let mut d = Discipline::new();
        typed(&mut d, b"ab");
        assert_eq!(d.set_mode(Mode::RAW).to_child, b"ab");
        assert!(d.pending().is_empty());
    }

    #[test]
    fn cooked_drops_key_sequences_and_stray_controls() {
        let mut d = Discipline::new();
        assert_eq!(d.key(b"\x1b[A"), Effect::default());
        assert_eq!(d.key(&[0x1b]), Effect::default());
        assert_eq!(d.key(&[0x01]), Effect::default());
        assert_eq!(d.key(b"\t").echo, b"\t");
    }

    #[test]
    fn a_full_line_rings_the_bell_and_drops() {
        let mut d = Discipline::new();
        let fx = typed(&mut d, &[b'x'; LINE_MAX]);
        assert!(!fx.bell);
        let fx = d.key(b"y");
        assert!(fx.bell);
        assert!(fx.echo.is_empty());
        assert_eq!(d.pending().len(), LINE_MAX);
    }

    /// A sanitized paste is text: LF ends a line, and nothing in it acts as
    /// a control (the sanitizer already removed them).
    #[test]
    fn paste_text_in_cooked_mode() {
        let mut d = Discipline::new();
        typed(&mut d, b"> ");
        let fx = d.text(b"one\ntwo");
        assert_eq!(fx.to_child, b"> one\n");
        assert_eq!(fx.echo, b"one\r\ntwo");
        assert_eq!(d.pending(), b"two");
    }
}
