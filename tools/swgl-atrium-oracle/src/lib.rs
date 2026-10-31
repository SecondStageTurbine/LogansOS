// SPDX-License-Identifier: MPL-2.0
//! OT-61 browser C0 — A5(b): the **SWGL ↔ Atrium byte-contract oracle**.
//!
//! This is the host-only, zero-kernel proof of the render seam the browser
//! floor depends on: WebRender rasterizes through SWGL into SWGL's
//! **caller-owned framebuffer**, which IS an Atrium `SharedPixelBuffer`
//! client mapping; the compositor then reads those exact bytes. Before a
//! single line of target/kernel work, this oracle pins — with **hand-laid
//! bytes, independent of any SWGL or Atrium code** (ADR-028 discipline: a
//! writer and a reader built from one spec agree even when both are wrong) —
//! the four things that can silently corrupt the seam:
//!
//! 1. **Channel order.** SWGL's default framebuffer is **BGRA8** in memory
//!    (bytes `[B, G, R, A]`, the desktop-swapchain order wrench consumes —
//!    `wrench/src/main.rs:200-203`, "raw color buffer as BGRA"). Atrium
//!    `FORMAT_XRGB8888_LE` (`kernel/src/atrium/logic.rs:14-17`) is the DRM
//!    fourcc `XR24`: a little-endian u32 `0x??RRGGBB`, i.e. memory bytes
//!    `[B, G, R, X]`. They align **with no swap** on B/G/R; the 4th byte is
//!    A (SWGL) vs X (Atrium, read-ignored). A latent RGBA-vs-BGRA mistake
//!    would put R in byte 0 — this oracle catches it with **distinct R and B**.
//! 2. **Stride / row padding.** `init_default_framebuffer(0,0,w,h,stride,ptr)`
//!    takes an **explicit stride**; Atrium accepts any stride ≥ `width*4`
//!    aligned to 4. A tight-stride assumption (`width*4`) against a padded
//!    Atrium buffer mislocates every row after row 0. The oracle uses a
//!    **padded stride** so a tight-stride blit fails the byte compare.
//! 3. **The ignored high byte.** Atrium ignores byte 3; the displayed color
//!    depends only on B,G,R. The oracle proves a **nontrivial A** input does
//!    not change the interpreted RGB.
//! 4. **`get_color_buffer(_, flush=true)` delayed-clear resolution.** SWGL
//!    defers clears; `flush=true` (`swgl/src/gl.cc:2574-2593`) materializes
//!    them before readback. The oracle models the contract that a region
//!    cleared to color C reads back as C's BGRA bytes **after** flush (never
//!    stale/zero) — so the integration must pass `flush=true`, not assume the
//!    clear already landed.
//!
//! The future `AtriumSwglRenderingContext` (the SWGL rung, S1) must satisfy
//! these exact bytes; this crate is that spec, verifiable now with `cargo test`.

#![forbid(unsafe_code)]

/// Atrium's pixel format: DRM fourcc `XR24`, little-endian XRGB8888.
/// (Mirror of `kernel/src/atrium/logic.rs::FORMAT_XRGB8888_LE`, asserted below
/// so this oracle is pinned to the real kernel constant.)
pub const FORMAT_XRGB8888_LE: u32 = 0x3432_5258;

/// Bytes per pixel in both SWGL's BGRA8 framebuffer and Atrium XRGB8888.
pub const BYTES_PER_PIXEL: usize = 4;

// ── The seam contract, as pure functions (the SPEC the integration must meet) ──

/// One SWGL framebuffer pixel, in SWGL's **BGRA8 memory order** `[B, G, R, A]`.
/// This is the byte order SWGL writes into its caller-owned color buffer.
pub const fn swgl_bgra_pixel(b: u8, g: u8, r: u8, a: u8) -> [u8; BYTES_PER_PIXEL] {
    [b, g, r, a]
}

/// Read a 4-byte little-endian pixel word from Atrium buffer memory. Atrium's
/// present path does a `u32` row copy, so the in-memory bytes are interpreted
/// little-endian: bytes `[b, g, r, x]` → `0xXXRRGGBB`.
pub const fn atrium_word_le(bytes: [u8; BYTES_PER_PIXEL]) -> u32 {
    u32::from_le_bytes(bytes)
}

/// The displayed color Atrium derives from an XRGB8888_LE word `0x??RRGGBB`:
/// `(R, G, B)`, high byte ignored. A channel-swap anywhere in the seam changes
/// this triple for a pixel with distinct channels.
pub const fn atrium_display_rgb(word: u32) -> (u8, u8, u8) {
    (
        ((word >> 16) & 0xff) as u8, // R
        ((word >> 8) & 0xff) as u8,  // G
        (word & 0xff) as u8,         // B
    )
}

