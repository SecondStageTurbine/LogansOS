// SPDX-License-Identifier: MPL-2.0
//! A streaming text region: characters arrive one at a time (keystrokes, model
//! tokens) and are laid out proportionally with word wrap, backspace and
//! scrolling. Every pixel change is accumulated into a damage rectangle so the
//! client commits only what moved ("real glyph -> buffer -> damage rects").

use alloc::collections::VecDeque;
use alloc::string::String;

use crate::canvas::{Canvas, Rect};
use crate::font::Font;
use crate::layout::{draw_line, line_break, measure};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowStyle {
    pub px: f32,
    pub line_height: i32,
    /// Baseline distance from the top of each line band.
    pub baseline: i32,
    pub fg: u32,
    pub bg: u32,
}

pub struct TextFlow {
    region: Rect,
    style: FlowStyle,
    line: String,
    row: i32,
    rows: i32,
    damage: Rect,
    /// A9-C: the finished visual lines (newest last), kept only by
    /// [`Self::with_scrollback`] flows.
    scrollback: Option<Scrollback>,
}

/// The lines a flow has finished (by a newline or a wrap), oldest first, at
/// most `cap`; `dropped` counts the ones let go, so a line keeps one absolute
/// index for its whole life.
struct Scrollback {
    lines: VecDeque<String>,
    cap: usize,
    dropped: u64,
}

impl TextFlow {
    pub fn new(region: Rect, style: FlowStyle) -> Self {
        let rows = (region.height / style.line_height.max(1)).max(1);
        Self {
            region,
            style,
            line: String::new(),
            row: 0,
            rows,
            damage: Rect::new(0, 0, 0, 0),
            scrollback: None,
        }
    }

    /// A9-C: a flow that also keeps its last `cap` finished lines, so a view
    /// of earlier text can be drawn again ([`Self::render_view`]). It draws
    /// exactly as [`Self::new`]'s does.
    pub fn with_scrollback(region: Rect, style: FlowStyle, cap: usize) -> Self {
        let mut flow = Self::new(region, style);
        flow.scrollback = Some(Scrollback {
            lines: VecDeque::new(),
            cap: cap.max(1),
            dropped: 0,
        });
        flow
    }

    /// The absolute index of the current line: the number of lines finished
    /// so far, kept or let go. The text runs from the oldest kept line
    /// ([`Self::first_kept`]) to this one.
    pub fn current_index(&self) -> u64 {
        self.scrollback
            .as_ref()
            .map_or(0, |s| s.dropped + s.lines.len() as u64)
    }

    /// The absolute index of the oldest finished line still kept.
    pub fn first_kept(&self) -> u64 {
        self.scrollback.as_ref().map_or(0, |s| s.dropped)
    }

    /// The index of the line the bottom-following view shows in its top
    /// row: what the flow itself has on screen.
    pub fn bottom_top(&self) -> u64 {
        self.current_index().saturating_sub(self.row as u64)
    }

    /// Draw the region as a view whose top row shows line `top` (absolute;
    /// clamped to the kept lines): kept finished lines, then the current
    /// line, then empty rows. At [`Self::bottom_top`] this is pixel-for-pixel
    /// what the flow drew itself. Damages the whole region.
    pub fn render_view(&mut self, font: &mut Font<'_>, canvas: &mut Canvas<'_>, top: u64) {
        let used = Rect::new(
            self.region.x,
            self.region.y,
            self.region.width,
            self.rows * self.style.line_height,
        );
        canvas.fill(used, self.style.bg);
        let top = top.max(self.first_kept());
        let current = self.current_index();
        for row in 0..self.rows {
            let index = top + row as u64;
            let text = if index == current {
                self.line.as_str()
            } else {
                match self.scrollback.as_ref().and_then(|s| {
                    let at = index.checked_sub(s.dropped)? as usize;
                    s.lines.get(at)
                }) {
                    Some(line) => line.as_str(),
                    None => continue,
                }
            };
            let band = Rect::new(
                self.region.x,
                self.region.y + row * self.style.line_height,
                self.region.width,
                self.style.line_height,
            );
            draw_line(
                canvas,
                font,
                self.style.px,
                text,
                self.region.x as f32,
                band.y + self.style.baseline,
                self.style.fg,
                band,
            );
        }
        self.damage = self.damage.union(&used);
    }

    pub fn region(&self) -> Rect {
        self.region
    }

    pub fn row(&self) -> i32 {
        self.row
    }

    pub fn rows(&self) -> i32 {
        self.rows
    }

