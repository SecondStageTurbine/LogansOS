// SPDX-License-Identifier: MPL-2.0
//! Soft wrap, and the visual moves that need it (THINK §4.1-§4.2).
//!
//! The core never draws. It asks a [`Measure`] for the x of every grapheme
//! boundary in a line; the editor app implements it over neo-text (T0, and
//! later T2 behind the same trait), and the KATs over a fixed-advance fake.
//! Wrapping is per logical line, cached, and invalidated only for the lines
//! an edit touched ([`LineChange`]) or all of them when the width changes.

use alloc::vec::Vec;
use core::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::document::{Document, LineChange};

pub trait Measure {
    /// The x of each grapheme boundary in `line` (a line without its `\n`),
    /// in order: `(0, 0.0)` first and `(line.len(), width)` last.
    fn boundaries(&self, line: &str, out: &mut Vec<(usize, f32)>);
}

/// Break `line` into rows no wider than `width`: after the last space that
/// fits, else at the last grapheme that fits, and never an empty row except
/// for an empty line. Returns byte ranges into `line`.
pub fn wrap_line(measure: &dyn Measure, line: &str, width: f32) -> Vec<Range<usize>> {
    let mut stops = Vec::new();
    measure.boundaries(line, &mut stops);
    let mut rows = Vec::new();
    let mut start = 0usize;
    let mut start_x = 0.0f32;
    let mut last_space: Option<(usize, f32)> = None;
    let mut prev: (usize, f32) = (0, 0.0);
    for &(at, x) in stops.iter().skip(1) {
        // Spaces hang at a row's end: never break before one.
        if line[prev.0..at].chars().all(|c| c == ' ' || c == '\t') {
            if at > start {
                last_space = Some((at, x));
            }
            prev = (at, x);
            continue;
        }
        if x - start_x > width && prev.0 > start {
            let (cut, cut_x) = last_space.unwrap_or(prev);
            rows.push(start..cut);
            start = cut;
            start_x = cut_x;
            last_space = None;
        }
        prev = (at, x);
    }
    rows.push(start..line.len());
    rows
}

/// A fixed advance per grapheme (tabs count as four): the KATs' measure,
/// and a stand-in until the app's neo-text measure exists.
pub struct FixedAdvance(pub f32);

impl Measure for FixedAdvance {
    fn boundaries(&self, line: &str, out: &mut Vec<(usize, f32)>) {
        out.clear();
        out.push((0, 0.0));
        let mut x = 0.0;
        for (i, g) in line.grapheme_indices(true) {
            x += if g == "\t" { 4.0 * self.0 } else { self.0 };
            out.push((i + g.len(), x));
        }
    }
}

/// The wrap of every logical line, computed when first needed.
#[derive(Clone, Debug, Default)]
pub struct WrapCache {
    width: f32,
    rows: Vec<Option<Vec<Range<usize>>>>,
}

impl WrapCache {
    pub fn new(width: f32, lines: usize) -> Self {
        Self {
            width,
            rows: alloc::vec![None; lines],
        }
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    /// A new width re-wraps everything, as lines come into view.
    pub fn set_width(&mut self, width: f32) {
        if width != self.width {
            self.width = width;
            self.rows.iter_mut().for_each(|r| *r = None);
        }
    }

    /// Forget the lines an edit replaced.
    pub fn edit(&mut self, change: LineChange) {
        let end = (change.first + change.old).min(self.rows.len());
        self.rows
            .splice(change.first..end, core::iter::repeat_n(None, change.new));
    }

    /// Whether line `line`'s wrap is cached.
    pub fn is_cached(&self, line: usize) -> bool {
        self.rows.get(line).is_some_and(Option::is_some)
    }

    /// Line `line`'s rows, as byte ranges into the line.
    pub fn rows(&mut self, doc: &Document, measure: &dyn Measure, line: usize) -> &[Range<usize>] {
        if self.rows.len() != doc.lines().count() {
            self.rows.resize(doc.lines().count(), None);
        }
        let width = self.width;
        self.rows[line].get_or_insert_with(|| wrap_line(measure, &doc.line_text(line), width))
    }

    /// The (line, row) holding the cursor at `at`. A cursor at a row's end
    /// that is also the next row's start belongs to the next row.
    pub fn row_of(&mut self, doc: &Document, measure: &dyn Measure, at: usize) -> (usize, usize) {
        let line = doc.lines().line_of(at);
        let local = at - doc.lines().start(line);
        let rows = self.rows(doc, measure, line);
        let row = rows
            .iter()
            .position(|r| {
                local < r.end || (local == r.end && r.end == rows.last().map_or(0, |l| l.end))
            })
            .unwrap_or(rows.len() - 1);
        (line, row)
    }

    /// The x of the cursor at `at` within its row.
    pub fn x_of(&mut self, doc: &Document, measure: &dyn Measure, at: usize) -> f32 {
        let (line, row) = self.row_of(doc, measure, at);
        let start = doc.lines().start(line);
        let range = self.rows(doc, measure, line)[row].clone();
        let text = doc.line_text(line);
        let mut stops = Vec::new();
        measure.boundaries(&text[range.clone()], &mut stops);
        let local = at - start - range.start;
        stops.iter().find(|s| s.0 == local).map_or(0.0, |s| s.1)
    }

    /// Move `rows` visual rows up (negative) or down from `at`, keeping
    /// `goal_x` (the x the run of vertical moves started from). Clamps to
    /// the first and last rows of the document.
    pub fn vertical(
        &mut self,
        doc: &Document,
        measure: &dyn Measure,
        at: usize,
        rows: isize,
        goal_x: f32,
    ) -> usize {
        let (mut line, mut row) = self.row_of(doc, measure, at);
        let mut left = rows;
        while left != 0 {
            if left < 0 {
                if row > 0 {
                    row -= 1;
                } else if line > 0 {
                    line -= 1;
                    row = self.rows(doc, measure, line).len() - 1;
                } else {
                    return 0;
                }
                left += 1;
            } else {
                if row + 1 < self.rows(doc, measure, line).len() {
                    row += 1;
                } else if line + 1 < doc.lines().count() {
                    line += 1;
                    row = 0;
                } else {
                    return doc.len();
                }
                left -= 1;
            }
        }
        let range = self.rows(doc, measure, line)[row].clone();
        let text = doc.line_text(line);
        let mut stops = Vec::new();
        measure.boundaries(&text[range.clone()], &mut stops);
        // The nearest stop to `goal_x`; a wrapped row's last stop is the
        // next row's first, so it is left out unless this is the last row.
        let last_row = row + 1 == self.rows(doc, measure, line).len();
        let usable = if last_row {
            stops.len()
        } else {
            stops.len().saturating_sub(1).max(1)
        };
        let best = stops[..usable]
            .iter()
            .min_by(|a, b| (a.1 - goal_x).abs().total_cmp(&(b.1 - goal_x).abs()))
            .map_or(0, |s| s.0);
        doc.lines().start(line) + range.start + best
    }
}
