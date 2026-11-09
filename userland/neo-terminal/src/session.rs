// SPDX-License-Identifier: MPL-2.0
//! One child's session (THINK §3.3, §3.4, §4).
//!
//! The terminal owns the child's stdin write end and its stdout read end. Keys
//! go through [`keys::encode`] and the [`Discipline`]; what the discipline
//! sends goes to the child's stdin through a [`Sink`] that never blocks (a
//! full pipe drops the rest of the bytes and rings the bell, §4). The child's
//! output goes into the screen, at most [`READ_BUDGET`] bytes per wake (§3.3:
//! a flooding child can't starve the keyboard); the answerbacks it asks for
//! go to its stdin through the same sink (§5.4).

use alloc::vec::Vec;
use core::num::NonZeroU16;
use neo_term_core::discipline::{Discipline, Effect};
use neo_term_core::keys::{self, MOD_LCTRL, MOD_LSHIFT, MOD_RCTRL, MOD_RSHIFT};
use neo_term_core::page::Mode;
use neo_term_core::term::Term;

/// The most child output the terminal parses per wake (§3.3).
pub const READ_BUDGET: usize = 16 * 1024;

/// A non-blocking write to the child's stdin.
pub trait Sink {
    fn try_write(&mut self, bytes: &[u8]) -> Write;
}

/// What one try-write did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Write {
    /// This many bytes went in (fewer than asked: the pipe is full).
    Wrote(usize),
    /// The reader is gone (EPIPE).
    Gone,
}

/// What a key asks of the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// Nothing beyond what the session did itself.
    None,
    /// Ctrl+Shift+C: kill the child (§4). Never sent to it.
    EndProcess,
    /// The discipline closed stdin (Ctrl+D on an empty cooked line): close
    /// the write end, so the child reads EOF.
    CloseStdin,
}

pub struct Session {
    term: Term,
    discipline: Discipline,
    /// The last `mode_req_seq` applied.
    mode_seq: u32,
    /// The child's stdin reader is gone (EPIPE): nothing more is written.
    stdin_gone: bool,
    /// Keys and answerbacks dropped because the stdin pipe was full.
    dropped: u64,
    /// A bell is pending (the app shows it, then clears it).
    bell: bool,
    exited: bool,
}

impl Session {
    pub fn new(rows: NonZeroU16, cols: NonZeroU16) -> Self {
        Self {
            term: Term::new(rows, cols),
            discipline: Discipline::new(),
            mode_seq: 0,
            stdin_gone: false,
            dropped: 0,
            bell: false,
            exited: false,
        }
    }

    /// The state every child starts from (§3.4): cooked with echo, the
    /// screen's modes reset, the line empty.
    pub fn reset_for_child(&mut self) {
        self.term.reset_for_child();
        self.discipline = Discipline::new();
        self.mode_seq = 0;
        self.stdin_gone = false;
        self.exited = false;
    }

    pub fn term(&self) -> &Term {
        &self.term
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.term.screen()
    }

