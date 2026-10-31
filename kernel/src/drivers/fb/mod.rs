// SPDX-License-Identifier: MPL-2.0
//! Linear framebuffer driver.
//!
//! Limine provides a pre-configured linear framebuffer, typically 32-bit
//! BGRA. We expose simple primitives: clear, plot pixel, draw rect, draw
//! glyph, draw string. Everything is software-rendered; this is fine for
//! the cyberpunk flat-color aesthetic.
//!
//! This driver is **in-kernel** for Phase 0/1 only. Phase 2 will replace
//! it with a userspace driver communicating via a capability-gated IPC
//! port. The `// PHASE-2-MIGRATE` markers indicate code that will leave
//! the kernel.
//!
//! Concurrency: a single spinlock guards the framebuffer. There is no
//! tearing protection beyond that; double-buffering arrives with the
//! userspace compositor in Phase 3.

use core::ptr::NonNull;
use core::slice;
use limine::framebuffer::Framebuffer;
use spin::Mutex;

#[cfg(feature = "framebuffer-console")]
pub mod console;
#[cfg(feature = "framebuffer-console")]
pub mod console_logic;
pub(crate) mod font;
#[cfg(feature = "fb-progress-console")]
pub mod progress;
#[cfg(feature = "fb-progress-console")]
pub mod progress_logic;

