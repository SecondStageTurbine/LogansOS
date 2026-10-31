// SPDX-License-Identifier: MPL-2.0
//! A parsed TrueType face: glyph lookup, metrics, GPOS kerning and outline
//! rasterization into [`GlyphBitmap`]s, with a per-face glyph cache.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
use ttf_parser::{Face, GlyphId, OutlineBuilder, Tag};

use crate::raster::{Point, Rasterizer, ceil, floor};

/// Horizontal subpixel positions per pixel. Glyph origins are quantised to
/// 1/SUBPIXEL_BINS px, so each glyph is cached at most this many times per size.
pub const SUBPIXEL_BINS: u8 = 4;

/// Sizes below this many pixels get stem darkening (design language §6.1,
/// text-engine trap R4): thin AA stems otherwise read grey and fuzzy.
pub const STEM_DARKENING_BELOW_PX: f32 = 16.0;

/// One rendered glyph: row-major 8-bit coverage, positioned relative to the
/// glyph origin on the baseline (`left` right, `top` down from the baseline;
/// `top` is negative for ink above the baseline).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphBitmap {
    pub left: i32,
    pub top: i32,
    pub width: usize,
    pub height: usize,
    pub coverage: Vec<u8>,
}

/// Vertical metrics in pixels at a given size (hhea; y grows downward, so
/// `descent` is positive).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CacheKey {
    glyph: u16,
    /// Size in 1/64 px.
    size_64: u32,
    bin: u8,
}

pub struct Font<'a> {
    face: Face<'a>,
    units_per_em: f32,
    kern_lookups: Vec<u16>,
    cache: BTreeMap<CacheKey, Option<GlyphBitmap>>,
}

impl<'a> Font<'a> {
    pub fn from_bytes(data: &'a [u8]) -> Option<Self> {
        let face = Face::parse(data, 0).ok()?;
        let units_per_em = face.units_per_em() as f32;
        let mut kern_lookups = Vec::new();
        if let Some(gpos) = face.tables().gpos {
            for feature in gpos.features {
                if feature.tag == Tag::from_bytes(b"kern") {
                    for index in feature.lookup_indices {
                        if !kern_lookups.contains(&index) {
                            kern_lookups.push(index);
                        }
                    }
                }
            }
        }
        kern_lookups.sort_unstable();
        Some(Self {
            face,
            units_per_em,
            kern_lookups,
            cache: BTreeMap::new(),
        })
    }

    pub fn units_per_em(&self) -> f32 {
        self.units_per_em
    }

    /// Pixels per font unit at `px` (px = ppem, the em square in pixels).
    pub fn scale(&self, px: f32) -> f32 {
        px / self.units_per_em
    }

    /// Glyph for `c`, or glyph 0 (.notdef) when the face lacks it.
    pub fn glyph_id(&self, c: char) -> u16 {
        self.face.glyph_index(c).map_or(0, |g| g.0)
    }

    pub fn advance_units(&self, glyph: u16) -> u16 {
        self.face.glyph_hor_advance(GlyphId(glyph)).unwrap_or(0)
    }

    pub fn advance(&self, glyph: u16, px: f32) -> f32 {
        self.advance_units(glyph) as f32 * self.scale(px)
    }

    pub fn line_metrics(&self, px: f32) -> LineMetrics {
        let s = self.scale(px);
        LineMetrics {
            ascent: self.face.ascender() as f32 * s,
            descent: -(self.face.descender() as f32) * s,
            line_gap: self.face.line_gap() as f32 * s,
        }
    }

    /// GPOS `kern` x-advance adjustment for the ordered pair, in font units.
    ///
    /// Every lookup reachable from a `kern` feature is applied (Latin needs no
    /// script/language selection); within a lookup the first subtable that
    /// *matches* decides: Format 1 only when the exact pair is listed, Format 2
    /// on coverage of `left` unless `right` is class 0 with a zero record (the
    /// HarfBuzz behaviour split class-kerning subtables depend on).
    pub fn kerning_units(&self, left: u16, right: u16) -> i32 {
        let Some(gpos) = self.face.tables().gpos else {
            return 0;
        };
        let (left, right) = (GlyphId(left), GlyphId(right));
        let mut total = 0i32;
        for &index in &self.kern_lookups {
            let Some(lookup) = gpos.lookups.get(index) else {
                continue;
            };
            for sub in 0..lookup.subtables.len() {
                let Some(PositioningSubtable::Pair(pair)) =
                    lookup.subtables.get::<PositioningSubtable>(sub)
                else {
                    continue;
                };
                let value = match pair {
                    // A Format-1 subtable only matches when the exact pair is
                    // listed; a miss falls through to the next subtable (Plex
                    // puts class kerning in Format-2 subtables after it).
                    PairAdjustment::Format1 { coverage, sets } => {
                        let Some(i) = coverage.get(left) else {
                            continue;
                        };
                        let Some((first, _)) = sets.get(i).and_then(|set| set.get(right)) else {
                            continue;
                        };
                        Some(first.x_advance)
                    }
                    PairAdjustment::Format2 {
                        coverage,
                        classes,
                        matrix,
                    } => {
                        if coverage.get(left).is_none() {
                            continue;
                        }
                        let class2 = classes.1.get(right);
                        let value = matrix
                            .get((classes.0.get(left), class2))
                            .map_or(0, |(first, _)| first.x_advance);
                        // An unclassified right glyph with a zero record falls
                        // through, as HarfBuzz does: compilers split class
                        // kerning across subtables and rely on it (Plex `jV`).
                        if class2 == 0 && value == 0 {
                            continue;
                        }
                        Some(value)
                    }
                };
                total += i32::from(value.unwrap_or(0));
                break;
            }
        }
        total
    }

