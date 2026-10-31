// SPDX-License-Identifier: MPL-2.0
//! The LogansOS boot sequence.
//!
//! This is design-doc §8 made real. The intent is that every boot looks
//! like jacking in: a single pixel becomes the wordmark, then a diagnostic
//! feed scrolls underneath in dim cyan. In Phase 0 we paint a static frame
//! of this; the timed/animated version lands once we have a timer source.
//!
//! Aesthetic locked here so the Phase 1 dynamic version can match it.

use super::boot_logo;
use crate::drivers::fb::{self, Color};
use crate::serial_println;

/// Render the full boot frame: clear, wordmark, diagnostic feed, status.
pub fn render_boot_sequence() {
    fb::clear(Color::BLACK);

    let (w, h) = fb::dimensions();
    if w == 0 || h == 0 {
        serial_println!("[ boot  ] framebuffer dimensions are zero, skipping render");
        return;
    }

    draw_borders(w, h);
    draw_boot_logo(w, h);
    draw_wordmark(w, h);
    draw_diagnostic_feed(w, h);
    draw_status_line(w, h);

    serial_println!("[ boot  ] boot sequence rendered ({}x{})", w, h);
}

/// Two thin neon rules at top and bottom — frames the screen as a
/// "terminal" without being heavy.
fn draw_borders(w: usize, h: usize) {
    fb::fill_rect(0, 16, w, 1, Color::DIM_BLUE);
    fb::fill_rect(0, h.saturating_sub(40), w, 1, Color::DIM_BLUE);
}

/// Center the LogansOS crest above the text wordmark.
fn draw_boot_logo(w: usize, h: usize) {
    let title_y = wordmark_title_y(h);
    let logo_x = w.saturating_sub(boot_logo::WIDTH) / 2;
    let logo_y = title_y.saturating_sub(boot_logo::HEIGHT + 24).max(24);

    fb::blit_rgba(
        logo_x,
        logo_y,
        boot_logo::WIDTH,
        boot_logo::HEIGHT,
        boot_logo::RGBA,
    );
}

/// Centered wordmark stacked over the project tagline and version line.
fn draw_wordmark(w: usize, h: usize) {
    let title = "LOGANS OS";
    let tagline = "Intelligence as a kernel primitive.";
    let version = concat!("v", env!("CARGO_PKG_VERSION"), "  -  PHASE 8 - ARTHUR");

    // Each glyph is GLYPH_WIDTH wide; we render the title at ~3x scale by
    // doubling pixels later. For Phase 0 we keep it 1:1 to avoid burning
    // time on a scaled blitter — the kernel-debug font is intentionally
    // utilitarian. The compositor will own pretty type later.
    let title_pixel_width = title.len() * 8;
    let tagline_pixel_width = tagline.len() * 8;
    let version_pixel_width = version.len() * 8;

    let title_x = w.saturating_sub(title_pixel_width) / 2;
    let title_y = wordmark_title_y(h);
    let tagline_x = w.saturating_sub(tagline_pixel_width) / 2;
    let tagline_y = title_y + 24;
    let version_x = w.saturating_sub(version_pixel_width) / 2;
    let version_y = title_y + 48;

    fb::draw_string(title_x, title_y, title, Color::ELECTRIC_BLUE, None);
    fb::draw_string(tagline_x, tagline_y, tagline, Color::SILVER, None);
    fb::draw_string(version_x, version_y, version, Color::MUTED, None);
}

/// The diagnostic feed under the wordmark. Still hard-coded — the timestamps
/// are aesthetic, not real (we don't yet have a system clock to drive them).
/// Each line is rewritten to reflect the actual state shipped in this phase.
/// Phase 1.4+ will replace this with a live read from kernel diagnostics.
fn draw_diagnostic_feed(w: usize, h: usize) {
    let lines = [
        "[ 0.001 ] LogansOS kernel online",
        "[ 0.012 ] limine handshake OK",
        "[ 0.024 ] framebuffer attached",
        "[ 0.038 ] serial console up on COM1",
        "[ 0.051 ] memory subsystem ready",
        "[ 0.067 ] scheduler: preemptive SMP ready",
        "[ 0.078 ] timer at 10 ms tick",
        "[ 0.089 ] welcome to THE WORLD",
    ];

    let max_line_chars = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let block_width = max_line_chars * 8;
    let block_x = w.saturating_sub(block_width) / 2;
    let mut y = wordmark_title_y(h) + 80;

    for line in &lines {
        fb::draw_string(block_x, y, line, Color::DIM_BLUE, None);
        y += 18;
    }
}

/// Status line: the line that closes the boot sequence and lives there
/// until Phase 1 replaces this with a real login prompt.
#[rustfmt::skip] fn draw_status_line(w: usize, h: usize) {
    #[cfg(not(feature = "boot-eth0"))] let status = "the world is listening."; #[cfg(feature = "boot-eth0")] let status_buf = boot_eth0_status_line(); #[cfg(feature = "boot-eth0")] let status = status_buf.as_str();
    let pixel_width = status.len() * 8;
    let x = w.saturating_sub(pixel_width) / 2;
    let y = h.saturating_sub(60);
    fb::draw_string(x, y, status, Color::TEXT, None);
}

fn wordmark_title_y(h: usize) -> usize {
    let default_y = h / 3;
    let min_logo_clearance = boot_logo::HEIGHT + 48;
    let max_text_clearance = h.saturating_sub(240);

    default_y
        .max(min_logo_clearance)
        .min(max_text_clearance.max(default_y))
}

#[cfg(feature = "boot-eth0")]
fn boot_eth0_status_line() -> alloc::string::String {
    let mut line = alloc::string::String::new();
    crate::drivers::usb::boot_eth0::write_status_line(&mut line);
    line
}