/// 32-bit packed pixel in the framebuffer's native byte order.
/// On all known Limine targets this is BGRA8888 (B in the lowest byte).
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Color(pub u32);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        // BGRA layout: blue is in the low byte, alpha in the high byte.
        Color(((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }

    // === LogansOS palette ===
    // Sampled from the LogansOS identity — a blue lens-flare star inside broken
    // silver ring arcs on a void-black field. Electric Blue is the primary
    // accent (the flare glow); Silver is the structural / wordmark / secondary
    // tone (the ring arcs); Magenta is demoted to alert/fatal use ONLY. Lime and
    // Red keep their functional pass/fail semantics. These are the canonical
    // boot-render colors and will move into a theme module once we have
    // configurable themes.
    pub const BLACK: Color = Color::rgb(0x05, 0x07, 0x0a); // Void background
    pub const ELECTRIC_BLUE: Color = Color::rgb(0x3c, 0x8e, 0xf4); // primary accent (flare glow)
    pub const SILVER: Color = Color::rgb(0xa8, 0xad, 0xb2); // structure / wordmark / secondary
    pub const DIM_BLUE: Color = Color::rgb(0x16, 0x38, 0x60); // dim electric-blue rules / feed
    pub const TEXT: Color = Color::rgb(0xce, 0xd3, 0xda); // neutral silver-white body text
    pub const MUTED: Color = Color::rgb(0x5b, 0x62, 0x6b); // muted metadata grey
    pub const ALERT_MAGENTA: Color = Color::rgb(0xff, 0x29, 0xa8); // alert / fatal ONLY (panic bar)
    #[cfg(feature = "neo-test")]
    pub const WHITE: Color = Color::rgb(0xff, 0xff, 0xff);
    #[cfg(feature = "neo-test")]
    pub const DARK_BLUE: Color = Color::rgb(0x05, 0x18, 0x45);
    #[cfg(feature = "neo-test")]
    pub const PASS_GREEN: Color = Color::rgb(0x08, 0x7f, 0x3b); // Lime (functional: pass)
    #[cfg(feature = "neo-test")]
    pub const FAIL_RED: Color = Color::rgb(0xa6, 0x12, 0x22); // functional: fail
}

#[cfg(feature = "neo-test")]
pub const GLYPH_WIDTH: usize = font::GLYPH_WIDTH;
#[cfg(feature = "neo-test")]
pub const GLYPH_HEIGHT: usize = font::GLYPH_HEIGHT;

/// State for the active framebuffer. Held inside `FB` once `init` runs.
struct FbState {
    /// Base pointer to framebuffer pixels. Wrapped in NonNull because
    /// it is never null after init.
    base: NonNull<u32>,
    width: usize,
    height: usize,
    /// Pitch in bytes (NOT pixels). May exceed `width * 4` due to padding.
    pitch_bytes: usize,
    /// Bytes per pixel. We assume 4; we'll panic in `init` if not.
    bpp: usize,
}

// SAFETY: We are the only owner; access is serialized through the Mutex.
unsafe impl Send for FbState {}

static FB: Mutex<Option<FbState>> = Mutex::new(None);

/// Initialize the framebuffer module from a Limine framebuffer response.
///
/// # Safety
/// The caller must ensure `framebuffer` describes a valid, exclusively-owned
/// framebuffer that lives for the rest of the kernel's lifetime. Limine's
/// guarantees satisfy this when called once during boot.
pub unsafe fn init(framebuffer: &Framebuffer) {
    let bpp = framebuffer.bpp as usize;
    assert!(
        bpp == 32,
        "neoplutarch currently requires a 32-bpp framebuffer (got {})",
        bpp
    );

    let base = NonNull::new(framebuffer.address() as *mut u32)
        .expect("limine: framebuffer address was null");

    let state = FbState {
        base,
        width: framebuffer.width as usize,
        height: framebuffer.height as usize,
        pitch_bytes: framebuffer.pitch as usize,
        bpp,
    };

    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::publish_framebuffer(
        state.base.as_ptr() as usize,
        state.width,
        state.height,
        state.pitch_bytes,
    );
    *FB.lock() = Some(state);

    clear_screen(Color::BLACK);
}

#[cfg(feature = "fb-progress-console")]
impl FbState {
    /// Fill a rectangle, clipped to the framebuffer.
    fn fill(&self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        let x_end = x.saturating_add(w).min(self.width);
        let y_end = y.saturating_add(h).min(self.height);
        if x >= x_end || y >= y_end {
            return;
        }
        // SAFETY: bounds clamped to the framebuffer above.
        unsafe {
            for yy in y..y_end {
                let row_ptr =
                    (self.base.as_ptr() as *mut u8).add(yy * self.pitch_bytes) as *mut u32;
                slice::from_raw_parts_mut(row_ptr.add(x), x_end - x).fill(color.0);
            }
        }
    }
}

/// Run `f` on the framebuffer if its lock is free right now. False when the
/// lock was contended (nothing drawn). The progress console must never wait
/// on a lock its own caller may hold.
#[cfg(feature = "fb-progress-console")]
fn try_with_fb(f: impl FnOnce(&FbState)) -> bool {
    match FB.try_lock() {
        Some(guard) => {
            if let Some(state) = guard.as_ref() {
                f(state);
            }
            true
        }
        None => false,
    }
}

/// Wait for any progress-console draw in flight to finish: the console draws
/// only while holding the framebuffer lock, and re-checks it is active under
/// that lock, so once this returns no console pixel can land.
#[cfg(feature = "fb-progress-console")]
fn console_barrier() {
    drop(FB.lock());
}

/// Whether the framebuffer lock is held (panic takeover).
#[cfg(feature = "fb-progress-console")]
fn fb_locked() -> bool {
    FB.is_locked()
}

/// Panic path only: take the framebuffer lock from whoever holds it (a CPU
/// that faulted mid-draw, or the one that is now halting).
///
/// # Safety
/// Only on the panic path, where nothing else will run to completion.
#[cfg(feature = "fb-progress-console")]
unsafe fn steal_fb() {
    if FB.is_locked() {
        // SAFETY: per the caller's contract.
        unsafe { FB.force_unlock() };
    }
}

/// Any drawer other than the progress console takes the screen: the mirror
/// stops, and any console draw already in flight finishes, before its first
/// pixel, so the console can never scribble over a compositor scene, a
/// splash, a panic screen or a digest-checked frame.
#[inline]
fn take_screen() {
    #[cfg(feature = "fb-progress-console")]
    {
        // R1-3: after a panic on another CPU, this CPU never draws again.
        progress::halt_if_panicked_elsewhere();
        let owner = OVERLAY_OWNER.load(core::sync::atomic::Ordering::Acquire);
        if owner == 0 || owner != overlay_tag() {
            progress::stop();
        }
    }
}

/// The thread drawing through `overlay` (its thread id + 1), or 0.
#[cfg(feature = "fb-progress-console")]
static OVERLAY_OWNER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

#[cfg(feature = "fb-progress-console")]
fn overlay_tag() -> u64 {
    crate::arch::x86_64::per_cpu::current_thread_id().wrapping_add(1)
}

/// Draw `f` as an overlay that does not take the screen: the progress
/// console keeps mirroring. Only for the scenario phase indicator, whose
/// band (`progress_logic::RESERVED_BOTTOM`) the console never draws into.
/// The exemption is tagged with the calling thread: any other thread that
/// draws meanwhile still takes the screen. Feature off it is `f()`, inlined.
#[inline(always)]
pub fn overlay(f: impl FnOnce()) {
    #[cfg(feature = "fb-progress-console")]
    OVERLAY_OWNER.store(overlay_tag(), core::sync::atomic::Ordering::Release);
    f();
    #[cfg(feature = "fb-progress-console")]
    OVERLAY_OWNER.store(0, core::sync::atomic::Ordering::Release);
}

/// Returns (width, height) in pixels, or (0, 0) if the framebuffer
/// hasn't been initialized yet.
pub fn dimensions() -> (usize, usize) {
    match FB.lock().as_ref() {
        Some(s) => (s.width, s.height),
        None => (0, 0),
    }
}

/// Fill the entire framebuffer with a single color.
pub fn clear(color: Color) {
    take_screen();
    clear_screen(color);
}

/// `clear` without taking the screen (`init`'s own blank).
fn clear_screen(color: Color) {
    if let Some(s) = FB.lock().as_ref() {
        // SAFETY: `s.base` is valid for `s.height * s.pitch_bytes` bytes,
        // which is at least `s.height * s.width` u32s.
        unsafe {
            for y in 0..s.height {
                let row_ptr = (s.base.as_ptr() as *mut u8).add(y * s.pitch_bytes) as *mut u32;
                let row = slice::from_raw_parts_mut(row_ptr, s.width);
                row.fill(color.0);
            }
        }
    }
}

/// Plot a single pixel. Out-of-bounds coordinates are silently clipped.
pub fn plot(x: usize, y: usize, color: Color) {
    take_screen();
    if let Some(s) = FB.lock().as_ref() {
        if x >= s.width || y >= s.height {
            return;
        }
        // SAFETY: bounds checked above.
        unsafe {
            let row_ptr = (s.base.as_ptr() as *mut u8).add(y * s.pitch_bytes) as *mut u32;
            *row_ptr.add(x) = color.0;
        }
    }
}

/// Fill a rectangle. Clipped against framebuffer bounds.
pub fn fill_rect(x: usize, y: usize, w: usize, h: usize, color: Color) {
    take_screen();
    if let Some(s) = FB.lock().as_ref() {
        let x_end = (x + w).min(s.width);
        let y_end = (y + h).min(s.height);
        if x >= x_end || y >= y_end {
            return;
        }
        // SAFETY: bounds clamped above.
        unsafe {
            for yy in y..y_end {
                let row_ptr = (s.base.as_ptr() as *mut u8).add(yy * s.pitch_bytes) as *mut u32;
                let row = slice::from_raw_parts_mut(row_ptr.add(x), x_end - x);
                row.fill(color.0);
            }
        }
    }
}

#[cfg(feature = "framebuffer-console")]
fn scroll_up_pixels(rows: usize, color: Color) {
    if rows == 0 {
        return;
    }
    if let Some(s) = FB.lock().as_ref() {
        let rows = rows.min(s.height);
        // SAFETY: both source and destination rows are clamped inside the
        // Limine-provided framebuffer. `copy` handles the overlapping row
        // ranges while scrolling toward lower addresses.
        unsafe {
            for y in 0..s.height.saturating_sub(rows) {
                let dst = (s.base.as_ptr() as *mut u8).add(y * s.pitch_bytes) as *mut u32;
                let src =
                    (s.base.as_ptr() as *mut u8).add((y + rows) * s.pitch_bytes) as *const u32;
                core::ptr::copy(src, dst, s.width);
            }
            for y in s.height.saturating_sub(rows)..s.height {
                let row_ptr = (s.base.as_ptr() as *mut u8).add(y * s.pitch_bytes) as *mut u32;
                let row = slice::from_raw_parts_mut(row_ptr, s.width);
                row.fill(color.0);
            }
        }
    }
}

/// Blit row-major RGBA8888 pixels with alpha blending. Clipped against
/// framebuffer bounds.
pub fn blit_rgba(x: usize, y: usize, w: usize, h: usize, rgba: &[u8]) {
    take_screen();
    let expected_len = match w.checked_mul(h).and_then(|px| px.checked_mul(4)) {
        Some(len) => len,
        None => return,
    };
    if rgba.len() < expected_len {
        return;
    }

    if let Some(s) = FB.lock().as_ref() {
        // SAFETY: destination coordinates are bounds-checked before every
        // write, and each row pointer is derived from Limine's valid
        // framebuffer base and pitch.
        unsafe {
            for src_y in 0..h {
                let Some(dst_y) = y.checked_add(src_y) else {
                    continue;
                };
                if dst_y >= s.height {
                    continue;
                }

                let row_ptr = (s.base.as_ptr() as *mut u8).add(dst_y * s.pitch_bytes) as *mut u32;

                for src_x in 0..w {
                    let Some(dst_x) = x.checked_add(src_x) else {
                        continue;
                    };
                    if dst_x >= s.width {
                        continue;
                    }

                    let src_i = (src_y * w + src_x) * 4;
                    let r = rgba[src_i] as u32;
                    let g = rgba[src_i + 1] as u32;
                    let b = rgba[src_i + 2] as u32;
                    let a = rgba[src_i + 3] as u32;

                    if a == 0 {
                        continue;
                    }

                    let dst = row_ptr.add(dst_x);
                    if a == 255 {
                        *dst = Color::rgb(r as u8, g as u8, b as u8).0;
                        continue;
                    }

                    let old = *dst;
                    let old_r = (old >> 16) & 0xff;
                    let old_g = (old >> 8) & 0xff;
                    let old_b = old & 0xff;
                    let inv_a = 255 - a;
                    let out_r = (r * a + old_r * inv_a + 127) / 255;
                    let out_g = (g * a + old_g * inv_a + 127) / 255;
                    let out_b = (b * a + old_b * inv_a + 127) / 255;

                    *dst = Color::rgb(out_r as u8, out_g as u8, out_b as u8).0;
                }
            }
        }
    }
}

/// One framebuffer-lock scope for a compositor present batch (atrium G1,
/// chad §4.5): the caller validates everything client-controlled BEFORE
/// calling, acquires the lock exactly once for the whole batch, and copies
/// rows through [`PresentTarget::copy_row_px`]. `blit_rgba` stays the
/// alpha-blending floor; this is the stride-aware opaque-XRGB path (XRGB8888
/// little-endian is byte-identical to the native 32-bit layout, so a row
/// copy is a u32 copy).
#[cfg(feature = "atrium-compositor-g1")]
pub fn with_present<R>(f: impl FnOnce(Option<PresentTarget<'_>>) -> R) -> R {
    take_screen();
    let guard = FB.lock();
    f(guard.as_ref().map(|s| PresentTarget { s }))
}

/// Access token proving the framebuffer lock is held (constructed only by
/// [`with_present`]). Bounds are re-clipped per row against the immutable
/// framebuffer geometry — no client-controlled validation happens here.
#[cfg(feature = "atrium-compositor-g1")]
pub struct PresentTarget<'a> {
    s: &'a FbState,
}

#[cfg(feature = "atrium-compositor-g1")]
impl PresentTarget<'_> {
    /// Copy `src` (whole little-endian XRGB8888 pixels, `src.len() % 4 == 0`)
    /// to the row `dst_y`, starting at pixel `dst_x`. Clipped against the
    /// framebuffer bounds.
    pub fn copy_row_px(&self, dst_x: usize, dst_y: usize, src: &[u8]) {
        if dst_y >= self.s.height || dst_x >= self.s.width {
            return;
        }
        let px = (src.len() / 4).min(self.s.width - dst_x);
        // SAFETY: dst_y/dst_x bounds-checked above, `px` clipped to the row
        // remainder; the row pointer derives from Limine's valid base+pitch.
        // Source pixels are read as unaligned little-endian u32s (frame
        // chunks are 4-byte multiples but carry no alignment guarantee).
        unsafe {
            let row_ptr =
                (self.s.base.as_ptr() as *mut u8).add(dst_y * self.s.pitch_bytes) as *mut u32;
            for i in 0..px {
                let v = u32::from_le_bytes([
                    src[i * 4],
                    src[i * 4 + 1],
                    src[i * 4 + 2],
                    src[i * 4 + 3],
                ]);
                *row_ptr.add(dst_x + i) = v;
            }
        }
    }
}

