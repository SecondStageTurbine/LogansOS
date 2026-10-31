// SPDX-License-Identifier: MPL-2.0
//! The selection, and where a cursor may stop (THINK §2, §3.1).
//!
//! A cursor stops only on extended-grapheme-cluster boundaries (UAX #29, via
//! `unicode-segmentation`), so it never lands inside a combining sequence or
//! an emoji ZWJ sequence. LF is a Control: UAX #29 always breaks before and
//! after it (GB4, GB5), so the boundaries in a line, `\n` included, are
//! exactly the document's boundaries there, and every move works on one
//! line's text.

use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

/// `anchor` stays put while `head` moves; empty when they are equal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub const fn caret(at: usize) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }
}

/// The grapheme boundary after `at` in `text`, if any.
pub fn next_grapheme(text: &str, at: usize) -> Option<usize> {
    GraphemeCursor::new(at, text.len(), true)
        .next_boundary(text, 0)
        .ok()
        .flatten()
}

/// The grapheme boundary before `at` in `text`, if any.
pub fn prev_grapheme(text: &str, at: usize) -> Option<usize> {
    GraphemeCursor::new(at, text.len(), true)
        .prev_boundary(text, 0)
        .ok()
        .flatten()
}

/// The nearest grapheme boundary at or before `at`.
pub fn floor_grapheme(text: &str, at: usize) -> usize {
    if GraphemeCursor::new(at, text.len(), true).is_boundary(text, 0) == Ok(true) {
        at
    } else {
        prev_grapheme(text, at).unwrap_or(0)
    }
}

fn is_word(segment: &str) -> bool {
    segment.chars().next().is_some_and(char::is_alphanumeric)
}

/// The end of the first word ending after `at` (Ctrl+Right).
pub fn word_right(text: &str, at: usize) -> Option<usize> {
    text.split_word_bound_indices()
        .filter(|(_, w)| is_word(w))
        .map(|(i, w)| i + w.len())
        .find(|&end| end > at)
}

/// The start of the last word starting before `at` (Ctrl+Left).
pub fn word_left(text: &str, at: usize) -> Option<usize> {
    text.split_word_bound_indices()
        .filter(|(_, w)| is_word(w))
        .map(|(i, _)| i)
        .take_while(|&start| start < at)
        .last()
}
