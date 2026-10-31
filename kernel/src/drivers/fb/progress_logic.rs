// SPDX-License-Identifier: MPL-2.0
//! Pure layout rules for the framebuffer progress console
//! (`fb-progress-console`), shared with the host KATs in
//! `tools/fb-progress-logic-tests` (mounted by `#[path]`).
//!
//! The console mirrors kernel log lines so a photo of the screen carries the
//! boot evidence on a target with no serial port (the Zenbook S14). It wraps
//! to the top instead of scrolling: a scroll moves the whole framebuffer
//! (about 20 MB at 2880x1800) per line, a wrap moves nothing. The row after
//! the newest line is always cleared, so the newest line is the one above
//! the blank band.

/// Glyph cell size of the 8x16 bitmap font (`fb::font`).
pub const GLYPH_W: usize = 8;
pub const GLYPH_H: usize = 16;
/// Rows the console aims to show; the integer scale is the largest that
/// still fits this many.
pub const TARGET_ROWS: usize = 45;
/// Never draw glyphs larger than this.
pub const MAX_SCALE: usize = 4;

/// The integer glyph scale for a screen `height` pixels tall: 1 up to
/// 1439 lines (QEMU's 800, a 1080p panel), 2 from 1440 (the Zenbook's 1800).
pub const fn scale_for(height: usize) -> usize {
    let s = height / (GLYPH_H * TARGET_ROWS);
    if s < 1 {
        1
    } else if s > MAX_SCALE {
        MAX_SCALE
    } else {
        s
    }
}

/// Pixel rows at the bottom of the screen the console never draws into: the
/// scenario phase indicator's band (`scenario::phase_indicator`, at most
/// 200 px tall), where a bundle shows its big PASS/FAIL.
pub const RESERVED_BOTTOM: usize = 200;

/// Height of the console's text area on a screen `height` pixels tall.
pub const fn text_height(height: usize) -> usize {
    height.saturating_sub(RESERVED_BOTTOM)
}

/// Text grid for a `width` x `height` text area at `scale`: (cols, rows).
pub const fn grid(width: usize, height: usize, scale: usize) -> (usize, usize) {
    let cols = width / (GLYPH_W * scale);
    let rows = height / (GLYPH_H * scale);
    (
        if cols == 0 { 1 } else { cols },
        if rows == 0 { 1 } else { rows },
    )
}

/// What one byte does to the cursor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Draw at (row, col); the cursor has already moved on.
    Draw { row: usize, col: usize },
    /// A line ended (newline, or the line reached the right edge): clear
    /// `clear_row`, the row the cursor now sits on.
    NewLine { clear_row: usize },
    /// Nothing to draw (carriage return, or another control byte).
    Skip,
}

/// Cursor over a `cols` x `rows` grid that wraps from the last row to row 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cursor {
    cols: usize,
    rows: usize,
    row: usize,
    col: usize,
}

impl Cursor {
    pub const fn new(cols: usize, rows: usize) -> Self {
        Self {
            cols: if cols == 0 { 1 } else { cols },
            rows: if rows == 0 { 1 } else { rows },
            row: 0,
            col: 0,
        }
    }

    pub const fn position(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    fn advance_line(&mut self) -> Step {
        self.col = 0;
        self.row = (self.row + 1) % self.rows;
        Step::NewLine {
            clear_row: self.row,
        }
    }

    /// Feed one byte. A printable byte at the right edge first breaks the
    /// line (the caller clears the new row), then is drawn by the next call:
    /// so callers loop until a byte reports `Draw` or `Skip`.
    pub fn step(&mut self, byte: u8) -> Step {
        match byte {
            b'\n' => self.advance_line(),
            b'\r' => Step::Skip,
            0x20..=0x7e => {
                if self.col >= self.cols {
                    return self.advance_line();
                }
                let at = Step::Draw {
                    row: self.row,
                    col: self.col,
                };
                self.col += 1;
                at
            }
            _ => Step::Skip,
        }
    }
}

/// Which CPU this is, for the panic latch, from CPUID (R1 pass 2: validate
/// leaf 0xB before trusting it). Leaf 0xB's x2APIC ID (EDX) counts only when
/// the leaf exists (`max_leaf >= 0xb`) and is valid: its sub-leaf 0 reports a
/// non-zero logical-processor count in EBX[15:0] (Intel SDM, CPUID leaf 0BH:
/// "if EBX[15:0] is 0, the leaf is not supported"). Otherwise the initial
/// APIC ID in leaf 1's EBX[31:24].
pub const fn panic_cpu_id(max_leaf: u32, leaf_b_ebx: u32, leaf_b_edx: u32, leaf1_ebx: u32) -> u32 {
    if max_leaf >= 0xb && leaf_b_ebx & 0xffff != 0 {
        leaf_b_edx
    } else {
        leaf1_ebx >> 24
    }
}