/// FNV-1a digest over the visible pixels (each row's `width` u32s; pitch
/// padding excluded). The kill-switch equality oracle: the compositor-off
/// build and the no-feature build must produce the SAME digest for the
/// same deterministic console transcript (compositor-off ≡ v1 console,
/// bit-for-bit).
#[cfg(feature = "neo-test")]
pub fn pixel_digest() -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    if let Some(s) = FB.lock().as_ref() {
        // SAFETY: same bounds discipline as `clear` — each row is `width`
        // u32s inside the Limine-provided framebuffer.
        unsafe {
            for y in 0..s.height {
                let row_ptr = (s.base.as_ptr() as *mut u8).add(y * s.pitch_bytes) as *const u32;
                for x in 0..s.width {
                    let px = *row_ptr.add(x);
                    for b in px.to_le_bytes() {
                        hash ^= b as u64;
                        hash = hash.wrapping_mul(FNV_PRIME);
                    }
                }
            }
        }
    }
    hash
}

/// [`pixel_digest`] with the pixels inside `masked` (x, y, w, h) hashed as 0
/// — Desktop S3's readback, whose Plex label boxes are verified separately.
#[cfg(feature = "neo-test")]
pub fn pixel_digest_masked(masked: &[(usize, usize, usize, usize)]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    if let Some(s) = FB.lock().as_ref() {
        // SAFETY: same bounds discipline as `pixel_digest`.
        unsafe {
            for y in 0..s.height {
                let row_ptr = (s.base.as_ptr() as *mut u8).add(y * s.pitch_bytes) as *const u32;
                for x in 0..s.width {
                    let hidden = masked
                        .iter()
                        .any(|&(mx, my, mw, mh)| x >= mx && y >= my && x < mx + mw && y < my + mh);
                    let px = if hidden { 0 } else { *row_ptr.add(x) };
                    for b in px.to_le_bytes() {
                        hash ^= b as u64;
                        hash = hash.wrapping_mul(FNV_PRIME);
                    }
                }
            }
        }
    }
    hash
}