    pub fn mode(&self) -> Mode {
        self.discipline.mode()
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn exited(&self) -> bool {
        self.exited
    }

    /// The pending bell, cleared.
    pub fn take_bell(&mut self) -> bool {
        core::mem::take(&mut self.bell)
    }

    /// A key press: a HID usage and the modifier byte (keys.rs's bits).
    pub fn key(&mut self, usage: u8, mods: u8, sink: &mut impl Sink) -> KeyAction {
        let ctrl = mods & (MOD_LCTRL | MOD_RCTRL) != 0;
        let shift = mods & (MOD_LSHIFT | MOD_RSHIFT) != 0;
        if ctrl && shift && usage == keys::usage::C {
            return KeyAction::EndProcess;
        }
        if self.exited {
            return KeyAction::None;
        }
        let encoded = keys::encode(usage, mods, self.term.decckm());
        let fx = self.discipline.key(encoded.as_bytes());
        self.apply(fx, sink)
    }

    /// The child asked for `mode` (the term page's request, already masked).
    pub fn set_mode(&mut self, mode: Mode, seq: u32, sink: &mut impl Sink) {
        self.mode_seq = seq;
        let fx = self.discipline.set_mode(mode);
        let _ = self.apply(fx, sink);
    }

    /// The `mode_req_seq` last applied.
    pub fn mode_seq(&self) -> u32 {
        self.mode_seq
    }

    /// Child output: parsed into the screen; answerbacks to its stdin.
    pub fn output(&mut self, bytes: &[u8], sink: &mut impl Sink) {
        let out = self.term.process(bytes);
        self.bell |= out.bell;
        if !out.answers.is_empty() {
            self.write_child(&out.answers, sink);
        }
    }

    /// The child ended (`code` its exit value, or `None` when it was killed).
    pub fn child_exited(&mut self, code: Option<i64>) {
        if self.exited {
            return;
        }
        self.exited = true;
        let mut line: Vec<u8> = Vec::new();
        line.extend_from_slice(b"\r\n[process exited");
        if let Some(code) = code {
            line.push(b' ');
            push_dec(&mut line, code);
        } else {
            line.extend_from_slice(b": ended");
        }
        line.extend_from_slice(b"]\r\n");
        let _ = self.term.process(&line);
    }

    fn apply(&mut self, fx: Effect, sink: &mut impl Sink) -> KeyAction {
        if !fx.echo.is_empty() {
            // The discipline's echo is drawn, never answered (it is the
            // user's own text, not child output).
            let _ = self.term.process(&fx.echo);
        }
        self.bell |= fx.bell;
        if !fx.to_child.is_empty() {
            self.write_child(&fx.to_child, sink);
        }
        if fx.close {
            KeyAction::CloseStdin
        } else {
            KeyAction::None
        }
    }

    /// One try-write; whatever doesn't fit is dropped with the bell (§4).
    fn write_child(&mut self, bytes: &[u8], sink: &mut impl Sink) {
        if self.stdin_gone {
            return;
        }
        match sink.try_write(bytes) {
            Write::Wrote(n) if n >= bytes.len() => {}
            Write::Wrote(_) => {
                self.dropped += 1;
                self.bell = true;
            }
            Write::Gone => self.stdin_gone = true,
        }
    }
}

fn push_dec(out: &mut Vec<u8>, value: i64) {
    if value < 0 {
        out.push(b'-');
    }
    let mut v = value.unsigned_abs();
    let mut digits = [0u8; 20];
    let mut n = 0;
    loop {
        digits[n] = b'0' + (v % 10) as u8;
        n += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    out.extend(digits[..n].iter().rev());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dump;
    use neo_term_core::keys::usage;

    /// A stdin pipe of `capacity` bytes that no one reads.
    struct Pipe {
        bytes: Vec<u8>,
        capacity: usize,
        gone: bool,
    }

    impl Pipe {
        fn new(capacity: usize) -> Self {
            Self {
                bytes: Vec::new(),
                capacity,
                gone: false,
            }
        }
    }

    impl Sink for Pipe {
        fn try_write(&mut self, bytes: &[u8]) -> Write {
            if self.gone {
                return Write::Gone;
            }
            let n = bytes.len().min(self.capacity - self.bytes.len());
            self.bytes.extend_from_slice(&bytes[..n]);
            Write::Wrote(n)
        }
    }

    fn session() -> Session {
        Session::new(NonZeroU16::new(3).unwrap(), NonZeroU16::new(10).unwrap())
    }

    fn type_text(s: &mut Session, text: &[u8], pipe: &mut Pipe) {
        for &b in text {
            let (u, m) = match b {
                b'a'..=b'z' => (usage::A + (b - b'a'), 0),
                b'\n' => (usage::ENTER, 0),
                _ => panic!("untyped byte {b:#x}"),
            };
            assert_eq!(s.key(u, m, pipe), KeyAction::None);
        }
    }

    /// Cooked with echo is where every child starts: the keys show at once,
    /// and the child gets the line only at Enter (§4).
    #[test]
    fn cooked_echoes_and_delivers_at_enter() {
        let mut s = session();
        let mut pipe = Pipe::new(4096);
        assert_eq!(s.mode(), Mode::COOKED_ECHO);
        type_text(&mut s, b"ab", &mut pipe);
        assert!(pipe.bytes.is_empty());
        assert_eq!(dump::row_text(s.screen(), 0), "ab");
        type_text(&mut s, b"\n", &mut pipe);
        assert_eq!(pipe.bytes, b"ab\n");
        assert_eq!(s.screen().cursor_position(), (1, 0));
    }

    /// Raw without echo (what neo-shell asks for): each key goes at once and
    /// nothing is drawn by the terminal.
    #[test]
    fn raw_sends_each_key_and_draws_nothing() {
        let mut s = session();
        let mut pipe = Pipe::new(4096);
        s.set_mode(Mode::RAW, 1, &mut pipe);
        assert_eq!(s.mode_seq(), 1);
        type_text(&mut s, b"a", &mut pipe);
        assert_eq!(pipe.bytes, b"a");
        assert_eq!(dump::row_text(s.screen(), 0), "");
        // The arrows (DECCKM off, the reset): CSI A.
        assert_eq!(s.key(usage::UP, 0, &mut pipe), KeyAction::None);
        assert_eq!(pipe.bytes, b"a\x1b[A");
    }

    /// Ctrl+Shift+C is End process, in either mode, and never reaches the
    /// child; Ctrl+C is 0x03.
    #[test]
    fn ctrl_shift_c_is_end_process_and_ctrl_c_is_etx() {
        let mut s = session();
        let mut pipe = Pipe::new(4096);
        assert_eq!(
            s.key(usage::C, MOD_LCTRL | MOD_LSHIFT, &mut pipe),
            KeyAction::EndProcess
        );
        assert_eq!(
            s.key(usage::C, MOD_RCTRL | MOD_RSHIFT, &mut pipe),
            KeyAction::EndProcess
        );
        assert!(pipe.bytes.is_empty());
        assert_eq!(s.key(usage::C, MOD_LCTRL, &mut pipe), KeyAction::None);
        assert_eq!(pipe.bytes, [0x03]);
    }

    /// Ctrl+D on an empty cooked line closes stdin (the child reads EOF).
    #[test]
    fn ctrl_d_on_empty_line_closes_stdin() {
        let mut s = session();
        let mut pipe = Pipe::new(4096);
        let d = usage::A + 3;
        assert_eq!(s.key(d, MOD_LCTRL, &mut pipe), KeyAction::CloseStdin);
        assert!(pipe.bytes.is_empty());
    }

    /// A child that stops reading: its stdin fills, and further keys are
    /// dropped with the bell. The terminal never waits (§3.3, §4).
    #[test]
    fn full_stdin_drops_keys_with_the_bell() {
        let mut s = session();
        let mut pipe = Pipe::new(2);
        s.set_mode(Mode::RAW, 1, &mut pipe);
        type_text(&mut s, b"ab", &mut pipe);
        assert!(!s.take_bell());
        type_text(&mut s, b"c", &mut pipe);
        assert_eq!(pipe.bytes, b"ab");
        assert_eq!(s.dropped(), 1);
        assert!(s.take_bell());
        assert!(!s.take_bell());
    }

    /// A gone reader (EPIPE) stops the writes; output and End process keep
    /// working.
    #[test]
    fn gone_reader_stops_writes() {
        let mut s = session();
        let mut pipe = Pipe::new(16);
        pipe.gone = true;
        s.set_mode(Mode::RAW, 1, &mut pipe);
        type_text(&mut s, b"a", &mut pipe);
        type_text(&mut s, b"b", &mut pipe);
        assert!(pipe.bytes.is_empty());
        s.output(b"hi", &mut pipe);
        assert_eq!(dump::row_text(s.screen(), 0), "hi");
        assert_eq!(
            s.key(usage::C, MOD_LCTRL | MOD_LSHIFT, &mut pipe),
            KeyAction::EndProcess
        );
    }

    /// The child's output is drawn; a DSR 6n gets its answerback on stdin
    /// (THINK §5.3's row: after `ab`, `CSI 1 ; 3 R`).
    #[test]
    fn output_draws_and_answers() {
        let mut s = session();
        let mut pipe = Pipe::new(4096);
        s.output(b"ab\x1b[6n", &mut pipe);
        assert_eq!(dump::row_text(s.screen(), 0), "ab");
        assert_eq!(pipe.bytes, b"\x1b[1;3R");
    }

    /// The exit line, once; then keys are not sent (End process still is).
    #[test]
    fn exit_is_shown_once() {
        let mut s = Session::new(NonZeroU16::new(4).unwrap(), NonZeroU16::new(40).unwrap());
        let mut pipe = Pipe::new(4096);
        s.child_exited(Some(3));
        s.child_exited(Some(4));
        assert_eq!(dump::row_text(s.screen(), 1), "[process exited 3]");
        assert_eq!(dump::row_text(s.screen(), 2), "");
        s.child_exited(None);
        let mut k = Session::new(NonZeroU16::new(4).unwrap(), NonZeroU16::new(40).unwrap());
        k.child_exited(None);
        assert_eq!(dump::row_text(k.screen(), 1), "[process exited: ended]");
        assert!(s.exited());
        type_text(&mut s, b"a", &mut pipe);
        assert!(pipe.bytes.is_empty());
    }

    /// A new child starts cooked with echo and DECCKM off, whatever the last
    /// one left (§3.4, `terminal_child_reset`).
    #[test]
    fn reset_for_child_restores_cooked_and_decckm() {
        let mut s = session();
        let mut pipe = Pipe::new(4096);
        s.set_mode(Mode::RAW, 1, &mut pipe);
        s.output(b"\x1b[?1h", &mut pipe);
        assert!(s.term().decckm());
        s.child_exited(Some(0));
        s.reset_for_child();
        assert_eq!(s.mode(), Mode::COOKED_ECHO);
        assert!(!s.term().decckm());
        assert!(!s.exited());
        assert_eq!(s.mode_seq(), 0);
    }
}
