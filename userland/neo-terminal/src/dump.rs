// SPDX-License-Identifier: MPL-2.0
//! The grid as text: the scenarios' read-back of what the window shows.
//!
//! A row is its cells' characters, a blank cell a space, trailing blanks
//! trimmed. [`Dumper`] reports only the rows that changed since its last
//! report, so a scenario can follow the screen on the serial log without the
//! whole grid on every frame.

use alloc::string::String;
use alloc::vec::Vec;

/// Row `row`'s text, trailing blanks trimmed.
pub fn row_text(screen: &vt100::Screen, row: u16) -> String {
    let (_, cols) = screen.size();
    let mut text = String::new();
    for col in 0..cols.get() {
        let Some(cell) = screen.cell(row, col) else {
            break;
        };
        if cell.is_wide_continuation() {
            continue;
        }
        if cell.has_contents() {
            text.push_str(cell.contents());
        } else {
            text.push(' ');
        }
    }
    let trimmed = text.trim_end().len();
    text.truncate(trimmed);
    text
}

/// The rows that changed since the last report.
#[derive(Default)]
pub struct Dumper {
    last: Vec<String>,
}

impl Dumper {
    pub fn new() -> Self {
        Self::default()
    }

    /// `(row, text)` for every row whose text differs from the last report
    /// (every non-empty row the first time).
    pub fn changed(&mut self, screen: &vt100::Screen) -> Vec<(u16, String)> {
        let (rows, _) = screen.size();
        let rows = rows.get();
        self.last.resize(usize::from(rows), String::new());
        let mut out = Vec::new();
        for row in 0..rows {
            let text = row_text(screen, row);
            let slot = &mut self.last[usize::from(row)];
            if *slot != text {
                *slot = text.clone();
                out.push((row, text));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU16;
    use neo_term_core::term::Term;

    fn term(rows: u16, cols: u16) -> Term {
        Term::new(
            NonZeroU16::new(rows).unwrap(),
            NonZeroU16::new(cols).unwrap(),
        )
    }

    #[test]
    fn row_text_trims_and_keeps_inner_blanks() {
        let mut t = term(3, 10);
        let _ = t.process(b"a  b\r\n\x1b[2;5Hx");
        assert_eq!(row_text(t.screen(), 0), "a  b");
        assert_eq!(row_text(t.screen(), 1), "    x");
        assert_eq!(row_text(t.screen(), 2), "");
    }

    #[test]
    fn dumper_reports_only_changes() {
        let mut t = term(3, 10);
        let mut d = Dumper::new();
        let _ = t.process(b"neo> ");
        assert_eq!(d.changed(t.screen()), [(0, String::from("neo>"))]);
        assert!(d.changed(t.screen()).is_empty());
        let _ = t.process(b"ls\r\nhi");
        assert_eq!(
            d.changed(t.screen()),
            [(0, String::from("neo> ls")), (1, String::from("hi"))]
        );
    }

    /// A wide character is one entry, not two.
    #[test]
    fn wide_character_counts_once() {
        let mut t = term(1, 10);
        let _ = t.process("a\u{4e2d}b".as_bytes());
        assert_eq!(row_text(t.screen(), 0), "a\u{4e2d}b");
    }
}
