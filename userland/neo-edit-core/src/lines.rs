// SPDX-License-Identifier: MPL-2.0
//! The line index: the byte offset where each line starts.
//!
//! The buffer holds LF line ends only (`format` turns CRLF into LF on load),
//! so a line starts at 0 and after every `\n`. An edit updates the index in
//! place; the KATs check it against a fresh scan after every step.

use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineIndex {
    starts: Vec<usize>,
}

impl Default for LineIndex {
    fn default() -> Self {
        Self { starts: vec![0] }
    }
}

impl LineIndex {
    /// The index of `text`, by a full scan.
    pub fn scan(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self { starts }
    }

    /// How many lines (an empty text has one).
    pub fn count(&self) -> usize {
        self.starts.len()
    }

    /// Where line `line` starts.
    pub fn start(&self, line: usize) -> usize {
        self.starts[line]
    }

    /// The line holding byte offset `at`.
    pub fn line_of(&self, at: usize) -> usize {
        self.starts.partition_point(|&s| s <= at) - 1
    }

    /// Update for `text` inserted at `at`.
    pub fn insert(&mut self, at: usize, text: &str) {
        let line = self.line_of(at);
        for s in &mut self.starts[line + 1..] {
            *s += text.len();
        }
        let new = text
            .bytes()
            .enumerate()
            .filter(|&(_, b)| b == b'\n')
            .map(|(i, _)| at + i + 1);
        self.starts.splice(line + 1..line + 1, new);
    }

    /// Update for `range` deleted.
    pub fn delete(&mut self, range: Range<usize>) {
        // A line start `s` follows the `\n` at `s - 1`; it goes when that
        // `\n` was in the range.
        let first = self.starts.partition_point(|&s| s <= range.start);
        let last = self.starts.partition_point(|&s| s <= range.end);
        self.starts.drain(first..last);
        for s in &mut self.starts[first..] {
            *s -= range.len();
        }
    }
}
