// SPDX-License-Identifier: MPL-2.0
//! Framebuffer phase indicator for scenario runs.

use crate::drivers::fb::{self, Color};

pub enum ScenarioPhase {
    Boot,
    EnumerateDevices,
    MountSource,
    WritePhase,
    ReadbackPhase,
    Fsync,
    Cleanup,
    Pass,
    Fail(&'static str),
}

pub fn set_phase(phase: ScenarioPhase) {
    let (fb_w, fb_h) = fb::dimensions();
    if fb_w == 0 || fb_h == 0 {
        return;
    }

    let (rect_w, rect_h, scale) = indicator_geometry(fb_w, fb_h);
    let x = fb_w.saturating_sub(rect_w);
    let y = fb_h.saturating_sub(rect_h);
    let (bg, text) = phase_text(&phase);

    // An overlay: under `fb-progress-console` the log mirror keeps running
    // above this band instead of stopping.
    fb::overlay(|| {
        fb::fill_rect(x, y, rect_w, rect_h, bg);

        let text_w = text.len() * fb::GLYPH_WIDTH * scale;
        let text_h = fb::GLYPH_HEIGHT * scale;
        let text_x = x + rect_w.saturating_sub(text_w) / 2;
        let text_y = y + rect_h.saturating_sub(text_h) / 2;
        fb::draw_string_scaled(text_x, text_y, text, scale, Color::WHITE, Some(bg));
    });
}

/// The band never exceeds 200 px tall: `progress_logic::RESERVED_BOTTOM`.
fn indicator_geometry(fb_w: usize, fb_h: usize) -> (usize, usize, usize) {
    if fb_w < 1280 || fb_h < 800 {
        (fb_w.min(640), fb_h.min(160), 6)
    } else {
        (800, 200, 8)
    }
}

fn phase_text(phase: &ScenarioPhase) -> (Color, &'static str) {
    match phase {
        ScenarioPhase::Boot => (Color::DARK_BLUE, "BOOT"),
        ScenarioPhase::EnumerateDevices => (Color::DARK_BLUE, "ENUM DEVS"),
        ScenarioPhase::MountSource => (Color::DARK_BLUE, "MOUNT SRC"),
        ScenarioPhase::WritePhase => (Color::DARK_BLUE, "WRITE"),
        ScenarioPhase::ReadbackPhase => (Color::DARK_BLUE, "READBACK"),
        ScenarioPhase::Fsync => (Color::DARK_BLUE, "FSYNC"),
        ScenarioPhase::Cleanup => (Color::DARK_BLUE, "CLEANUP"),
        ScenarioPhase::Pass => (Color::PASS_GREEN, "PASS"),
        ScenarioPhase::Fail(reason) => (Color::FAIL_RED, fail_text(reason)),
    }
}

fn fail_text(reason: &'static str) -> &'static str {
    match reason {
        "no log sink device available" => "FAIL NO LOG",
        "log sink install failed" => "FAIL LOG",
        "log sink invalid" => "FAIL LOG",
        "log sink selftest failed" => "FAIL LOG SELFTEST",
        _ => "FAIL",
    }
}
