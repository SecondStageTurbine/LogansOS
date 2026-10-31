// SPDX-License-Identifier: MPL-2.0
//! Pure policy and raster fixtures for the Arthur-on-Atrium showcase.

#![cfg_attr(not(feature = "arthur-compositor-baremetal"), allow(dead_code))]

pub const SURFACE_WIDTH: usize = 640;
pub const SURFACE_HEIGHT: usize = 400;
pub const SURFACE_STRIDE: usize = SURFACE_WIDTH * 4;
pub const SURFACE_BYTES: usize = SURFACE_STRIDE * SURFACE_HEIGHT;
/// DI-1b resize (feature `desktop-resize`): the surface is still created at
/// SURFACE_WIDTH x SURFACE_HEIGHT, but lives in one buffer sized for the
/// largest window the WM may configure, so a resize never reallocates. Off,
/// the buffer is exactly the surface (the kill-switch).
#[cfg(feature = "desktop-resize")]
pub const BUFFER_WIDTH: usize = 1280;
#[cfg(feature = "desktop-resize")]
pub const BUFFER_HEIGHT: usize = 800;
#[cfg(not(feature = "desktop-resize"))]
pub const BUFFER_WIDTH: usize = SURFACE_WIDTH;
#[cfg(not(feature = "desktop-resize"))]
pub const BUFFER_HEIGHT: usize = SURFACE_HEIGHT;
pub const BUFFER_STRIDE: usize = BUFFER_WIDTH * 4;
pub const BUFFER_BYTES: usize = BUFFER_STRIDE * BUFFER_HEIGHT;
/// DI-1b resize: the smallest size Arthur lays out (the WM's own minimum).
pub const MIN_WIDTH: usize = 320;
pub const MIN_HEIGHT: usize = 200;
pub const GLYPH_WIDTH: usize = 8;
pub const GLYPH_HEIGHT: usize = 16;
pub const SURFACE_COLS: usize = SURFACE_WIDTH / GLYPH_WIDTH;
pub const SURFACE_ROWS: usize = SURFACE_HEIGHT / GLYPH_HEIGHT;
pub const SURFACE_BG: u32 = 0x0013_1924;
pub const SURFACE_FG: u32 = 0x00EA_F6FF;
pub const ANSWER_CLIENT_SERIAL: u64 = 0x4152_5448_5552_0001;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputRoute {
    StandaloneScenarios,
    G1ClientSurface,
}

/// Rule-1's two-layer decision. A normal build or an unselected showcase build
/// retains the standalone Arthur/Atrium scenario routes.
pub const fn output_route(compiled_in: bool, selected: bool) -> OutputRoute {
    if compiled_in && selected {
        OutputRoute::G1ClientSurface
    } else {
        OutputRoute::StandaloneScenarios
    }
}

/// Decode one pressed ADR-074 key event into the same ASCII set as the legacy
/// HID decoder. Releases, pointer events, and unknown usages are ignored.
pub fn input_event_to_ascii(event: [u8; 8]) -> Option<u8> {
    if event[0] != 1 || event[1] & 1 == 0 {
        return None;
    }
    let code = u16::from_le_bytes([event[2], event[3]]);
    let keycode = u8::try_from(code).ok()?;
    let modifiers = u16::from_le_bytes([event[4], event[5]]);
    keycode_to_ascii(keycode, modifiers & 0x22 != 0)
}

/// A9-C: the keys that scroll Arthur's transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollKey {
    PageUp,
    PageDown,
    LineUp,
    LineDown,
    /// A9-C2: wheel notches (positive away from the user: toward older
    /// text), from a kind-5 event the WM forwarded.
    Wheel(i16),
}

/// A9-C2: lines the transcript moves per wheel notch.
pub const WHEEL_LINES: i64 = 3;

/// A9-C: a pressed PgUp (HID usage 0x4B), PgDn (0x4E), Up (0x52) or Down
/// (0x51), as a scroll key. Both keyboard drivers deliver these usages
/// (the USB boot decoder as-is, the i8042 through its E0 set-1 map); the
/// ASCII decoder above has no byte for them.
pub fn input_event_to_scroll(event: [u8; 8]) -> Option<ScrollKey> {
    // A9-C2: kind 5, a wheel event (`code` = the wheel, i16).
    if event[0] == 5 {
        let wheel = i16::from_le_bytes([event[2], event[3]]);
        return (wheel != 0).then_some(ScrollKey::Wheel(wheel));
    }
    if event[0] != 1 || event[1] & 1 == 0 {
        return None;
    }
    match u16::from_le_bytes([event[2], event[3]]) {
        0x4b => Some(ScrollKey::PageUp),
        0x4e => Some(ScrollKey::PageDown),
        0x52 => Some(ScrollKey::LineUp),
        0x51 => Some(ScrollKey::LineDown),
        _ => None,
    }
}