    pub fn kerning(&self, left: u16, right: u16, px: f32) -> f32 {
        self.kerning_units(left, right) as f32 * self.scale(px)
    }

    /// Cached rasterization of `glyph` at `px` with the origin shifted right by
    /// `bin / SUBPIXEL_BINS` px. `None` for glyphs without ink (space).
    pub fn glyph(&mut self, glyph: u16, px: f32, bin: u8) -> Option<&GlyphBitmap> {
        let key = CacheKey {
            glyph,
            size_64: (px * 64.0 + 0.5) as u32,
            bin: bin % SUBPIXEL_BINS,
        };
        if !self.cache.contains_key(&key) {
            let mut bitmap =
                self.rasterize(glyph, px, f32::from(key.bin) / f32::from(SUBPIXEL_BINS));
            if let Some(bitmap) = bitmap.as_mut() {
                darken_stems(&mut bitmap.coverage, px);
            }
            self.cache.insert(key, bitmap);
        }
        self.cache.get(&key).and_then(Option::as_ref)
    }

    /// Uncached, undarkened rasterization (the raw exact-area coverage that the
    /// byte oracles assert against).
    pub fn rasterize(&self, glyph: u16, px: f32, x_offset: f32) -> Option<GlyphBitmap> {
        let bbox = self.face.glyph_bounding_box(GlyphId(glyph))?;
        let s = self.scale(px);
        let left = floor(bbox.x_min as f32 * s + x_offset);
        let right = ceil(bbox.x_max as f32 * s + x_offset);
        let top = floor(-(bbox.y_max as f32) * s);
        let bottom = ceil(-(bbox.y_min as f32) * s);
        let width = (right - left) as usize;
        let height = (bottom - top) as usize;
        if width == 0 || height == 0 {
            return None;
        }
        let mut builder = Builder {
            raster: Rasterizer::new(width, height),
            scale: s,
            dx: x_offset - left,
            dy: -top,
            start: Point::new(0.0, 0.0),
            last: Point::new(0.0, 0.0),
        };
        self.face.outline_glyph(GlyphId(glyph), &mut builder)?;
        builder.close();
        Some(GlyphBitmap {
            left: left as i32,
            top: top as i32,
            width,
            height,
            coverage: builder.raster.coverage(),
        })
    }
}

/// Stem darkening for small sizes, as a coverage curve: `c' = c^(1/(1+k))`
/// with `k` growing linearly from 0 at [`STEM_DARKENING_BELOW_PX`] to 0.3 at
/// 8 px. Fully-off and fully-on pixels are fixed points; partial edge pixels
/// are pushed toward ink, which thickens thin stems by a fraction of a pixel.
/// This is the cheap coverage-domain approximation of FreeType's outline
/// emboldening; sizes >= 16 px are untouched (bit-identical to raw coverage).
pub fn darken_stems(coverage: &mut [u8], px: f32) {
    if px >= STEM_DARKENING_BELOW_PX {
        return;
    }
    let k = ((STEM_DARKENING_BELOW_PX - px) / 8.0).min(1.0) * 0.3;
    let exponent = 1.0 / (1.0 + k);
    let mut lut = [0u8; 256];
    for (c, out) in lut.iter_mut().enumerate() {
        *out = (libm::powf(c as f32 / 255.0, exponent) * 255.0 + 0.5) as u8;
    }
    for c in coverage {
        *c = lut[*c as usize];
    }
}

struct Builder {
    raster: Rasterizer,
    scale: f32,
    dx: f32,
    dy: f32,
    start: Point,
    last: Point,
}

impl Builder {
    fn map(&self, x: f32, y: f32) -> Point {
        Point::new(x * self.scale + self.dx, -y * self.scale + self.dy)
    }
}

impl OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        let p = self.map(x, y);
        self.start = p;
        self.last = p;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.raster.line(self.last, p);
        self.last = p;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let c = self.map(x1, y1);
        let p = self.map(x, y);
        self.raster.quad(self.last, c, p);
        self.last = p;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let c1 = self.map(x1, y1);
        let c2 = self.map(x2, y2);
        let p = self.map(x, y);
        self.raster.cubic(self.last, c1, c2, p);
        self.last = p;
    }

    fn close(&mut self) {
        if self.last != self.start {
            self.raster.line(self.last, self.start);
        }
        self.last = self.start;
    }
}