    /// Text of the line currently being written.
    pub fn current_line(&self) -> &str {
        &self.line
    }

    pub fn clear(&mut self, canvas: &mut Canvas<'_>) {
        canvas.fill(self.region, self.style.bg);
        self.line.clear();
        self.row = 0;
        if let Some(s) = self.scrollback.as_mut() {
            s.lines.clear();
            s.dropped = 0;
        }
        self.damage = self.damage.union(&self.region);
    }

    /// Append one character. `\n` breaks the line, `\x08`/`\x7f` erase the last
    /// character of the current line (a no-op at its start, like the legacy
    /// grid), `\r` is ignored and other control characters render as `?`.
    pub fn push(&mut self, font: &mut Font<'_>, canvas: &mut Canvas<'_>, c: char) {
        match c {
            '\r' => {}
            '\n' => self.newline(canvas),
            '\x08' | '\x7f' => {
                if self.line.pop().is_some() {
                    self.redraw_line(font, canvas);
                }
            }
            '\t' => {
                for _ in 0..4 {
                    self.push(font, canvas, ' ');
                }
            }
            c if c.is_control() => self.push(font, canvas, '?'),
            c => {
                self.line.push(c);
                let width = self.region.width as f32;
                if c != ' ' && measure(font, self.style.px, &self.line) > width {
                    let (end, next) =
                        line_break(font, self.style.px, &self.line, 0..self.line.len(), width);
                    let carry = String::from(&self.line[next..]);
                    self.line.truncate(end);
                    self.redraw_line(font, canvas);
                    self.newline(canvas);
                    self.line = carry;
                }
                self.redraw_line(font, canvas);
            }
        }
    }

    pub fn push_str(&mut self, font: &mut Font<'_>, canvas: &mut Canvas<'_>, text: &str) {
        for c in text.chars() {
            self.push(font, canvas, c);
        }
    }

    /// Where the next character goes: its x, and the top of the current
    /// line's band.
    pub fn caret(&self, font: &Font<'_>) -> (i32, i32) {
        let x = measure(font, self.style.px, &self.line);
        (
            self.region.x + libm::ceilf(x) as i32,
            self.region.y + self.row * self.style.line_height,
        )
    }

    /// Keep `lines` empty rows below the current one: scroll the region up
    /// (the current line with it) until they fit, or the current line is
    /// the top row. The rows below the current one are always empty, so only
    /// rows above it scroll away, as they would when the text reached them.
    pub fn make_room(&mut self, canvas: &mut Canvas<'_>, lines: i32) {
        let lines = lines.clamp(0, self.rows - 1);
        let short = self.row + lines - (self.rows - 1);
        if short <= 0 {
            return;
        }
        let used = Rect::new(
            self.region.x,
            self.region.y,
            self.region.width,
            self.rows * self.style.line_height,
        );
        canvas.scroll_up(used, short * self.style.line_height, self.style.bg);
        self.row -= short;
        self.damage = self.damage.union(&used);
    }

    /// Damage accumulated since the last call, if any.
    pub fn take_damage(&mut self) -> Option<Rect> {
        let damage = core::mem::replace(&mut self.damage, Rect::new(0, 0, 0, 0));
        (!damage.is_empty()).then_some(damage)
    }

    fn band(&self) -> Rect {
        Rect::new(
            self.region.x,
            self.region.y + self.row * self.style.line_height,
            self.region.width,
            self.style.line_height,
        )
    }

    fn newline(&mut self, canvas: &mut Canvas<'_>) {
        if let Some(s) = self.scrollback.as_mut() {
            if s.lines.len() == s.cap {
                s.lines.pop_front();
                s.dropped += 1;
            }
            s.lines.push_back(self.line.clone());
        }
        self.line.clear();
        self.row += 1;
        if self.row >= self.rows {
            let used = Rect::new(
                self.region.x,
                self.region.y,
                self.region.width,
                self.rows * self.style.line_height,
            );
            canvas.scroll_up(used, self.style.line_height, self.style.bg);
            self.row = self.rows - 1;
            self.damage = self.damage.union(&used);
        }
    }

    fn redraw_line(&mut self, font: &mut Font<'_>, canvas: &mut Canvas<'_>) {
        let band = self.band();
        canvas.fill(band, self.style.bg);
        draw_line(
            canvas,
            font,
            self.style.px,
            &self.line,
            self.region.x as f32,
            band.y + self.style.baseline,
            self.style.fg,
            band,
        );
        self.damage = self.damage.union(&band);
    }
}