/// A9-C: PgUp and PgDn always scroll; Up and Down only while the input line
/// is empty (they are free then: there is no line to edit or recall).
pub const fn scroll_allowed(key: ScrollKey, input_empty: bool) -> bool {
    match key {
        ScrollKey::PageUp | ScrollKey::PageDown | ScrollKey::Wheel(_) => true,
        ScrollKey::LineUp | ScrollKey::LineDown => input_empty,
    }
}

pub fn keycode_to_ascii(keycode: u8, shifted: bool) -> Option<u8> {
    match keycode {
        0x04..=0x1d => {
            let base = b'a' + (keycode - 0x04);
            Some(if shifted { base - 32 } else { base })
        }
        0x1e..=0x26 => {
            let unshifted = b'1' + (keycode - 0x1e);
            let shifted_digits = b"!@#$%^&*(";
            Some(if shifted {
                shifted_digits[(keycode - 0x1e) as usize]
            } else {
                unshifted
            })
        }
        0x27 => Some(if shifted { b')' } else { b'0' }),
        0x28 => Some(b'\n'),
        // DS-1: Esc hands the keyboard back to Home (ignored elsewhere).
        0x29 => Some(0x1b),
        0x2a => Some(0x08),
        0x2b => Some(b'\t'),
        0x2c => Some(b' '),
        0x2d => Some(if shifted { b'_' } else { b'-' }),
        0x2e => Some(if shifted { b'+' } else { b'=' }),
        0x2f => Some(if shifted { b'{' } else { b'[' }),
        0x30 => Some(if shifted { b'}' } else { b']' }),
        0x31 => Some(if shifted { b'|' } else { b'\\' }),
        0x33 => Some(if shifted { b':' } else { b';' }),
        0x34 => Some(if shifted { b'"' } else { b'\'' }),
        0x35 => Some(if shifted { b'~' } else { b'`' }),
        0x36 => Some(if shifted { b'<' } else { b',' }),
        0x37 => Some(if shifted { b'>' } else { b'.' }),
        0x38 => Some(if shifted { b'?' } else { b'/' }),
        _ => None,
    }
}

/// A compact 5x7 uppercase font. Lowercase is deliberately folded to uppercase
/// so arbitrary model text remains legible without importing a font stack.
pub fn glyph_rows(byte: u8) -> [u8; 7] {
    match byte.to_ascii_uppercase() {
        b'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        b'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        b'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        b'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        b'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        b'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        b'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        b'J' => [0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0c],
        b'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        b'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        b'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        b'N' => [0x11, 0x19, 0x19, 0x15, 0x13, 0x13, 0x11],
        b'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        b'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        b'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        b'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        b'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        b'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        b'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        b'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        b'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        b'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        b'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        b'0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        b'1' => [0x04, 0x0c, 0x14, 0x04, 0x04, 0x04, 0x1f],
        b'2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        b'3' => [0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e],
        b'4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        b'5' => [0x1f, 0x10, 0x10, 0x1e, 0x01, 0x01, 0x1e],
        b'6' => [0x0e, 0x10, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        b'7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        b'9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x01, 0x0e],
        b' ' => [0; 7],
        b'.' => [0, 0, 0, 0, 0, 0x0c, 0x0c],
        b',' => [0, 0, 0, 0, 0x0c, 0x0c, 0x08],
        b':' => [0, 0x0c, 0x0c, 0, 0x0c, 0x0c, 0],
        b';' => [0, 0x0c, 0x0c, 0, 0x0c, 0x0c, 0x08],
        b'-' => [0, 0, 0, 0x1f, 0, 0, 0],
        b'_' => [0, 0, 0, 0, 0, 0, 0x1f],
        b'/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        b'\\' => [0x10, 0x08, 0x08, 0x04, 0x02, 0x02, 0x01],
        b'>' => [0x10, 0x08, 0x04, 0x02, 0x04, 0x08, 0x10],
        b'<' => [0x01, 0x02, 0x04, 0x08, 0x04, 0x02, 0x01],
        b'!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0, 0x04],
        b'?' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0, 0x04],
        b'=' => [0, 0x1f, 0, 0x1f, 0, 0, 0],
        b'+' => [0, 0x04, 0x04, 0x1f, 0x04, 0x04, 0],
        b'[' => [0x0e, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0e],
        b']' => [0x0e, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0e],
        b'(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        b')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        b'\'' => [0x04, 0x04, 0x08, 0, 0, 0, 0],
        b'"' => [0x0a, 0x0a, 0x14, 0, 0, 0, 0],
        _ => [0x0e, 0x11, 0x01, 0x02, 0x04, 0, 0x04],
    }
}

