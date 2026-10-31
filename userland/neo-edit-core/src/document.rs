// SPDX-License-Identifier: MPL-2.0
//! One open document: the buffer, its line index, the selection and the
//! history, kept consistent by one edit path (`apply`).

use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use crate::cursor::{self, Selection};
use crate::format::{self, Format, NotText};
use crate::gap::GapBuffer;
use crate::history::{Edit, History, Kind};
use crate::lines::LineIndex;

/// Cursor moves the core knows without layout. Up, Down and the page moves
/// are visual and live in `wrap`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    DocStart,
    DocEnd,
}

/// The lines an edit replaced, for the wrap cache: `old` lines from
/// `first` became `new` lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineChange {
    pub first: usize,
    pub old: usize,
    pub new: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Document {
    buf: GapBuffer,
    lines: LineIndex,
    sel: Selection,
    history: History,
    format: Option<Format>,
}

impl Document {
    /// A new, empty document.
    pub fn new() -> Self {
        Self {
            format: Some(Format::NEW),
            ..Self::default()
        }
    }

    /// A file's bytes, if they are text (THINK §3.4).
    pub fn load(bytes: &[u8]) -> Result<Self, NotText> {
        let (text, format) = format::load(bytes)?;
        Ok(Self {
            lines: LineIndex::scan(&text),
            buf: GapBuffer::from_text(&text),
            format: Some(format),
            ..Self::default()
        })
    }

    /// The bytes to write back, in the file's own format.
    pub fn save_bytes(&self) -> Vec<u8> {
        format::save(&self.text(), &self.format())
    }

    pub fn format(&self) -> Format {
        self.format.unwrap_or(Format::NEW)
    }

