// SPDX-License-Identifier: MPL-2.0
//! Minimal framebuffer-backed text console for fd=1.

use spin::Mutex;

use super::console_logic::{ConsoleCursor, ConsoleSink};
use super::font::{GLYPH_HEIGHT, GLYPH_WIDTH};
use super::{Color, clear, dimensions, draw_glyph, fill_rect, scroll_up_pixels};

const FG: Color = Color::TEXT;
const BG: Color = Color::BLACK;
const CURSOR: Color = Color::MUTED;
const CURSOR_HEIGHT: usize = 2;

struct FramebufferConsole {
    cursor: ConsoleCursor,
}

static CONSOLE: Mutex<Option<FramebufferConsole>> = Mutex::new(None);

pub fn init() {
    let Some(console) = new_console() else {
        return;
    };
    clear(BG);
    draw_cursor(console.cursor.position());
    *CONSOLE.lock() = Some(console);
}

pub fn write_bytes(bytes: &[u8]) {
    let mut guard = CONSOLE.lock();
    if guard.is_none() {
        *guard = new_console();
    }
    let Some(console) = guard.as_mut() else {
        return;
    };

    erase_cursor(console.cursor.position());
    let mut sink = FbConsoleSink;
    for &byte in bytes {
        console.cursor.write_byte(byte, &mut sink);
    }
    draw_cursor(console.cursor.position());
}

fn new_console() -> Option<FramebufferConsole> {
    let (width, height) = dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    Some(FramebufferConsole {
        cursor: ConsoleCursor::new(width / GLYPH_WIDTH, height / GLYPH_HEIGHT),
    })
}

struct FbConsoleSink;

impl ConsoleSink for FbConsoleSink {
    fn put(&mut self, row: usize, col: usize, byte: u8) {
        let ch = byte as char;
        let x = col * GLYPH_WIDTH;
        let y = row * GLYPH_HEIGHT;
        draw_glyph(x, y, ch, FG, Some(BG));
    }

    fn clear(&mut self, row: usize, col: usize) {
        clear_cell((row, col));
    }

    fn scroll(&mut self) {
        scroll_up_pixels(GLYPH_HEIGHT, BG);
    }
}

fn clear_cell((row, col): (usize, usize)) {
    fill_rect(
        col * GLYPH_WIDTH,
        row * GLYPH_HEIGHT,
        GLYPH_WIDTH,
        GLYPH_HEIGHT,
        BG,
    );
}

fn erase_cursor(pos: (usize, usize)) {
    clear_cell(pos);
}

fn draw_cursor((row, col): (usize, usize)) {
    let x = col * GLYPH_WIDTH;
    let y = row * GLYPH_HEIGHT + GLYPH_HEIGHT.saturating_sub(CURSOR_HEIGHT);
    fill_rect(x, y, GLYPH_WIDTH, CURSOR_HEIGHT, CURSOR);
}
