// SPDX-License-Identifier: MPL-2.0
//! Pure text-console cursor/layout logic for the framebuffer console.

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct ConsoleCursor {
    cols: usize,
    rows: usize,
    col: usize,
    row: usize,
}

pub trait ConsoleSink {
    fn put(&mut self, row: usize, col: usize, byte: u8);
    fn clear(&mut self, row: usize, col: usize);
    fn scroll(&mut self);
}

impl ConsoleCursor {
    pub fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols: cols.max(1),
            rows: rows.max(1),
            col: 0,
            row: 0,
        }
    }

    pub fn position(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn write_byte<S: ConsoleSink>(&mut self, byte: u8, sink: &mut S) {
        match byte {
            b'\r' => self.col = 0,
            b'\n' => self.newline(sink),
            b'\t' => {
                for _ in 0..4 {
                    self.printable(b' ', sink);
                }
            }
            0x08 | 0x7f => self.backspace(sink),
            0x20..=0x7e => self.printable(byte, sink),
            _ => {}
        }
    }

    fn printable<S: ConsoleSink>(&mut self, byte: u8, sink: &mut S) {
        sink.put(self.row, self.col, byte);
        self.col += 1;
        if self.col >= self.cols {
            self.col = 0;
            self.row += 1;
            self.scroll_if_needed(sink);
        }
    }

    fn newline<S: ConsoleSink>(&mut self, sink: &mut S) {
        self.col = 0;
        self.row += 1;
        self.scroll_if_needed(sink);
    }

    fn backspace<S: ConsoleSink>(&mut self, sink: &mut S) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.cols - 1;
        } else {
            return;
        }
        sink.clear(self.row, self.col);
    }

    fn scroll_if_needed<S: ConsoleSink>(&mut self, sink: &mut S) {
        if self.row < self.rows {
            return;
        }
        sink.scroll();
        self.row = self.rows - 1;
    }
}