    pub fn text(&self) -> String {
        self.buf.text()
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn buffer(&self) -> &GapBuffer {
        &self.buf
    }

    pub fn lines(&self) -> &LineIndex {
        &self.lines
    }

    /// Line `line`'s byte range, without its `\n`.
    pub fn line_range(&self, line: usize) -> Range<usize> {
        let start = self.lines.start(line);
        let end = if line + 1 < self.lines.count() {
            self.lines.start(line + 1) - 1
        } else {
            self.len()
        };
        start..end
    }

    pub fn line_text(&self, line: usize) -> String {
        self.buf.slice(self.line_range(line))
    }

    pub fn selection(&self) -> Selection {
        self.sel
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn is_dirty(&self) -> bool {
        self.history.is_dirty()
    }

    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// Put the cursor at `at` (snapped to a grapheme boundary), keeping the
    /// anchor when `extend`.
    pub fn set_head(&mut self, at: usize, extend: bool) {
        let at = self.snap(at.min(self.len()));
        self.history.break_run();
        self.sel = if extend {
            Selection {
                anchor: self.sel.anchor,
                head: at,
            }
        } else {
            Selection::caret(at)
        };
    }

    pub fn select_all(&mut self) {
        self.history.break_run();
        self.sel = Selection {
            anchor: 0,
            head: self.len(),
        };
    }

    pub fn selected_text(&self) -> String {
        self.buf.slice(self.sel.start()..self.sel.end())
    }

    /// The nearest grapheme boundary at or before `at`.
    pub fn snap(&self, at: usize) -> usize {
        let mut at = at.min(self.len());
        while !self.buf.is_char_boundary(at) {
            at -= 1;
        }
        let line = self.lines.line_of(at);
        let range = self.line_range(line);
        let text = self.line_with_end(line);
        range.start + cursor::floor_grapheme(&text, at - range.start)
    }

    /// Line `line` with its `\n`, so its boundaries are the document's.
    fn line_with_end(&self, line: usize) -> String {
        let range = self.line_range(line);
        let end = if range.end < self.len() {
            range.end + 1
        } else {
            range.end
        };
        self.buf.slice(range.start..end)
    }

    pub fn motion(&mut self, motion: Motion, extend: bool) {
        if !extend && !self.sel.is_empty() {
            match motion {
                Motion::Left => return self.set_head(self.sel.start(), false),
                Motion::Right => return self.set_head(self.sel.end(), false),
                _ => {}
            }
        }
        let at = self.target(motion, self.sel.head);
        self.set_head(at, extend);
    }

    /// Where `motion` from `at` lands.
    pub fn target(&self, motion: Motion, at: usize) -> usize {
        let line = self.lines.line_of(at);
        let range = self.line_range(line);
        let text = self.line_with_end(line);
        let local = at - range.start;
        let up = |n: Option<usize>| n.map(|n| range.start + n);
        // From a line's start, Left steps back over the line end before it:
        // a lone `\n`, or a CR LF pair, which is one grapheme (GB3).
        let back_over_line_end = || {
            let prev = self.line_with_end(line - 1);
            self.lines.start(line - 1) + cursor::prev_grapheme(&prev, prev.len()).unwrap_or(0)
        };
        match motion {
            Motion::Right => up(cursor::next_grapheme(&text, local)).unwrap_or(at),
            Motion::Left | Motion::WordLeft if local == 0 && line == 0 => 0,
            Motion::Left | Motion::WordLeft if local == 0 => back_over_line_end(),
            Motion::Left => up(cursor::prev_grapheme(&text, local)).unwrap_or(at),
            Motion::WordRight => {
                up(cursor::word_right(&text, local)).unwrap_or(if range.end < self.len() {
                    range.end + 1
                } else {
                    range.end
                })
            }
            Motion::WordLeft => up(cursor::word_left(&text, local)).unwrap_or(range.start),
            Motion::LineStart => range.start,
            Motion::LineEnd => range.end,
            Motion::DocStart => 0,
            Motion::DocEnd => self.len(),
        }
    }

    /// Typed text replaces the selection; a typing run is one undo step.
    pub fn type_text(&mut self, text: &str) -> LineChange {
        let kind = if self.sel.is_empty() {
            Kind::Typing
        } else {
            Kind::Other
        };
        self.replace_selection(&format::normalize(text), kind)
    }

    /// Pasted text replaces the selection as one undo step.
    pub fn paste(&mut self, text: &str) -> LineChange {
        self.replace_selection(&format::normalize(text), Kind::Other)
    }

    /// Remove the selection and return it (one undo step).
    pub fn cut(&mut self) -> (String, LineChange) {
        let text = self.selected_text();
        (text, self.replace_selection("", Kind::Other))
    }

    /// Backspace: the selection, or the grapheme before the cursor.
    pub fn backspace(&mut self) -> Option<LineChange> {
        if !self.sel.is_empty() {
            return Some(self.replace_selection("", Kind::Other));
        }
        let at = self.sel.head;
        let from = self.target(Motion::Left, at);
        (from < at).then(|| self.edit(from..at, "", Kind::Backspace))
    }

    /// Delete: the selection, or the grapheme after the cursor.
    pub fn delete_forward(&mut self) -> Option<LineChange> {
        if !self.sel.is_empty() {
            return Some(self.replace_selection("", Kind::Other));
        }
        let at = self.sel.head;
        let to = self.target(Motion::Right, at);
        (to > at).then(|| self.edit(at..to, "", Kind::DeleteForward))
    }

    fn replace_selection(&mut self, text: &str, kind: Kind) -> LineChange {
        self.edit(self.sel.start()..self.sel.end(), text, kind)
    }

    fn edit(&mut self, range: Range<usize>, text: &str, kind: Kind) -> LineChange {
        let before = self.sel;
        let after = Selection::caret(range.start + text.len());
        let (removed, change) = self.apply(range.start, range.len(), text);
        self.sel = after;
        self.history.record(
            Edit {
                at: range.start,
                removed,
                inserted: text.into(),
                before,
                after,
            },
            kind,
        );
        change
    }

    /// The one place the buffer and the line index change.
    pub(crate) fn apply(&mut self, at: usize, remove: usize, insert: &str) -> (String, LineChange) {
        let first = self.lines.line_of(at);
        let old_last = self.lines.line_of(at + remove);
        let removed = self.buf.delete(at..at + remove);
        self.lines.delete(at..at + remove);
        self.buf.insert(at, insert);
        self.lines.insert(at, insert);
        let new_last = self.lines.line_of(at + insert.len());
        let change = LineChange {
            first,
            old: old_last - first + 1,
            new: new_last - first + 1,
        };
        (removed, change)
    }

    pub fn undo(&mut self) -> Option<LineChange> {
        let edit = self.history.undo()?;
        let (_, change) = self.apply(edit.at, edit.inserted.len(), &edit.removed);
        self.sel = edit.before;
        Some(change)
    }

    pub fn redo(&mut self) -> Option<LineChange> {
        let edit = self.history.redo()?;
        let (_, change) = self.apply(edit.at, edit.removed.len(), &edit.inserted);
        self.sel = edit.after;
        Some(change)
    }

    /// The next match of `query` starting at or after `from` (or ending at
    /// or before it, backwards), wrapping round the document once. ASCII
    /// letters match either case when `fold_case`.
    pub fn find(
        &self,
        query: &str,
        from: usize,
        forward: bool,
        fold_case: bool,
    ) -> Option<Range<usize>> {
        if query.is_empty() {
            return None;
        }
        let text = self.text();
        let hay = text.as_bytes();
        let needle = query.as_bytes();
        if needle.len() > hay.len() {
            return None;
        }
        let at = |i: usize| -> bool {
            text.is_char_boundary(i)
                && hay[i..i + needle.len()].iter().zip(needle).all(|(a, b)| {
                    if fold_case {
                        a.eq_ignore_ascii_case(b)
                    } else {
                        a == b
                    }
                })
        };
        let last = hay.len() - needle.len();
        let hit = if forward {
            (from.min(last + 1)..=last)
                .chain(0..from.min(last + 1))
                .find(|&i| at(i))
        } else {
            // Matches ending at or before `from`, then the rest from the end.
            let top = from.checked_sub(needle.len()).map(|t| t.min(last));
            let wrap_from = top.map_or(0, |t| t + 1);
            top.into_iter()
                .flat_map(|t| (0..=t).rev())
                .chain((wrap_from..=last).rev())
                .find(|&i| at(i))
        };
        hit.map(|i| i..i + needle.len())
    }
}