/// The pre-S2 TEXT surface renderer: an 80x25 grid of 8x16 cells, each glyph
/// the 5x7 table above doubled vertically. Kept verbatim as the
/// `arthur-legacy-5x7-font` kill-switch; host KATs prove it is bit-for-bit the
/// renderer Arthur shipped before the text engine.
pub struct LegacyGrid {
    row: usize,
    col: usize,
}

impl Default for LegacyGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl LegacyGrid {
    pub const fn new() -> Self {
        Self { row: 0, col: 0 }
    }

    /// `pixels` is the SURFACE_WIDTH x SURFACE_HEIGHT client mapping.
    pub fn clear(&mut self, pixels: &mut [u32]) {
        pixels[..SURFACE_WIDTH * SURFACE_HEIGHT].fill(SURFACE_BG);
    }

    pub fn write_byte(&mut self, pixels: &mut [u32], byte: u8) {
        match byte {
            b'\r' => {}
            b'\n' => self.newline(pixels),
            b'\t' => {
                for _ in 0..4 {
                    self.write_byte(pixels, b' ');
                }
            }
            0x08 | 0x7f => self.backspace(pixels),
            0x20..=0x7e => {
                if self.col >= SURFACE_COLS {
                    self.newline(pixels);
                }
                draw_cell(pixels, self.col, self.row, byte);
                self.col += 1;
            }
            _ => self.write_byte(pixels, b'?'),
        }
    }

    fn newline(&mut self, pixels: &mut [u32]) {
        self.col = 0;
        self.row += 1;
        if self.row >= SURFACE_ROWS {
            scroll(pixels);
            self.row = SURFACE_ROWS - 1;
        }
    }

    fn backspace(&mut self, pixels: &mut [u32]) {
        if self.col == 0 {
            return;
        }
        self.col -= 1;
        draw_cell(pixels, self.col, self.row, b' ');
    }
}

fn scroll(pixels: &mut [u32]) {
    pixels.copy_within(
        GLYPH_HEIGHT * SURFACE_WIDTH..SURFACE_HEIGHT * SURFACE_WIDTH,
        0,
    );
    pixels[(SURFACE_HEIGHT - GLYPH_HEIGHT) * SURFACE_WIDTH..SURFACE_HEIGHT * SURFACE_WIDTH]
        .fill(SURFACE_BG);
}

fn draw_cell(pixels: &mut [u32], cell_x: usize, cell_y: usize, byte: u8) {
    let x0 = cell_x * GLYPH_WIDTH;
    let y0 = cell_y * GLYPH_HEIGHT;
    for y in 0..GLYPH_HEIGHT {
        let row = (y0 + y) * SURFACE_WIDTH + x0;
        pixels[row..row + GLYPH_WIDTH].fill(SURFACE_BG);
    }
    for (glyph_y, bits) in glyph_rows(byte).iter().copied().enumerate() {
        for glyph_x in 0..5 {
            if bits & (1 << (4 - glyph_x)) == 0 {
                continue;
            }
            let x = x0 + 1 + glyph_x;
            let y = y0 + 1 + glyph_y * 2;
            pixels[y * SURFACE_WIDTH + x] = SURFACE_FG;
            pixels[(y + 1) * SURFACE_WIDTH + x] = SURFACE_FG;
        }
    }
}

/// DS-1c: whether a line asks for a listing of `/`, the directory the Lens
/// is lent. Chad DS-1c F2: parsed positively, not by rejecting slash paths —
/// the line (ASCII whitespace-separated words, any case, any spacing) must be
/// exactly one of the supported forms:
///
/// * `ls /`
/// * `list /`
/// * `list directory /`
/// * `list the directory /`
///
/// Anything else — another path in any form (`ls / tmp`, `list /
/// ../scratch`), extra words, a different verb — stays with the model, so a
/// command naming another target can never show `/` in its place.
pub fn wants_root_lens(line: &[u8]) -> bool {
    const FORMS: [&[&[u8]]; 4] = [
        &[b"ls", b"/"],
        &[b"list", b"/"],
        &[b"list", b"directory", b"/"],
        &[b"list", b"the", b"directory", b"/"],
    ];
    let mut words = [&[][..]; 4];
    let mut count = 0;
    for word in line
        .split(|b| b.is_ascii_whitespace())
        .filter(|w| !w.is_empty())
    {
        if count == words.len() {
            return false;
        }
        words[count] = word;
        count += 1;
    }
    FORMS.iter().any(|form| {
        form.len() == count
            && form
                .iter()
                .zip(&words[..count])
                .all(|(want, got)| want.eq_ignore_ascii_case(got))
    })
}