/// Read one framebuffer pixel (neo-test scenario asserts). None if out of
/// bounds or the framebuffer is uninitialized.
#[cfg(feature = "neo-test")]
pub fn read_pixel(x: usize, y: usize) -> Option<u32> {
    FB.lock().as_ref().and_then(|s| {
        if x >= s.width || y >= s.height {
            return None;
        }
        // SAFETY: bounds checked above; row pointer from Limine base+pitch.
        unsafe {
            let row_ptr = (s.base.as_ptr() as *const u8).add(y * s.pitch_bytes) as *const u32;
            Some(*row_ptr.add(x))
        }
    })
}

/// Draw a single glyph at pixel coordinates. The font is an 8x16 bitmap
/// font; see `font.rs` for the glyph table.
pub fn draw_glyph(x: usize, y: usize, glyph: char, fg: Color, bg: Option<Color>) {
    take_screen();
    let bitmap = font::glyph(glyph);
    for (row, byte) in bitmap.iter().enumerate() {
        for col in 0..font::GLYPH_WIDTH {
            let bit_set = (byte >> (font::GLYPH_WIDTH - 1 - col)) & 1 == 1;
            if bit_set {
                plot(x + col, y + row, fg);
            } else if let Some(bg_color) = bg {
                plot(x + col, y + row, bg_color);
            }
        }
    }
}