/// Copy an SWGL BGRA framebuffer into an Atrium XRGB8888 buffer, honoring the
/// (possibly different) per-row strides. This is the *contract* the direct
/// zero-copy binding realizes structurally (SWGL writes straight into the
/// Atrium mapping); modeled here so the byte oracle is executable. Panics on a
/// contract violation (stride < width*4, buffer too small) — the integration
/// must validate before binding, never over-run.
pub fn blit_swgl_to_atrium(
    swgl: &[u8],
    swgl_stride: usize,
    atrium: &mut [u8],
    atrium_stride: usize,
    width: usize,
    height: usize,
) {
    let row_bytes = width * BYTES_PER_PIXEL;
    assert!(swgl_stride >= row_bytes, "swgl stride < width*4");
    assert!(atrium_stride >= row_bytes, "atrium stride < width*4");
    assert!(
        swgl.len() >= (height - 1) * swgl_stride + row_bytes,
        "swgl buf short"
    );
    assert!(
        atrium.len() >= (height - 1) * atrium_stride + row_bytes,
        "atrium buf short"
    );
    for y in 0..height {
        let src = &swgl[y * swgl_stride..y * swgl_stride + row_bytes];
        let dst = &mut atrium[y * atrium_stride..y * atrium_stride + row_bytes];
        // Byte-identical copy: BGRA[B,G,R,A] → XRGB[B,G,R,X]; the A byte lands
        // in Atrium's ignored high byte. NO channel remap — the layouts align.
        dst.copy_from_slice(src);
    }
}

