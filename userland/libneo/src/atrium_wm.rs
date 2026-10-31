// SPDX-License-Identifier: MPL-2.0
//! Userland Atrium window-manager scene composition helpers.
//!
//! DW-1 keeps WM policy in trusted userland: the compositor owns the output
//! buffer, paints the Home/Spine root, and then copies client surfaces in
//! WM-chosen stacking order. Clients never provide position or peer identity.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneError {
    BadGeometry,
    BufferTooSmall,
    SurfaceOutOfBounds,
    /// The first layer is not an opaque layer covering the whole scene.
    NoBase,
}

#[derive(Clone, Copy, Debug)]
pub struct DesktopRoot {
    pub background: u32,
    pub spine: u32,
    pub spine_width: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct SceneSurface<'a> {
    pub pixels: &'a [u32],
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub x: usize,
    pub y: usize,
}

pub fn compose_desktop_scene_xrgb8888(
    dst: &mut [u32],
    width: usize,
    height: usize,
    stride: usize,
    root: DesktopRoot,
    surfaces: &[SceneSurface<'_>],
) -> Result<(), SceneError> {
    if width == 0 || height == 0 || stride < width {
        return Err(SceneError::BadGeometry);
    }
    let required = required_len(width, height, stride)?;
    if dst.len() < required {
        return Err(SceneError::BufferTooSmall);
    }

    for y in 0..height {
        let row = y * stride;
        for x in 0..width {
            dst[row + x] = if x < root.spine_width {
                root.spine
            } else {
                root.background
            };
        }
    }

    for surface in surfaces {
        validate_surface(width, height, surface)?;
        for sy in 0..surface.height {
            let src_row = sy * surface.stride;
            let dst_row = (surface.y + sy) * stride + surface.x;
            let src = &surface.pixels[src_row..src_row + surface.width];
            dst[dst_row..dst_row + surface.width].copy_from_slice(src);
        }
    }

    Ok(())
}

fn validate_surface(
    scene_width: usize,
    scene_height: usize,
    surface: &SceneSurface<'_>,
) -> Result<(), SceneError> {
    if surface.width == 0 || surface.height == 0 || surface.stride < surface.width {
        return Err(SceneError::BadGeometry);
    }
    let required = required_len(surface.width, surface.height, surface.stride)?;
    if surface.pixels.len() < required {
        return Err(SceneError::BufferTooSmall);
    }
    let x_end = surface
        .x
        .checked_add(surface.width)
        .ok_or(SceneError::SurfaceOutOfBounds)?;
    let y_end = surface
        .y
        .checked_add(surface.height)
        .ok_or(SceneError::SurfaceOutOfBounds)?;
    if x_end > scene_width || y_end > scene_height {
        return Err(SceneError::SurfaceOutOfBounds);
    }
    Ok(())
}

const fn required_len(width: usize, height: usize, stride: usize) -> Result<usize, SceneError> {
    let Some(last_row) = height.checked_sub(1) else {
        return Err(SceneError::BadGeometry);
    };
    let Some(prefix) = last_row.checked_mul(stride) else {
        return Err(SceneError::BufferTooSmall);
    };
    let Some(total) = prefix.checked_add(width) else {
        return Err(SceneError::BufferTooSmall);
    };
    Ok(total)
}

// ---------------------------------------------------------------------------
// S1 desktop session: layered, damage-bounded, alpha-aware composition.
//
// `compose_desktop_scene_xrgb8888` above is kept byte-for-byte for DW-1/DW-2
// (their stacking oracles pin it). The session compositor below is the
// generalisation the full-screen desktop needs:
//   * the FIRST layer is the WM-owned opaque base and must cover the whole
//     scene, so no frame can ever leave stale pixels from an earlier owner;
//   * only the damage rectangle is recomposed (clipped to the scene);
//   * layers are either `Opaque` (row copies) or `Alpha` (straight alpha in
//     bits 31..24, blended with `blend_over`).
// ---------------------------------------------------------------------------

/// An axis-aligned rectangle in scene pixels. Empty when `w == 0 || h == 0`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Rect {
    pub const fn new(x: usize, y: usize, w: usize, h: usize) -> Self {
        Self { x, y, w, h }
    }

    pub const fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub const fn right(&self) -> usize {
        self.x.saturating_add(self.w)
    }

    pub const fn bottom(&self) -> usize {
        self.y.saturating_add(self.h)
    }

    pub const fn contains(&self, x: usize, y: usize) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    /// Intersection; an empty rect at the origin when they do not overlap.
    pub fn intersect(&self, other: &Rect) -> Rect {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x1 <= x0 || y1 <= y0 {
            Rect::default()
        } else {
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        }
    }

    /// Smallest rect covering both. An empty operand is ignored.
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = self.right().max(other.right());
        let y1 = self.bottom().max(other.bottom());
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// `true` when `other` lies entirely inside `self`.
    pub fn covers(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }
}

/// A compositor's read-only view of a client's shared pixel buffer.
///
/// The client keeps its read-write mapping and can write the same pages at
/// any moment: "write, then commit" binds a well-behaved client, not a buggy
/// or hostile one. So the WM never forms a Rust reference (`&[u32]`) over
/// them — that would promise the compiler the memory cannot change. Every
/// read here is a relaxed atomic load (an `AtomicU32` is a `u32` behind an
/// `UnsafeCell`, the type for memory others may write), and pixels are copied
/// into WM-owned storage, the only thing a [`Layer`] ever points at. A
/// concurrent writer can at most make the copy mix its old and new pixels:
/// its own surface's appearance, never the WM's memory or control flow.
pub struct SharedPixels {
    base: *const core::sync::atomic::AtomicU32,
    width: usize,
    height: usize,
}