/// Draw a string starting at pixel coordinates. Stops at the framebuffer
/// edge; does not wrap. Newlines advance to the next row.
pub fn draw_string(x: usize, y: usize, s: &str, fg: Color, bg: Option<Color>) {
    take_screen();
    let mut cx = x;
    let mut cy = y;
    for ch in s.chars() {
        if ch == '\n' {
            cx = x;
            cy += font::GLYPH_HEIGHT;
            continue;
        }
        draw_glyph(cx, cy, ch, fg, bg);
        cx += font::GLYPH_WIDTH;
    }
}

/// Draw a glyph with nearest-neighbor integer scaling.
#[cfg(feature = "neo-test")]
pub fn draw_glyph_scaled(
    x: usize,
    y: usize,
    glyph: char,
    scale: usize,
    fg: Color,
    bg: Option<Color>,
) {
    take_screen();
    if scale == 0 {
        return;
    }
    let bitmap = font::glyph(glyph);
    for (row, byte) in bitmap.iter().enumerate() {
        for col in 0..font::GLYPH_WIDTH {
            let bit_set = (byte >> (font::GLYPH_WIDTH - 1 - col)) & 1 == 1;
            let color = if bit_set { Some(fg) } else { bg };
            if let Some(color) = color {
                fill_rect(x + col * scale, y + row * scale, scale, scale, color);
            }
        }
    }
}

/// Draw a scaled string. Newlines advance by the scaled glyph height.
#[cfg(feature = "neo-test")]
pub fn draw_string_scaled(x: usize, y: usize, s: &str, scale: usize, fg: Color, bg: Option<Color>) {
    take_screen();
    let mut cx = x;
    let mut cy = y;
    for ch in s.chars() {
        if ch == '\n' {
            cx = x;
            cy += font::GLYPH_HEIGHT * scale;
            continue;
        }
        draw_glyph_scaled(cx, cy, ch, scale, fg, bg);
        cx += font::GLYPH_WIDTH * scale;
    }
}