/// The `get_color_buffer(_, flush=true)` contract: a region cleared to color
/// `C` resolves to `C`'s BGRA bytes after flush. Modeled as: the flushed
/// readback of a clear-only surface is every pixel = `C`'s BGRA. (An
/// integration that passes `flush=false` would read stale/zero bytes; the
/// oracle's expected bytes are the *post-flush* values, so that bug fails.)
pub fn swgl_flushed_clear(
    color_bgra: [u8; BYTES_PER_PIXEL],
    width: usize,
    height: usize,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(width * height * BYTES_PER_PIXEL);
    for _ in 0..width * height {
        buf.extend_from_slice(&color_bgra);
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the format constant to the real kernel value ('XR24').
    #[test]
    fn format_constant_matches_kernel() {
        // 'X'=0x58 'R'=0x52 '2'=0x32 '4'=0x34, little-endian fourcc.
        assert_eq!(
            FORMAT_XRGB8888_LE,
            u32::from_le_bytes([0x58, 0x52, 0x32, 0x34])
        );
        assert_eq!(FORMAT_XRGB8888_LE, 0x3432_5258);
    }

    /// PROOF 1 — channel order, no swap. A single SWGL pixel with **distinct**
    /// B,G,R,A lands byte-identical in Atrium, and Atrium's display RGB reads
    /// back the intended R,G,B. A BGRA/RGBA confusion would fail this.
    #[test]
    fn channel_order_bgra_to_xrgb_no_swap() {
        // SWGL BGRA memory bytes for B=0x10, G=0x20, R=0x30, A=0x40.
        let px = swgl_bgra_pixel(0x10, 0x20, 0x30, 0x40);
        assert_eq!(px, [0x10, 0x20, 0x30, 0x40]); // hand-laid: [B,G,R,A]

        // Atrium reads it little-endian → 0x40302010.
        let word = atrium_word_le(px);
        assert_eq!(word, 0x4030_2010);

        // Atrium display interpretation 0x??RRGGBB → R=0x30, G=0x20, B=0x10.
        assert_eq!(atrium_display_rgb(word), (0x30, 0x20, 0x10));
        // i.e. the intended R=0x30, G=0x20, B=0x10 survive with no swap, and
        // the A=0x40 sits in the ignored high byte.
    }

    /// PROOF 3 — the high byte is ignored: a nontrivial A does not change the
    /// displayed RGB. (Numbered to match the module doc's four proofs.)
    #[test]
    fn high_byte_ignored() {
        let base = swgl_bgra_pixel(0x11, 0x22, 0x33, 0x00);
        let with_a = swgl_bgra_pixel(0x11, 0x22, 0x33, 0xAB);
        assert_eq!(
            atrium_display_rgb(atrium_word_le(base)),
            atrium_display_rgb(atrium_word_le(with_a)),
            "the A/X high byte must not affect the displayed RGB"
        );
        assert_eq!(
            atrium_display_rgb(atrium_word_le(with_a)),
            (0x33, 0x22, 0x11)
        );
    }

    /// PROOF 2 — padded stride. A 3×2 image whose Atrium stride is 16 bytes
    /// (4 bytes of padding past the 12-byte row) must place row 1 at byte
    /// offset 16, not 12. The full expected Atrium buffer is HAND-LAID here,
    /// independent of `blit_swgl_to_atrium`; a tight-stride blit would write
    /// row 1 at offset 12 and fail this exact-byte compare.
    #[test]
    fn padded_stride_row_placement_byte_oracle() {
        const W: usize = 3;
        const H: usize = 2;
        const SWGL_STRIDE: usize = 12; // tight source
        const ATRIUM_STRIDE: usize = 16; // padded destination (4-byte tail pad)

        // SWGL source (tight, 12 bytes/row). Distinct channels per pixel so any
        // transposition is visible. Bytes are [B,G,R,A].
        // row0: px(1,2,3,X)=[01,02,03,F0]  px(4,5,6)=[04,05,06,F1]  px(7,8,9)=[07,08,09,F2]
        // row1: px(A,B,C)=[0A,0B,0C,F3]     px(D,E,F)=[0D,0E,0F,F4]  px(10,11,12)=[10,11,12,F5]
        let swgl: [u8; 24] = [
            0x01, 0x02, 0x03, 0xF0, 0x04, 0x05, 0x06, 0xF1, 0x07, 0x08, 0x09, 0xF2, // row0
            0x0A, 0x0B, 0x0C, 0xF3, 0x0D, 0x0E, 0x0F, 0xF4, 0x10, 0x11, 0x12, 0xF5, // row1
        ];

        // HAND-LAID expected Atrium buffer (2 rows × 16-byte stride = 32 bytes):
        // row0 occupies bytes [0..12], pad [12..16]=00; row1 at [16..28], pad
        // [28..32]=00. The 12 payload bytes per row equal the SWGL row verbatim.
        let expected: [u8; 32] = [
            // row0 payload (offset 0), then 4 pad bytes
            0x01, 0x02, 0x03, 0xF0, 0x04, 0x05, 0x06, 0xF1, 0x07, 0x08, 0x09, 0xF2, //
            0x00, 0x00, 0x00, 0x00, // stride pad
            // row1 payload (offset 16), then 4 pad bytes
            0x0A, 0x0B, 0x0C, 0xF3, 0x0D, 0x0E, 0x0F, 0xF4, 0x10, 0x11, 0x12, 0xF5, //
            0x00, 0x00, 0x00, 0x00, // stride pad
        ];

        let mut atrium = [0u8; 32];
        blit_swgl_to_atrium(&swgl, SWGL_STRIDE, &mut atrium, ATRIUM_STRIDE, W, H);
        assert_eq!(
            atrium, expected,
            "row 1 must land at the padded stride offset"
        );

        // Cross-check the interpretation of pixel (row1, col2) = SWGL [10,11,12,F5]:
        // Atrium word LE = 0xF5121110 → R=0x12, G=0x11, B=0x10.
        let px_word = atrium_word_le([0x10, 0x11, 0x12, 0xF5]);
        assert_eq!(px_word, 0xF512_1110);
        assert_eq!(atrium_display_rgb(px_word), (0x12, 0x11, 0x10));
    }

    /// PROOF 4 — `flush=true` delayed-clear resolution. A 2×2 surface cleared
    /// to a nontrivial color must read back as that color's BGRA bytes after
    /// flush. Expected bytes are the POST-flush values, hand-laid; an
    /// integration that skipped `flush=true` would read stale bytes and fail.
    #[test]
    fn flush_resolves_delayed_clear_byte_oracle() {
        // Clear color: B=0x21, G=0x43, R=0x65, A=0x87.
        let clear = swgl_bgra_pixel(0x21, 0x43, 0x65, 0x87);
        let flushed = swgl_flushed_clear(clear, 2, 2);
        // HAND-LAID: four pixels, each the clear color's BGRA.
        let expected: [u8; 16] = [
            0x21, 0x43, 0x65, 0x87, 0x21, 0x43, 0x65, 0x87, //
            0x21, 0x43, 0x65, 0x87, 0x21, 0x43, 0x65, 0x87, //
        ];
        assert_eq!(flushed.as_slice(), &expected);
        // And the resolved display color is R=0x65,G=0x43,B=0x21 everywhere.
        assert_eq!(
            atrium_display_rgb(atrium_word_le(clear)),
            (0x65, 0x43, 0x21)
        );
    }

    /// Guard: the blit refuses a stride narrower than the row (no silent
    /// over-read/over-write — the integration validates before binding).
    #[test]
    #[should_panic(expected = "atrium stride < width*4")]
    fn rejects_underwide_atrium_stride() {
        let swgl = [0u8; 16];
        let mut atrium = [0u8; 16];
        // width=3 → row needs 12 bytes, but give Atrium stride 8.
        blit_swgl_to_atrium(&swgl, 12, &mut atrium, 8, 3, 1);
    }
}