impl SharedPixels {
    /// # Safety
    ///
    /// `base` must point to `width * height` tightly packed, 4-byte aligned
    /// pixels that stay mapped for the life of the view (the kernel pins a
    /// mapped buffer; there is no unmap). Others may write them at any time.
    pub const unsafe fn new(base: *const u32, width: usize, height: usize) -> Self {
        Self {
            base: base as *const core::sync::atomic::AtomicU32,
            width,
            height,
        }
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    /// One pixel of the surface, read now (`None` outside it): what a
    /// witness compares a composite against, independent of any copy.
    pub fn load(&self, x: usize, y: usize) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        // SAFETY: `y * width + x < width * height`, which `new`'s contract
        // keeps mapped and aligned.
        let pixel = unsafe { &*self.base.add(y * self.width + x) };
        Some(pixel.load(core::sync::atomic::Ordering::Relaxed))
    }

    /// Copy `rect` (surface coordinates, clipped to the surface) into the
    /// same place of `dst`, a WM-owned copy of the surface laid out with
    /// stride `width` (exactly like the source). Returns the clipped rect;
    /// empty when `dst` is too small. The clip, not the protocol, is what
    /// bounds a client-named rect.
    pub fn snapshot(&self, rect: Rect, dst: &mut [u32]) -> Rect {
        let r = rect.intersect(&Rect::new(0, 0, self.width, self.height));
        if r.is_empty() || dst.len() < self.width * self.height {
            return Rect::default();
        }
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                let i = y * self.width + x;
                // SAFETY: `i < width * height` (r is clipped to the surface),
                // which `new`'s contract keeps mapped and aligned.
                let pixel = unsafe { &*self.base.add(i) };
                dst[i] = pixel.load(core::sync::atomic::Ordering::Relaxed);
            }
        }
        r
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Blend {
    /// Copy the source pixel; its top byte is ignored (XRGB8888).
    Opaque,
    /// Straight (non-premultiplied) alpha in bits 31..24, see [`blend_over`].
    Alpha,
}

#[derive(Clone, Copy, Debug)]
pub struct Layer<'a> {
    pub pixels: &'a [u32],
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub x: usize,
    pub y: usize,
    pub blend: Blend,
}

impl Layer<'_> {
    pub const fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }
}

/// Source-over for one channel set, straight alpha, round-to-nearest:
/// `c = (s*a + d*(255-a) + 127) / 255` per 8-bit channel. The result's top
/// byte is always 0 (XRGB8888 scanout). `a == 255` is an exact copy of the
/// source colour and `a == 0` an exact copy of the destination colour.
pub const fn blend_over(dst: u32, src: u32) -> u32 {
    let a = src >> 24;
    let inv = 255 - a;
    let mut out = 0u32;
    let mut shift = 0;
    while shift < 24 {
        let s = (src >> shift) & 0xFF;
        let d = (dst >> shift) & 0xFF;
        let c = (s * a + d * inv + 127) / 255;
        out |= c << shift;
        shift += 8;
    }
    out
}

/// Recompose `damage` (clipped to the scene) from `layers`, back to front.
/// `layers[0]` must be an opaque layer covering the entire scene — the
/// WM-owned base — so every damaged pixel is rewritten every frame. Returns
/// the clipped damage that was actually written (what the caller commits).
pub fn compose_layers_xrgb8888(
    dst: &mut [u32],
    width: usize,
    height: usize,
    stride: usize,
    layers: &[Layer<'_>],
    damage: Rect,
) -> Result<Rect, SceneError> {
    if width == 0 || height == 0 || stride < width {
        return Err(SceneError::BadGeometry);
    }
    let required = required_len(width, height, stride)?;
    if dst.len() < required {
        return Err(SceneError::BufferTooSmall);
    }
    let scene = Rect::new(0, 0, width, height);
    let Some(base) = layers.first() else {
        return Err(SceneError::NoBase);
    };
    if base.blend != Blend::Opaque || !base.rect().covers(&scene) {
        return Err(SceneError::NoBase);
    }
    for layer in layers {
        validate_layer(width, height, layer)?;
    }
    let damage = damage.intersect(&scene);
    if damage.is_empty() {
        return Ok(damage);
    }

    for layer in layers {
        let area = layer.rect().intersect(&damage);
        if area.is_empty() {
            continue;
        }
        let sx = area.x - layer.x;
        for row in 0..area.h {
            let sy = area.y + row - layer.y;
            let src_start = sy * layer.stride + sx;
            let src = &layer.pixels[src_start..src_start + area.w];
            let dst_start = (area.y + row) * stride + area.x;
            let out = &mut dst[dst_start..dst_start + area.w];
            match layer.blend {
                Blend::Opaque => out.copy_from_slice(src),
                Blend::Alpha => {
                    for (d, &s) in out.iter_mut().zip(src) {
                        match s >> 24 {
                            0 => {}
                            255 => *d = s & 0x00FF_FFFF,
                            _ => *d = blend_over(*d, s),
                        }
                    }
                }
            }
        }
    }
    Ok(damage)
}

fn validate_layer(
    scene_width: usize,
    scene_height: usize,
    layer: &Layer<'_>,
) -> Result<(), SceneError> {
    validate_surface(
        scene_width,
        scene_height,
        &SceneSurface {
            pixels: layer.pixels,
            width: layer.width,
            height: layer.height,
            stride: layer.stride,
            x: layer.x,
            y: layer.y,
        },
    )
}
