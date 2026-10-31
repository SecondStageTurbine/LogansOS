// SPDX-License-Identifier: MPL-2.0
//! XRGB8888 pixel target plus gamma-correct coverage blending.

use crate::font::GlyphBitmap;

include!(concat!(env!("OUT_DIR"), "/gamma_tables.rs"));

/// A pixel rectangle in buffer coordinates (half-open).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn right(&self) -> i32 {
        self.x + self.width
    }

    pub const fn bottom(&self) -> i32 {
        self.y + self.height
    }

    pub const fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    pub fn intersect(&self, other: &Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        Rect::new(x, y, (r - x).max(0), (b - y).max(0))
    }

    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let r = self.right().max(other.right());
        let b = self.bottom().max(other.bottom());
        Rect::new(x, y, r - x, b - y)
    }
}

/// Blend one channel: `fg` over `bg` with 8-bit `alpha`, in linear light.
pub fn blend_channel(bg: u8, fg: u8, alpha: u8) -> u8 {
    let a = u32::from(alpha);
    let lb = u32::from(SRGB_TO_LINEAR[bg as usize]);
    let lf = u32::from(SRGB_TO_LINEAR[fg as usize]);
    let linear = (lb * (255 - a) + lf * a + 127) / 255;
    LINEAR_TO_SRGB[linear as usize]
}

/// Gamma-correct blend of XRGB8888 `fg` over `bg` at coverage `alpha`. The X
/// byte of the result is zero.
pub fn blend_pixel(bg: u32, fg: u32, alpha: u8) -> u32 {
    match alpha {
        0 => bg & 0x00FF_FFFF,
        255 => fg & 0x00FF_FFFF,
        _ => {
            let ch = |shift: u32| {
                u32::from(blend_channel(
                    (bg >> shift) as u8,
                    (fg >> shift) as u8,
                    alpha,
                )) << shift
            };
            ch(16) | ch(8) | ch(0)
        }
    }
}

/// A borrowed XRGB8888 surface. `stride` is in pixels.
pub struct Canvas<'p> {
    pixels: &'p mut [u32],
    width: usize,
    height: usize,
    stride: usize,
}

impl<'p> Canvas<'p> {
    /// `None` when `pixels` is too small for the stated geometry.
    pub fn new(pixels: &'p mut [u32], width: usize, height: usize, stride: usize) -> Option<Self> {
        let needed = if height == 0 {
            0
        } else {
            stride.checked_mul(height - 1)?.checked_add(width)?
        };
        (stride >= width && pixels.len() >= needed).then_some(Self {
            pixels,
            width,
            height,
            stride,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width as i32, self.height as i32)
    }

    pub fn pixel(&self, x: usize, y: usize) -> u32 {
        self.pixels[y * self.stride + x]
    }

    pub fn fill(&mut self, rect: Rect, color: u32) {
        let r = rect.intersect(&self.bounds());
        for y in r.y..r.bottom() {
            let row = y as usize * self.stride;
            for x in r.x..r.right() {
                self.pixels[row + x as usize] = color;
            }
        }
    }

    /// Composite `glyph` in `color` with its origin at (`x`, `baseline`),
    /// writing only inside `clip`.
    pub fn draw_glyph(
        &mut self,
        glyph: &GlyphBitmap,
        x: i32,
        baseline: i32,
        color: u32,
        clip: Rect,
    ) {
        let clip = clip.intersect(&self.bounds());
        let gx = x + glyph.left;
        let gy = baseline + glyph.top;
        let r = Rect::new(gx, gy, glyph.width as i32, glyph.height as i32).intersect(&clip);
        for y in r.y..r.bottom() {
            let src = (y - gy) as usize * glyph.width;
            let row = y as usize * self.stride;
            for x in r.x..r.right() {
                let alpha = glyph.coverage[src + (x - gx) as usize];
                if alpha != 0 {
                    let px = &mut self.pixels[row + x as usize];
                    *px = blend_pixel(*px, color, alpha);
                }
            }
        }
    }

    /// Move the pixels of `rect` up by `dy` rows and fill the vacated band.
    pub fn scroll_up(&mut self, rect: Rect, dy: i32, fill: u32) {
        let r = rect.intersect(&self.bounds());
        if r.is_empty() || dy <= 0 {
            return;
        }
        let dy = dy.min(r.height);
        for y in r.y..r.bottom() - dy {
            let dst = y as usize * self.stride + r.x as usize;
            let src = (y + dy) as usize * self.stride + r.x as usize;
            self.pixels.copy_within(src..src + r.width as usize, dst);
        }
        self.fill(Rect::new(r.x, r.bottom() - dy, r.width, dy), fill);
    }
}
