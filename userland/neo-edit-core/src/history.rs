// SPDX-License-Identifier: MPL-2.0
//! Undo and redo: a linear list of edits, bounded by bytes (THINK §3.2).
//!
//! Typing coalesces into one edit, and so does a run of Backspace or
//! Delete, until the run is broken: a move, a pause (the app calls
//! `break_run`), a save, or a word boundary (a typed space or line end
//! closes the word it follows). A new edit after an undo drops the redo
//! tail. Past `cap` bytes the oldest edits are dropped and `trimmed` says so.

use alloc::collections::VecDeque;
use alloc::string::String;

use crate::cursor::Selection;

/// The history's bound: removed plus inserted bytes over all kept edits.
pub const HISTORY_BYTES: usize = 1024 * 1024;

/// One change: at `at`, `removed` was replaced by `inserted`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub at: usize,
    pub removed: String,
    pub inserted: String,
    pub before: Selection,
    pub after: Selection,
}

impl Edit {
    fn size(&self) -> usize {
        self.removed.len() + self.inserted.len()
    }
}

/// How an edit was made, for coalescing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Typing,
    Backspace,
    DeleteForward,
    /// Paste, cut, a replaced selection: never merged.
    Other,
}

#[derive(Clone, Debug)]
pub struct History {
    edits: VecDeque<(Edit, Kind)>,
    /// Edits before `pos` are applied; those from `pos` on can be redone.
    pos: usize,
    bytes: usize,
    cap: usize,
    /// The `pos` the file was last saved at; `None` once it can't be reached.
    saved: Option<usize>,
    /// Whether the next edit may merge with the last.
    open: bool,
    trimmed: bool,
}

impl Default for History {
    fn default() -> Self {
        Self::with_cap(HISTORY_BYTES)
    }
}

impl History {
    pub fn with_cap(cap: usize) -> Self {
        Self {
            edits: VecDeque::new(),
            pos: 0,
            bytes: 0,
            cap,
            saved: Some(0),
            open: false,
            trimmed: false,
        }
    }

    /// Bytes held by the kept edits.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Edits that can be undone.
    pub fn undo_depth(&self) -> usize {
        self.pos
    }

    /// Edits that can be redone.
    pub fn redo_depth(&self) -> usize {
        self.edits.len() - self.pos
    }

    /// Whether old edits were dropped to stay under the cap.
    pub fn trimmed(&self) -> bool {
        self.trimmed
    }

    /// Whether the text differs from what was last saved.
    pub fn is_dirty(&self) -> bool {
        self.saved != Some(self.pos)
    }

    pub fn mark_saved(&mut self) {
        self.saved = Some(self.pos);
        self.open = false;
    }

    /// End the current typing or deleting run.
    pub fn break_run(&mut self) {
        self.open = false;
    }

    pub fn record(&mut self, edit: Edit, kind: Kind) {
        while self.edits.len() > self.pos {
            let (dropped, _) = self.edits.pop_back().expect("redo tail");
            self.bytes -= dropped.size();
        }
        if self.saved.is_some_and(|s| s > self.pos) {
            self.saved = None;
        }
        if !self.merge(&edit, kind) {
            self.bytes += edit.size();
            self.edits.push_back((edit, kind));
            self.pos += 1;
        }
        self.open = kind != Kind::Other;
        while self.bytes > self.cap && !self.edits.is_empty() {
            let (dropped, _) = self.edits.pop_front().expect("oldest edit");
            self.bytes -= dropped.size();
            self.pos -= 1;
            self.saved = self.saved.and_then(|s| s.checked_sub(1));
            self.trimmed = true;
            self.open = false;
        }
    }

    /// Fold `edit` into the last edit when they are one run.
    fn merge(&mut self, edit: &Edit, kind: Kind) -> bool {
        if !self.open || self.saved == Some(self.pos) || kind == Kind::Other {
            return false;
        }
        let Some((last, last_kind)) = self.edits.back_mut() else {
            return false;
        };
        if *last_kind != kind {
            return false;
        }
        let merged = match kind {
            Kind::Typing => {
                let word_closed = last.inserted.ends_with([' ', '\n', '\t'])
                    && !edit.inserted.starts_with([' ', '\n', '\t']);
                if edit.removed.is_empty()
                    && last.removed.is_empty()
                    && last.at + last.inserted.len() == edit.at
                    && !word_closed
                {
                    last.inserted.push_str(&edit.inserted);
                    true
                } else {
                    false
                }
            }
            Kind::Backspace => {
                if edit.inserted.is_empty()
                    && last.inserted.is_empty()
                    && edit.at + edit.removed.len() == last.at
                {
                    let mut removed = edit.removed.clone();
                    removed.push_str(&last.removed);
                    last.removed = removed;
                    last.at = edit.at;
                    true
                } else {
                    false
                }
            }
            Kind::DeleteForward => {
                if edit.inserted.is_empty() && last.inserted.is_empty() && edit.at == last.at {
                    last.removed.push_str(&edit.removed);
                    true
                } else {
                    false
                }
            }
            Kind::Other => false,
        };
        if merged {
            last.after = edit.after;
            self.bytes += edit.size();
        }
        merged
    }

    /// The edit to undo (the caller applies its inverse).
    pub fn undo(&mut self) -> Option<Edit> {
        self.open = false;
        if self.pos == 0 {
            return None;
        }
        self.pos -= 1;
        Some(self.edits[self.pos].0.clone())
    }

    /// The edit to redo (the caller applies it again).
    pub fn redo(&mut self) -> Option<Edit> {
        self.open = false;
        let edit = self.edits.get(self.pos)?.0.clone();
        self.pos += 1;
        Some(edit)
    }
}