/// What the footer says about the model. The name only ever comes from the
/// model Arthur actually loaded (see [`model_display_name`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FooterModel {
    /// Created, before the vocab and model load.
    Loading,
    /// Loaded from this path's model, under this display name.
    Named(alloc::string::String),
    /// Loaded through an inherited handle: there is no path to name it from.
    Unnamed,
    /// No model, or one that is absent, damaged or failed to load.
    NoModel,
}

const FOOTER_RUNS_HERE: &str = " \u{b7} runs on this machine";
const FOOTER_BACK: &str = " \u{b7} Esc to go back";
const FOOTER_BACK_ALONE: &str = "Esc to go back";
const FOOTER_ELLIPSIS: &str = "...";

/// A loaded model's path as a display name:
/// `/boot/models/model-qwen2.5-0.5b.neom` is `Qwen2.5 0.5B`. The `.neom`
/// extension and a `model-` prefix go; `-` separates words; a size word
/// (`0.5b`, `135m`) gets a capital unit; the first letter is capitalised.
/// `None` when nothing is left to show.
pub fn model_display_name(path: &str) -> Option<alloc::string::String> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.strip_suffix(".neom").unwrap_or(file);
    let stem = stem.strip_prefix("model-").unwrap_or(stem);
    let mut name = alloc::string::String::new();
    for word in stem.split('-').filter(|w| !w.is_empty()) {
        if !name.is_empty() {
            name.push(' ');
        }
        if is_size_word(word) {
            name.push_str(&word[..word.len() - 1]);
            name.push(word.as_bytes()[word.len() - 1].to_ascii_uppercase() as char);
        } else if name.is_empty() {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                name.push(first.to_ascii_uppercase());
                name.push_str(chars.as_str());
            }
        } else {
            name.push_str(word);
        }
    }
    (!name.is_empty()).then_some(name)
}

/// `0.5b`, `1.5B`, `135m`: digits (and dots) then a `b` or `m` unit.
fn is_size_word(word: &str) -> bool {
    let bytes = word.as_bytes();
    let Some((&unit, number)) = bytes.split_last() else {
        return false;
    };
    matches!(unit, b'b' | b'B' | b'm' | b'M')
        && number.first().is_some_and(u8::is_ascii_digit)
        && number.iter().all(|&b| b.is_ascii_digit() || b == b'.')
}

/// The footer for `model`, the longest form that `fits` (the surface's
/// footer width). The model's name outlasts the other words: first
/// "runs on this machine" goes, then the name is shortened with `...`; only
/// when no name fits does the footer say just "Esc to go back".
pub fn footer_text(model: &FooterModel, fits: impl Fn(&str) -> bool) -> alloc::string::String {
    use alloc::string::String;
    let joined = |head: &str, tail: &[&str]| {
        let mut s = String::from(head);
        tail.iter().for_each(|t| s.push_str(t));
        s
    };
    let candidates: [String; 2] = match model {
        FooterModel::Loading => [joined("loading model", &[FOOTER_BACK]), String::new()],
        FooterModel::NoModel => [joined("no model loaded", &[FOOTER_BACK]), String::new()],
        FooterModel::Unnamed => [
            joined("model loaded", &[FOOTER_RUNS_HERE, FOOTER_BACK]),
            joined("model loaded", &[FOOTER_BACK]),
        ],
        FooterModel::Named(name) => [
            joined(name, &[FOOTER_RUNS_HERE, FOOTER_BACK]),
            joined(name, &[FOOTER_BACK]),
        ],
    };
    if let Some(text) = candidates.into_iter().find(|c| !c.is_empty() && fits(c)) {
        return text;
    }
    if let FooterModel::Named(name) = model {
        // Shorten the name a character at a time; at least one is kept.
        let mut cut = name.len();
        while let Some((i, _)) = name[..cut].char_indices().next_back() {
            if i == 0 {
                break;
            }
            cut = i;
            let text = joined(&name[..cut], &[FOOTER_ELLIPSIS, FOOTER_BACK]);
            if fits(&text) {
                return text;
            }
        }
    }
    String::from(FOOTER_BACK_ALONE)
}
