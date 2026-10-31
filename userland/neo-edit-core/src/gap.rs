// SPDX-License-Identifier: MPL-2.0
//! The gap buffer: UTF-8 bytes with a hole at the last edit.
//!
//! Text is `buf[..gap_start] ++ buf[gap_end..]`. An edit moves the gap to
//! its offset first, so a run of edits at the cursor costs no copying. The
//! gap only ever sits on a char boundary, and the text outside it is valid
//! UTF-8 at all times (checked by `debug_assert_valid` and by the KATs).

use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

/// The smallest gap a reallocation leaves.
const MIN_GAP: usize = 64;

#[derive(Clone, Debug, Default)]
pub struct GapBuffer {
    buf: Vec<u8>,
    gap_start: usize,
    gap_end: usize,
}

impl GapBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_text(text: &str) -> Self {
        let mut buf = Vec::with_capacity(text.len() + MIN_GAP);
        buf.extend_from_slice(text.as_bytes());
        let gap_start = buf.len();
        buf.resize(gap_start + MIN_GAP, 0);
        Self {
            gap_end: buf.len(),
            buf,
            gap_start,
        }
    }

    /// The text's length in bytes.
    pub fn len(&self) -> usize {
        self.buf.len() - (self.gap_end - self.gap_start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Where the gap is (a byte offset into the text).
    pub fn gap_at(&self) -> usize {
        self.gap_start
    }

    /// The text as two halves: before the gap, after it.
    pub fn halves(&self) -> (&str, &str) {
        // Both halves are valid UTF-8 by the type's invariant; `from_utf8`
        // checks it rather than assuming it.
        let a = core::str::from_utf8(&self.buf[..self.gap_start]).expect("gap buffer: front half");
        let b = core::str::from_utf8(&self.buf[self.gap_end..]).expect("gap buffer: back half");
        (a, b)
    }

    /// The byte at text offset `at`.
    pub fn byte(&self, at: usize) -> u8 {
        if at < self.gap_start {
            self.buf[at]
        } else {
            self.buf[at + (self.gap_end - self.gap_start)]
        }
    }

    /// Whether `at` is a char boundary of the text (0 and `len` are).
    pub fn is_char_boundary(&self, at: usize) -> bool {
        at == 0 || at == self.len() || (at < self.len() && (self.byte(at) as i8) >= -0x40)
    }

    /// A copy of `range` of the text. `range` must lie on char boundaries.
    pub fn slice(&self, range: Range<usize>) -> String {
        assert!(range.start <= range.end && range.end <= self.len());
        assert!(self.is_char_boundary(range.start) && self.is_char_boundary(range.end));
        let (a, b) = self.halves();
        let mut out = String::with_capacity(range.len());
        if range.start < a.len() {
            out.push_str(&a[range.start..range.end.min(a.len())]);
        }
        if range.end > a.len() {
            out.push_str(&b[range.start.max(a.len()) - a.len()..range.end - a.len()]);
        }
        out
    }

    /// The whole text.
    pub fn text(&self) -> String {
        self.slice(0..self.len())
    }

    /// Insert `text` at `at`, a char boundary.
    pub fn insert(&mut self, at: usize, text: &str) {
        assert!(
            at <= self.len() && self.is_char_boundary(at),
            "insert off a char boundary"
        );
        self.move_gap(at);
        self.reserve(text.len());
        self.buf[self.gap_start..self.gap_start + text.len()].copy_from_slice(text.as_bytes());
        self.gap_start += text.len();
        self.debug_assert_valid();
    }

    /// Remove `range` (on char boundaries) and return what it held.
    pub fn delete(&mut self, range: Range<usize>) -> String {
        let removed = self.slice(range.clone());
        self.move_gap(range.start);
        self.gap_end += range.len();
        self.debug_assert_valid();
        removed
    }

    fn move_gap(&mut self, at: usize) {
        let gap = self.gap_end - self.gap_start;
        if at < self.gap_start {
            let n = self.gap_start - at;
            self.buf.copy_within(at..self.gap_start, self.gap_end - n);
            self.gap_start = at;
            self.gap_end -= n;
        } else if at > self.gap_start {
            let n = at - self.gap_start;
            self.buf
                .copy_within(self.gap_end..self.gap_end + n, self.gap_start);
            self.gap_start += n;
            self.gap_end += n;
        }
        debug_assert_eq!(self.gap_end - self.gap_start, gap);
    }

    fn reserve(&mut self, need: usize) {
        let gap = self.gap_end - self.gap_start;
        if gap >= need {
            return;
        }
        let grow = (need - gap).max(self.len() / 2).max(MIN_GAP);
        let tail = self.buf.len() - self.gap_end;
        self.buf.resize(self.buf.len() + grow, 0);
        let new_end = self.buf.len() - tail;
        self.buf
            .copy_within(self.gap_end..self.gap_end + tail, new_end);
        self.gap_end = new_end;
    }

    fn debug_assert_valid(&self) {
        debug_assert!(core::str::from_utf8(&self.buf[..self.gap_start]).is_ok());
        debug_assert!(core::str::from_utf8(&self.buf[self.gap_end..]).is_ok());
    }
}
