// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the S3 scene. The byte oracles are laid out by hand in the
//! comments; none is produced by the code under test.

use crate::blur::{blur_calls, blur_plane};
use crate::contrast::ratio;
use crate::layout::{Layout, Rect};
use crate::material::{ELECTRIC_BLUE, Mode, RT_PANEL, RT_RAISED, TrustState, glass};
use crate::scene::{BaseScene, LABEL_PRIMARY, LABEL_SECONDARY};
use crate::shape::{RoundRect, corner_mask};
use crate::wide::{dither, widen};

/// 1-D box, r = 1, edge-clamped, round half up: `(Σ + 1) / 3`.
///   in             [0, 0, 255, 0, 0]
///   pass 1   i0: (0+0+0+1)/3=0  i1: (0+0+255+1)/3=85  i2: 85  i3: 85  i4: 0
///            (the vertical box of a 1-row plane is the identity: (3v+1)/3 = v)
///   pass 2   i0: (0+0+85+1)/3=28  i1: (0+85+85+1)/3=57  i2: (255+1)/3=85  → [28,57,85,57,28]
///   pass 3   i0: (28+28+57+1)/3=38  i1: (28+57+85+1)/3=57  i2: (57+85+57+1)/3=66
///            → [38, 57, 66, 57, 38]
#[test]
fn blur_kernel_byte_oracle_1d() {
    let mut one = [0u32, 0, 255, 0, 0];
    blur_plane(&mut one, 5, 1, 1, 1);
    assert_eq!(one, [0, 85, 85, 85, 0]);
    let mut three = [0u32, 0, 255, 0, 0];
    blur_plane(&mut three, 5, 1, 1, 3);
    assert_eq!(three, [38, 57, 66, 57, 38]);
}

/// 3×3 impulse of 900, r = 1, one pass: every row clamps to three samples of
/// the middle row ⇒ middle row (0+900+0+1)/3 = 300 everywhere (the clamped
/// ends re-read the 0 and the 900), outer rows 0; then each column
/// [0,300,0] ⇒ (0+0+300+1)/3 = 100, (0+300+0+1)/3 = 100, 100 ⇒ all 100.
#[test]
fn blur_kernel_byte_oracle_2d_clamp() {
    let mut p = [0u32, 0, 0, 0, 900, 0, 0, 0, 0];
    blur_plane(&mut p, 3, 3, 1, 1);
    assert_eq!(p, [100; 9]);
}

/// Exact-area corner masks.
///   r = 1: the pixel is the whole quarter disc, area π/4 = 0.785398 → 255·A =
///          200.28 → 200.
///   r = 2 (row-major from the corner pixel):
///     (0,0) square [1,2]² ∩ disc: ∫₁^√3 (√(4−t²) − 1) dt = F(√3) − F(1) − (√3−1)
///           with F(t) = ½(t√(4−t²) + 4·asin(t/2)): 2.960420 − 1.913223 − 0.732051
///           = 0.315146 → 80.36 → 80
///     (1,0) and (0,1): ∫₀¹ (√(4−t²) − 1) dt = F(1) − 1 = 0.913223 → 232.87 → 233
///     (1,1) square [0,1]² is wholly inside → 255
///   Σ areas = 0.315146 + 2·0.913223 + 1 = π (the quarter disc) ✓
#[test]
fn corner_mask_exact_area_oracle() {
    assert_eq!(corner_mask(1), [200]);
    assert_eq!(corner_mask(2), [80, 233, 233, 255]);
    for r in [3usize, 6, 10, 12] {
        let total: f64 = corner_mask(r).iter().map(|&c| c as f64 / 255.0).sum();
        let exact = core::f64::consts::PI * (r * r) as f64 / 4.0;
        assert!(
            (total - exact).abs() <= (r * r) as f64 * 0.5 / 255.0,
            "r={r}: Σ={total} vs πr²/4={exact}"
        );
    }
}

#[test]
fn round_rect_coverage_is_symmetric_and_solid_inside() {
    let s = RoundRect::new(10, 20, 40, 30, 6);
    for dy in 0..30 {
        for dx in 0..40 {
            let c = s.coverage(10 + dx, 20 + dy);
            assert_eq!(c, s.coverage(10 + 39 - dx, 20 + dy));
            assert_eq!(c, s.coverage(10 + dx, 20 + 29 - dy));
        }
    }
    assert_eq!(s.coverage(30, 35), 255);
    assert_eq!(s.coverage(9, 35), 0);
    assert_eq!(s.coverage(10, 20), corner_mask(6)[0] as u32);
}

/// Owned glass over black: keep = 256−18−26 = 212,
///   R: (0 + 65280·18 + 60·256·26 + 128) >> 8 = 1574528 >> 8 = 6150
///   G: (1175040 + 142·256·26 + 128) >> 8 = 2120320 >> 8 = 8282
///   B: (1175040 + 244·256·26 + 128) >> 8 = 2799232 >> 8 = 10934
#[test]
fn owned_glass_tint_byte_oracle() {
    let t = TrustState::Owned.tint();
    assert_eq!(
        (t.frost, t.vibrancy, t.vibrancy_color),
        (18, 26, ELECTRIC_BLUE)
    );
    assert_eq!(
        glass([0, 0, 0], t.frost, t.vibrancy, t.vibrancy_color),
        [6150, 8282, 10934]
    );
}

/// Opaque tokens dither back to themselves; a half step dithers by the Bayer
/// threshold: W = 10.5·256 = 2688 → (2688 + 0·16 + 8)>>8 = 10 at (0,0), and
/// (2688 + 8·16 + 8)>>8 = 2824>>8 = 11 at (1,0).
#[test]
fn dither_oracle() {
    for (x, y) in [(0, 0), (1, 0), (3, 3), (7, 2)] {
        assert_eq!(dither(widen(0x0012_3456), x, y), 0x0012_3456);
        assert_eq!(dither(widen(0x00FF_FFFF), x, y), 0x00FF_FFFF);
    }
    assert_eq!(dither([2688, 0, 0], 0, 0) >> 16, 10);
    assert_eq!(dither([2688, 0, 0], 1, 0) >> 16, 11);
}

/// The Reduce-Transparency floor meets WCAG AAA (7:1) for every label on the
/// surface it sits on.
#[test]
fn reduce_transparency_contrast_is_aaa() {
    for (fg, bg) in [
        (LABEL_PRIMARY, RT_PANEL),
        (LABEL_SECONDARY, RT_PANEL),
        (LABEL_PRIMARY, RT_RAISED),
        (LABEL_SECONDARY, RT_RAISED),
    ] {
        let r = ratio(fg, bg);
        assert!(r >= 7.0, "{fg:06x} on {bg:06x} = {r:.2}:1 < 7:1");
    }
    // Sanity anchor from the design system: white on Void is 14.9:1 (≈18.5 by
    // the exact formula for #0a0e27; the doc's figure is rounded down).
    assert!(ratio(0x00FF_FFFF, 0x000A_0E27) > 14.9);
}

#[test]
fn layout_1080p_hand_derived() {
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    // workfield x 84..1920 (1836), y 36..1080 (1044); frame 656 × 442:
    // x = 84 + (1836−656)/2 = 674, y = 36 + (1044−442)/2 = 337; client +8, +34.
    assert_eq!(l.frame, Rect::new(674, 337, 656, 442));
    assert_eq!(l.client, Rect::new(682, 371, 640, 400));
    assert_eq!(l.glow, Rect::new(660, 323, 684, 470));
    // Minimum: 84 + 656 + 2·(14+16) = 800 wide, 36 + 442 + 60 = 538 tall.
    assert!(Layout::new(800, 538, 640, 400).is_some());
    assert!(Layout::new(799, 600, 640, 400).is_none());
    assert!(Layout::new(800, 537, 640, 400).is_none());
}

/// Typing damage never re-blurs: building the glass scene blurs exactly the
/// four glass boxes; repainting the client rect (what a keystroke damages)
/// and even a focus change run zero further blurs. RT never blurs at all.
#[test]
fn blur_runs_once_per_scene_never_per_frame() {
    let before = blur_calls();
    let scene = BaseScene::new(1280, 800, 640, 400, Mode::Glass).unwrap();
    let built = blur_calls();
    assert_eq!(built - before, 4);
    let mut buf = vec![0u32; 1280 * 800];
    for _ in 0..20 {
        scene.paint_rect(&mut buf, 1280, true, scene.layout.client);
    }
    scene.paint_rect(&mut buf, 1280, false, scene.layout.focus_damage());
    assert_eq!(blur_calls(), built);
    let _rt = BaseScene::new(1280, 800, 640, 400, Mode::ReduceTransparency).unwrap();
    assert_eq!(blur_calls(), built);
}

/// A damage repaint equals the corresponding region of a full repaint
/// (labels included), so recomposing only damage can never drift.
#[test]
fn damage_repaint_matches_full_paint() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let scene = BaseScene::new(1280, 800, 640, 400, mode).unwrap();
        let mut full = vec![0u32; 1280 * 800];
        scene.paint(&mut full, 1280, true);
        let mut part = full.clone();
        let r = scene.layout.focus_damage();
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                part[y * 1280 + x] = 0xDEAD;
            }
        }
        scene.paint_rect(&mut part, 1280, true, r);
        assert!(full == part, "{mode:?}");
        // The WM's focus repaint walks the disjoint regions; together they
        // must reproduce every pixel a focus change can alter.
        let mut other = vec![0u32; 1280 * 800];
        scene.paint(&mut other, 1280, false);
        for q in scene.layout.focus_regions() {
            scene.paint_rect(&mut other, 1280, true, q);
        }
        assert!(full == other, "{mode:?} focus regions");
    }
}

#[test]
fn glass_differs_from_rt_only_inside_chrome() {
    let g = BaseScene::new(1280, 800, 640, 400, Mode::Glass).unwrap();
    let rt = BaseScene::new(1280, 800, 640, 400, Mode::ReduceTransparency).unwrap();
    let l = g.layout;
    for (x, y) in [(400usize, 700usize), (1200, 100), (100, 790)] {
        if !l.spine.contains(x, y) && !l.bar.contains(x, y) && !l.frame.contains(x, y) {
            assert_eq!(g.wide_pixel(x, y, true), rt.wide_pixel(x, y, true));
        }
    }
    let (x, y) = (l.frame.x + 200, l.frame.y + 20);
    assert_eq!(rt.wide_pixel(x, y, true), widen(RT_PANEL));
    assert_ne!(g.wide_pixel(x, y, true), widen(RT_PANEL));
}

/// Native (host) cost of the 1080p glass scene, for the PR's perf report:
/// `cargo test -p neo-atrium -- --ignored --nocapture perf_native`. Not a gate.
#[test]
#[ignore]
fn perf_native_1080p() {
    let tsc = || unsafe { core::arch::x86_64::_rdtsc() };
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let t0 = tsc();
        let scene = BaseScene::new(1920, 1080, 640, 400, mode).unwrap();
        let t1 = tsc();
        let mut buf = vec![0u32; 1920 * 1080];
        scene.paint(&mut buf, 1920, true);
        let t2 = tsc();
        // A keystroke damages Arthur's client rect; the WM recomposes it from
        // the cached base (a copy), never from `paint_rect`. The worst damage
        // the base itself ever repaints is the focus change.
        for r in scene.layout.focus_regions() {
            scene.paint_rect(&mut buf, 1920, false, r);
        }
        let t3 = tsc();
        std::println!(
            "{mode:?}: build_cycles={} full_paint_cycles={} focus_repaint_cycles={}",
            t1 - t0,
            t2 - t1,
            t3 - t2
        );
    }
}

/// `NEO_ATRIUM_PREVIEW=<dir> cargo test -p neo-atrium -- --ignored preview`
/// writes 1080p PPMs of both modes (focused, with the focus light) for design
/// review. Not a gate.
#[test]
#[ignore]
fn preview() {
    let Ok(dir) = std::env::var("NEO_ATRIUM_PREVIEW") else {
        return;
    };
    let dims: (usize, usize) = std::env::var("NEO_ATRIUM_PREVIEW_DIMS")
        .ok()
        .and_then(|d| {
            let (w, h) = d.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((1920, 1080));
    let focused = std::env::var("NEO_ATRIUM_PREVIEW_UNFOCUSED").is_err();
    for (mode, name) in [(Mode::Glass, "glass"), (Mode::ReduceTransparency, "rt")] {
        let (w, h) = dims;
        let scene = BaseScene::new(w, h, 640, 400, mode).unwrap();
        let (buf, _) = render_scene(&scene, focused);
        write_ppm(&format!("{dir}/s3-{name}.ppm"), &buf, w, h);
        // DS-1 Home view: NEO_ATRIUM_PREVIEW_HOME=<typed text> (may be empty).
        if let Ok(text) = std::env::var("NEO_ATRIUM_PREVIEW_HOME") {
            let scene = BaseScene::with_home(w, h, 640, 400, mode).unwrap();
            let mut buf = vec![0u32; w * h];
            scene.paint_home_rect(&mut buf, w, focused, &text, scene.layout.screen);
            write_ppm(&format!("{dir}/ds1-home-{name}.ppm"), &buf, w, h);
        }
    }
}

fn write_ppm(path: &str, buf: &[u32], w: usize, h: usize) {
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in buf {
        out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
    }
    std::fs::write(path, out).unwrap();
}

/// A rect as the oracles' `(x, y, w, h)` tuple, and back.
fn tuple(r: &Rect) -> (usize, usize, usize, usize) {
    (r.x, r.y, r.w, r.h)
}

fn rect(&(x, y, w, h): &(usize, usize, usize, usize)) -> Rect {
    Rect::new(x, y, w, h)
}

/// The first pixel of window `win` (in a `stride`-pixel buffer) with a
/// channel outside the oracle's per-pixel `[lo, hi]` bounds:
/// `(x, y, got, lo, hi)`.
fn window_violation(
    buf: &[u32],
    stride: usize,
    (wx, wy, ww, wh): (usize, usize, usize, usize),
    lo: &[u32],
    hi: &[u32],
) -> Option<(usize, usize, u32, u32, u32)> {
    for y in 0..wh {
        for x in 0..ww {
            let got = buf[(wy + y) * stride + wx + x];
            let (l, h) = (lo[y * ww + x], hi[y * ww + x]);
            if [16u32, 8, 0].iter().any(|&s| {
                let c = (got >> s) & 0xFF;
                c < (l >> s) & 0xFF || c > (h >> s) & 0xFF
            }) {
                return Some((wx + x, wy + y, got, l, h));
            }
        }
    }
    None
}

mod oracle_digests {
    include!("../../../fixtures/desktop-s3/scene_digests.rs");
}

/// FNV-1a over the visible pixels (u32 LE, row-major) with `boxes` hashed as 0
/// — the same digest `scripts/desktop/scene_oracle.py` prints and the kernel
/// `desktop_glass_boot` readback computes over the real framebuffer.
fn masked_digest(px: &[u32], w: usize, h: usize, boxes: &[Rect]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for y in 0..h {
        for x in 0..w {
            let v = if boxes.iter().any(|b| b.contains(x, y)) {
                0
            } else {
                px[y * w + x]
            };
            for b in v.to_le_bytes() {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    hash
}

fn render(w: usize, h: usize, mode: Mode, focused: bool) -> (Vec<u32>, Layout) {
    render_scene(&BaseScene::new(w, h, 640, 400, mode).unwrap(), focused)
}

/// The window view of `scene` as the WM presents it (base + focus light).
fn render_scene(scene: &BaseScene, focused: bool) -> (Vec<u32>, Layout) {
    let (w, h) = (scene.layout.screen.w, scene.layout.screen.h);
    let mut buf = vec![0u32; w * h];
    scene.paint(&mut buf, w, focused);
    // The WM composites the focus light with atrium_wm::blend_over (S1).
    composite_glow(scene, &mut buf, w, focused);
    (buf, scene.layout)
}

/// The Rust scene equals the independent Python oracle, bit for bit, outside
/// the label boxes — at the pinned 1080p mode and at QEMU's default 1280×800,
/// in both materials and both focus states.
#[test]
fn scene_matches_independent_python_oracle() {
    use oracle_digests::*;
    // The kernel masks the oracle's boxes; they must be the layout's boxes.
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    let boxes = l.label_boxes().map(|r| tuple(&r));
    assert_eq!(boxes, S3_LABEL_BOXES_1920X1080);
    for (w, h, mode, focused, want) in [
        (1920, 1080, Mode::Glass, false, S3_GLASS_1920X1080_UNFOCUSED),
        (1920, 1080, Mode::Glass, true, S3_GLASS_1920X1080_FOCUSED),
        (
            1920,
            1080,
            Mode::ReduceTransparency,
            false,
            S3_RT_1920X1080_UNFOCUSED,
        ),
        (
            1920,
            1080,
            Mode::ReduceTransparency,
            true,
            S3_RT_1920X1080_FOCUSED,
        ),
        (1280, 800, Mode::Glass, false, S3_GLASS_1280X800_UNFOCUSED),
        (1280, 800, Mode::Glass, true, S3_GLASS_1280X800_FOCUSED),
        (
            1280,
            800,
            Mode::ReduceTransparency,
            false,
            S3_RT_1280X800_UNFOCUSED,
        ),
        (
            1280,
            800,
            Mode::ReduceTransparency,
            true,
            S3_RT_1280X800_FOCUSED,
        ),
    ] {
        let (buf, l) = render(w, h, mode, focused);
        assert_eq!(
            masked_digest(&buf, w, h, &l.label_boxes()),
            want,
            "{w}x{h} {mode:?} focused={focused}"
        );
    }
}

mod bar_label_rt {
    include!("../../../fixtures/desktop-s3/bar_label_rt.rs");
}
mod title_label_rt {
    include!("../../../fixtures/desktop-s3/title_label_rt.rs");
}

/// The chrome's Inter labels, checked against scripts/fonts/area_oracle.py
/// (exact outline area via fontTools, no neo-text, no FreeType) on the
/// uniform Reduce-Transparency panel: every pixel of the craft-bar "Atrium"
/// and the title "Arthur" lies within the blend of exact coverage ±2/255.
#[test]
fn chrome_labels_match_exact_area_font_oracle() {
    let (buf, _) = render(1920, 1080, Mode::ReduceTransparency, false);
    for (name, (wx, wy, ww, wh), lo, hi, ink_min) in [
        (
            "bar",
            bar_label_rt::WINDOW,
            &bar_label_rt::LO[..],
            &bar_label_rt::HI[..],
            bar_label_rt::INK_PIXELS,
        ),
        (
            "title",
            title_label_rt::WINDOW,
            &title_label_rt::LO[..],
            &title_label_rt::HI[..],
            title_label_rt::INK_PIXELS,
        ),
    ] {
        if let Some((x, y, got, l, h)) = window_violation(&buf, 1920, (wx, wy, ww, wh), lo, hi) {
            panic!("{name} label pixel ({x},{y}) = {got:06x} outside [{l:06x}, {h:06x}]");
        }
        let ink = (0..wh)
            .flat_map(|y| (0..ww).map(move |x| (x, y)))
            .filter(|&(x, y)| buf[(wy + y) * 1920 + wx + x] & 0xFF > 0x80)
            .count();
        assert!(ink >= ink_min / 2, "{name}: ink {ink} < {ink_min}/2");
    }
}

// ── DS-1 Home view ──────────────────────────────────────────────────────────

mod home_digests {
    include!("../../../fixtures/desktop-ds1/home_digests.rs");
}

fn render_home(w: usize, h: usize, mode: Mode, focused: bool, text: &str) -> (Vec<u32>, Layout) {
    let scene = BaseScene::with_home(w, h, 640, 400, mode).unwrap();
    let mut buf = vec![0u32; w * h];
    scene.paint_home_rect(&mut buf, w, focused, text, scene.layout.screen);
    (buf, scene.layout)
}

/// The Home view equals the independent Python oracle outside its label boxes
/// (the bar label, the intent field's text box, the chip labels), in both
/// materials, both focus states and both resolutions.
#[test]
fn home_matches_independent_python_oracle() {
    use home_digests::*;
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    let boxes = l.home_label_boxes().map(|r| tuple(&r));
    assert_eq!(boxes, DS1_HOME_LABEL_BOXES_1920X1080);
    for (w, h, mode, focused, want) in [
        (
            1920,
            1080,
            Mode::Glass,
            false,
            DS1_HOME_GLASS_1920X1080_UNFOCUSED,
        ),
        (
            1920,
            1080,
            Mode::Glass,
            true,
            DS1_HOME_GLASS_1920X1080_FOCUSED,
        ),
        (
            1920,
            1080,
            Mode::ReduceTransparency,
            false,
            DS1_HOME_RT_1920X1080_UNFOCUSED,
        ),
        (
            1920,
            1080,
            Mode::ReduceTransparency,
            true,
            DS1_HOME_RT_1920X1080_FOCUSED,
        ),
        (
            1280,
            800,
            Mode::Glass,
            false,
            DS1_HOME_GLASS_1280X800_UNFOCUSED,
        ),
        (
            1280,
            800,
            Mode::Glass,
            true,
            DS1_HOME_GLASS_1280X800_FOCUSED,
        ),
        (
            1280,
            800,
            Mode::ReduceTransparency,
            false,
            DS1_HOME_RT_1280X800_UNFOCUSED,
        ),
        (
            1280,
            800,
            Mode::ReduceTransparency,
            true,
            DS1_HOME_RT_1280X800_FOCUSED,
        ),
    ] {
        let (buf, l) = render_home(w, h, mode, focused, "typed text is masked");
        assert_eq!(
            masked_digest(&buf, w, h, &l.home_label_boxes()),
            want,
            "{w}x{h} {mode:?} focused={focused}"
        );
    }
}

/// Arthur held (yielded): the dot under its chip; then the keyboard on the
/// chip (Tab) — the focus light moves from the field to the chip and the
/// caret goes. Both against the independent oracle.
#[test]
fn home_held_and_chip_selected_match_independent_python_oracle() {
    use home_digests::*;
    for (mode, chip_selected, want) in [
        (Mode::Glass, false, DS1_HOME_GLASS_1920X1080_HELD),
        (Mode::Glass, true, DS1_HOME_GLASS_1920X1080_HELD_CHIP),
        (Mode::ReduceTransparency, false, DS1_HOME_RT_1920X1080_HELD),
        (
            Mode::ReduceTransparency,
            true,
            DS1_HOME_RT_1920X1080_HELD_CHIP,
        ),
    ] {
        let scene = BaseScene::with_home(1920, 1080, 640, 400, mode).unwrap();
        let mut buf = vec![0u32; 1920 * 1080];
        let view = crate::View::Home {
            focused: true,
            held: true,
            chip_selected,
        };
        scene.paint_home_view(&mut buf, 1920, view, "", scene.layout.screen);
        assert_eq!(
            masked_digest(&buf, 1920, 1080, &scene.layout.home_label_boxes()),
            want,
            "{mode:?} chip_selected={chip_selected}"
        );
    }
}

/// Toggling held / chip selection changes only the Spine and the field
/// (what the WM repaints for it).
#[test]
fn home_state_changes_stay_in_spine_and_field() {
    let scene = BaseScene::with_home(1280, 800, 640, 400, Mode::Glass).unwrap();
    let l = scene.layout;
    let paint = |view| {
        let mut b = vec![0u32; 1280 * 800];
        scene.paint_home_view(&mut b, 1280, view, "", l.screen);
        b
    };
    let plain = paint(crate::View::home(true));
    let held_chip = paint(crate::View::Home {
        focused: true,
        held: true,
        chip_selected: true,
    });
    for y in 0..800 {
        for x in 0..1280 {
            if plain[y * 1280 + x] != held_chip[y * 1280 + x] {
                assert!(
                    l.spine.contains(x, y) || l.field_glow.contains(x, y),
                    "({x},{y}) changed outside the Spine and the field"
                );
            }
        }
    }
}

/// The Home view adds exactly one blur (the intent field) at build; typing
/// into the field repaints only the field and never blurs again.
#[test]
fn home_blurs_once_and_typing_never_reblurs() {
    let before = blur_calls();
    let scene = BaseScene::with_home(1280, 800, 640, 400, Mode::Glass).unwrap();
    assert_eq!(blur_calls() - before, 5);
    let built = blur_calls();
    let mut buf = vec![0u32; 1280 * 800];
    let mut text = std::string::String::new();
    for ch in "hello desktop".chars() {
        text.push(ch);
        scene.paint_home_rect(&mut buf, 1280, true, &text, scene.layout.field);
    }
    assert_eq!(blur_calls(), built);
    let _ = BaseScene::with_home(1280, 800, 640, 400, Mode::ReduceTransparency).unwrap();
    assert_eq!(blur_calls(), built);
}

/// A keystroke repaints only the intent field, and that equals a full Home
/// paint with the new text (no stale glyph or caret survives).
#[test]
fn home_field_repaint_matches_full_paint() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let scene = BaseScene::with_home(1280, 800, 640, 400, mode).unwrap();
        let mut live = vec![0u32; 1280 * 800];
        scene.paint_home_rect(&mut live, 1280, true, "", scene.layout.screen);
        for text in [
            "h",
            "he",
            "hello",
            "hell",
            "a very long intent that overflows the field and must scroll left so that its end stays visible",
        ] {
            scene.paint_home_rect(&mut live, 1280, true, text, scene.layout.field);
            let mut full = vec![0u32; 1280 * 800];
            scene.paint_home_rect(&mut full, 1280, true, text, scene.layout.screen);
            assert!(live == full, "{mode:?} {text:?}");
        }
    }
}

/// Switching between the Home view and the window view repaints only
/// `Layout::view_regions()`, and lands exactly on a full paint of the target.
#[test]
fn view_switch_regions_cover_every_changed_pixel() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let scene = BaseScene::with_home(1280, 800, 640, 400, mode).unwrap();
        let mut live = vec![0u32; 1280 * 800];
        scene.paint_home_rect(&mut live, 1280, true, "", scene.layout.screen);
        for r in scene.layout.view_regions() {
            scene.paint_rect(&mut live, 1280, true, r);
        }
        let mut window = vec![0u32; 1280 * 800];
        scene.paint(&mut window, 1280, true);
        assert!(live == window, "{mode:?} home -> window");
        for r in scene.layout.view_regions() {
            scene.paint_home_rect(&mut live, 1280, true, "", r);
        }
        let mut home = vec![0u32; 1280 * 800];
        scene.paint_home_rect(&mut home, 1280, true, "", scene.layout.screen);
        assert!(live == home, "{mode:?} window -> home");
    }
}

/// Building with the Home view changes nothing about the S3 window view.
#[test]
fn home_build_keeps_the_window_view_bit_identical() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let a = BaseScene::new(1280, 800, 640, 400, mode).unwrap();
        let b = BaseScene::with_home(1280, 800, 640, 400, mode).unwrap();
        let (mut pa, mut pb) = (vec![0u32; 1280 * 800], vec![0u32; 1280 * 800]);
        a.paint(&mut pa, 1280, true);
        b.paint(&mut pb, 1280, true);
        assert!(pa == pb, "{mode:?}");
    }
}

/// HID usage → Home key, hand-checked against the USB HID Usage Tables
/// (Keyboard/Keypad page 0x07): a=0x04, 1=0x1E, Enter=0x28, Esc=0x29,
/// Backspace=0x2A, Tab=0x2B, Space=0x2C, '/'=0x38, Down=0x51, Up=0x52.
#[test]
fn home_keys_decode_hid_usages() {
    use crate::keys::{Key, decode};
    let ev = |usage: u8, shift: bool, pressed: bool| {
        [
            1,
            pressed as u8,
            usage,
            0,
            if shift { 0x02 } else { 0 },
            0,
            0,
            0,
        ]
    };
    assert_eq!(decode(ev(0x04, false, true)), Some(Key::Char(b'a')));
    assert_eq!(decode(ev(0x04, true, true)), Some(Key::Char(b'A')));
    assert_eq!(decode(ev(0x1e, true, true)), Some(Key::Char(b'!')));
    assert_eq!(decode(ev(0x27, false, true)), Some(Key::Char(b'0')));
    assert_eq!(decode(ev(0x38, true, true)), Some(Key::Char(b'?')));
    assert_eq!(decode(ev(0x34, false, true)), Some(Key::Char(b'\'')));
    assert_eq!(decode(ev(0x2c, false, true)), Some(Key::Char(b' ')));
    assert_eq!(decode(ev(0x28, false, true)), Some(Key::Enter));
    assert_eq!(decode(ev(0x29, false, true)), Some(Key::Escape));
    assert_eq!(decode(ev(0x2a, false, true)), Some(Key::Backspace));
    assert_eq!(decode(ev(0x2b, false, true)), Some(Key::Tab));
    assert_eq!(decode(ev(0x52, false, true)), Some(Key::Up));
    assert_eq!(decode(ev(0x51, false, true)), Some(Key::Down));
    assert_eq!(decode(ev(0x04, false, false)), None, "release");
    assert_eq!(decode(ev(0x32, false, true)), None, "non-US #");
    // Hand-laid INPUT_EVENT_V1 bytes: kind, flags, usage LE, modifiers LE
    // (x), lock state LE (y). Shift+`;` and right Shift+`'`; Caps Lock (y
    // bit 0) on a letter, with Shift, and on a symbol key (no effect).
    assert_eq!(
        decode([1, 1, 0x33, 0, 0x02, 0, 0, 0]),
        Some(Key::Char(b':'))
    );
    assert_eq!(
        decode([1, 1, 0x34, 0, 0x20, 0, 0, 0]),
        Some(Key::Char(b'"'))
    );
    assert_eq!(
        decode([1, 1, 0x04, 0, 0x00, 0, 1, 0]),
        Some(Key::Char(b'A'))
    );
    assert_eq!(
        decode([1, 1, 0x04, 0, 0x02, 0, 1, 0]),
        Some(Key::Char(b'a'))
    );
    assert_eq!(
        decode([1, 1, 0x33, 0, 0x00, 0, 1, 0]),
        Some(Key::Char(b';'))
    );
    // Keypad: Num Lock on (y bit 1 clear) types digits, keypad Enter is
    // Enter; Num Lock off (y = 2) turns keypad 8/2 into Up/Down and keypad 5
    // into nothing; the operators type in both states.
    assert_eq!(decode([1, 1, 0x60, 0, 0, 0, 0, 0]), Some(Key::Char(b'8')));
    assert_eq!(decode([1, 1, 0x58, 0, 0, 0, 0, 0]), Some(Key::Enter));
    assert_eq!(decode([1, 1, 0x60, 0, 0, 0, 2, 0]), Some(Key::Up));
    assert_eq!(decode([1, 1, 0x5a, 0, 0, 0, 2, 0]), Some(Key::Down));
    assert_eq!(decode([1, 1, 0x5d, 0, 0, 0, 2, 0]), None);
    assert_eq!(decode([1, 1, 0x57, 0, 0, 0, 2, 0]), Some(Key::Char(b'+')));
    assert_eq!(
        decode([1, 1, 0x60, 0, 0x02, 0, 0, 0]),
        Some(Key::Up),
        "Shift inverts"
    );
    assert_eq!(decode([2, 1, 4, 0, 0, 0, 0, 0]), None, "not a key event");
    let mut line = crate::keys::Line::<4>::default();
    for k in [
        Key::Char(b'h'),
        Key::Char(b'i'),
        Key::Backspace,
        Key::Char(b'o'),
    ] {
        line.edit(k);
    }
    assert_eq!(line.as_str(), "ho");
    for _ in 0..5 {
        line.edit(Key::Char(b'x'));
    }
    assert_eq!(line.as_str(), "hoxx", "bounded");
    assert!(!line.edit(Key::Enter));
}

// ── Exact labels (Codex S3 MEDIUM) ──────────────────────────────────────────

#[cfg(not(feature = "app-icons"))]
mod label_oracle {
    include!("../../../fixtures/desktop-s3/labels.rs");
}

/// Every pixel outside the five glyph windows must match the independent
/// scene digest, and every pixel inside each window must lie within the
/// exact-area bounds `scripts/desktop/label_oracle.py` derived (neither
/// neo-text nor neo-atrium involved). `Err` names the first failure.
#[cfg(not(feature = "app-icons"))]
fn check_labels(
    buf: &[u32],
    w: usize,
    h: usize,
    digest: u64,
    bounds: &[(&[u32], &[u32])],
) -> Result<(), String> {
    let windows: Vec<Rect> = label_oracle::WINDOWS.iter().map(rect).collect();
    let got = masked_digest(buf, w, h, &windows);
    if got != digest {
        return Err(format!("digest 0x{got:016x} != 0x{digest:016x}"));
    }
    for (i, (win, (lo, hi))) in windows.iter().zip(bounds).enumerate() {
        if let Some((x, y, px, l, hgh)) = window_violation(buf, w, tuple(win), lo, hi) {
            return Err(format!(
                "{} pixel ({x},{y}) 0x{px:06x} outside [0x{l:06x}, 0x{hgh:06x}]",
                label_oracle::RUNS[i]
            ));
        }
    }
    Ok(())
}

/// The S3 labels are exact in both materials (Glass over its real, non-flat
/// frost; RT on flat panels), with only the glyph windows masked from the
/// digest. Negative controls: erasing the "· owned" trust subtitle or the
/// chip's "Arthur" back to the bare base must FAIL.
///
/// The lettered chip only: with `app-icons` the letter is an icon and the
/// name sits lower (`icon_chips_*` below).
#[cfg(not(feature = "app-icons"))]
#[test]
fn labels_match_exact_oracle_and_deletions_fail() {
    use label_oracle::*;
    let (w, h) = (1920, 1080);
    for (mode, digest, bounds) in [
        (Mode::Glass, GLASS_UNFOCUSED_DIGEST, &GLASS_BOUNDS),
        (Mode::ReduceTransparency, RT_UNFOCUSED_DIGEST, &RT_BOUNDS),
    ] {
        let (buf, _) = render(w, h, mode, false);
        check_labels(&buf, w, h, digest, bounds).unwrap_or_else(|e| panic!("{mode:?}: {e}"));
        let scene = BaseScene::new(w, h, 640, 400, mode).unwrap();
        for run in ["TITLE_TRUST", "CHIP_NAME"] {
            let i = RUNS.iter().position(|r| *r == run).unwrap();
            let (x0, y0, ww, wh) = WINDOWS[i];
            let mut erased = buf.clone();
            for y in y0..y0 + wh {
                for x in x0..x0 + ww {
                    erased[y * w + x] = crate::wide::dither(scene.wide_pixel(x, y, false), x, y);
                }
            }
            let err = check_labels(&erased, w, h, digest, bounds)
                .expect_err(&format!("{mode:?}: deleting {run} must fail"));
            assert!(err.starts_with(run), "{mode:?} {run}: {err}");
        }
    }
}

// ── Codex DS-1a F2: the Home intent field, exact ────────────────────────────

mod field_oracle {
    include!("../../../fixtures/desktop-ds1/field_text.rs");
}

/// The first field-oracle state whose digest or window bounds `buf` (a
/// 1920×1080 Glass Home view) fails, if any.
fn field_failure(buf: &[u32], l: &Layout, state: usize) -> Option<String> {
    use field_oracle::*;
    let (wx, wy, ww, wh) = WINDOWS[state];
    let masks = [l.bar_label, l.chip_label, Rect::new(wx, wy, ww, wh)];
    let digest = masked_digest(buf, 1920, 1080, &masks);
    if digest != DIGESTS[state] {
        return Some(format!("{} digest 0x{digest:016x}", STATES[state].0));
    }
    let (lo, hi) = BOUNDS[state];
    window_violation(buf, 1920, WINDOWS[state], lo, hi).map(|(x, y, got, l, h)| {
        format!(
            "{} pixel ({x},{y}) = 0x{got:06x} outside [0x{l:06x}, 0x{h:06x}]",
            STATES[state].0
        )
    })
}

/// Every field state — the placeholder with its caret, a typed intent with
/// and without the caret, a long intent scrolled left — equals the
/// independent field oracle: only its tight window is masked from the Home
/// digest, and every window pixel lies within the exact-area bounds (the
/// caret exact). Negative controls: no caret, a wrong letter, a shifted run,
/// a missing placeholder and the long intent unscrolled each fail.
#[test]
fn field_matches_exact_oracle_and_mutations_fail() {
    use field_oracle::*;
    let (w, h) = (1920, 1080);
    let scene = BaseScene::with_home(w, h, 640, 400, Mode::Glass).unwrap();
    let l = scene.layout;
    let paint = |text: &str, focused: bool| {
        let mut buf = vec![0u32; w * h];
        scene.paint_home_rect(&mut buf, w, focused, text, l.screen);
        buf
    };
    for (i, &(_, text, focused)) in STATES.iter().enumerate() {
        assert_eq!(field_failure(&paint(text, focused), &l, i), None);
    }
    // EMPTY_FOCUSED (0): the caret removed; the placeholder replaced.
    assert!(
        field_failure(&paint("", false), &l, 0).is_some(),
        "no caret must fail"
    );
    assert!(
        field_failure(&paint(" ", true), &l, 0).is_some(),
        "no placeholder must fail"
    );
    // TYPED_FOCUSED (1): one wrong letter; the run shifted by a space.
    assert!(field_failure(&paint("hello desktoq", true), &l, 1).is_some());
    assert!(field_failure(&paint(" hello desktop", true), &l, 1).is_some());
    // SCROLLED_FOCUSED (3): a different ending; the intent not scrolled
    // (its first word kept visible) is the short-text rule misapplied.
    let mut other = String::from(LONG_INTENT);
    other.replace_range(other.len() - 1.., "a");
    assert!(field_failure(&paint(&other, true), &l, 3).is_some());
    let unscrolled = &LONG_INTENT[..60];
    assert!(field_failure(&paint(unscrolled, true), &l, 3).is_some());
    // The caret shows only while the field has the keyboard: not while the
    // keyboard is on the Spine chip (spec v1.3.1).
    let mut chip = vec![0u32; w * h];
    let view = crate::View::Home {
        focused: true,
        held: false,
        chip_selected: true,
    };
    scene.paint_home_view(&mut chip, w, view, "", l.screen);
    let caret_x = l.field_text.x;
    let caret_blue = (l.field.y + 16..l.field.y + 40)
        .any(|y| chip[y * w + caret_x] == crate::material::ELECTRIC_BLUE);
    assert!(!caret_blue, "no caret while the chip is selected");
    let lit = paint("", true);
    assert!(
        (l.field.y + 16..l.field.y + 40)
            .all(|y| lit[y * w + caret_x] == crate::material::ELECTRIC_BLUE),
        "the probe sees the caret when the field has the keyboard"
    );
}

/// DI-1a (Jarvis, #46 RETURN F2), through the WM's own decision function:
/// a press grabs; its release is lost; the user presses again and the next
/// observable record is a RESYNC with the button held. That record must
/// cancel the old grab (never forwarded to the old target) and must not
/// count as a fresh press; the grab ends when every button is up.
#[test]
fn resync_with_a_button_held_cancels_a_stale_grab() {
    use crate::pointer_grab::{RESYNC, decide};
    // Press: a fresh press, no grab yet.
    let d = decide(1, 0, false);
    assert!(d.pressed && !d.cancel_grab && !d.release);
    // (the release was lost) Held again, RESYNC, grab still held from before.
    let d = decide(RESYNC | 1, 1, true);
    assert!(!d.pressed, "a RESYNC record is never a fresh press");
    assert!(
        d.cancel_grab,
        "the stale grab must be cancelled before routing"
    );
    assert!(!d.release);
    // Up: the (now consumed) grab ends.
    let d = decide(0, 1, true);
    assert!(d.release && !d.cancel_grab);
}

/// A RESYNC record with every button up keeps the grab: it is the release
/// the grabbed client missed, so it still reaches that client, then ends.
#[test]
fn resync_with_buttons_up_reconciles_the_grabbed_client() {
    use crate::pointer_grab::{RESYNC, decide};
    let d = decide(RESYNC, 1, true);
    assert!(!d.cancel_grab && d.release && !d.pressed);
    // Without a grab, nothing to cancel either way.
    assert!(!decide(RESYNC | 1, 0, false).cancel_grab);
    // A plain held record (no RESYNC) never cancels.
    assert!(!decide(1, 1, true).cancel_grab);
}

// ── DS-1c Lens ──────────────────────────────────────────────────────────────

mod lens_digests {
    include!("../../../fixtures/desktop-ds1/lens_digests.rs");
}

fn lens_mask_boxes(l: &Layout) -> Vec<Rect> {
    let lens = l.lens.unwrap();
    let mut boxes = l.label_boxes().to_vec();
    boxes.extend([l.chip2_label, lens.title_label, l.client, lens.client]);
    boxes
}

/// The window view with the Lens bloomed (focused) and the Home view with
/// Arthur held and the Lens alive equal the independent Python oracle
/// outside the mask boxes (labels, and both clients' areas in the window).
#[test]
fn lens_matches_independent_python_oracle() {
    use lens_digests::*;
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    let boxes: Vec<_> = lens_mask_boxes(&l).iter().map(tuple).collect();
    assert_eq!(boxes, DS1C_LENS_MASK_BOXES_1920X1080);
    for (mode, window, home) in [
        (
            Mode::Glass,
            DS1C_WINDOW_LENS_GLASS_1920X1080_FOCUSED,
            DS1C_HOME_LENS_GLASS_1920X1080_HELD,
        ),
        (
            Mode::ReduceTransparency,
            DS1C_WINDOW_LENS_RT_1920X1080_FOCUSED,
            DS1C_HOME_LENS_RT_1920X1080_HELD,
        ),
    ] {
        let (w, h) = (1920, 1080);
        let mut scene = BaseScene::with_lens(w, h, 640, 400, mode).unwrap();
        assert!(scene.set_lens(true));
        let mut buf = vec![0u32; w * h];
        scene.paint(&mut buf, w, true);
        composite_glow(&scene, &mut buf, w, true);
        assert_eq!(
            masked_digest(&buf, w, h, &lens_mask_boxes(&scene.layout)),
            window,
            "{mode:?} window+lens"
        );
        let view = crate::View::Home {
            focused: true,
            held: true,
            chip_selected: false,
        };
        scene.paint_home_view(&mut buf, w, view, "masked", scene.layout.screen);
        let mut hb = scene.layout.home_label_boxes().to_vec();
        hb.push(scene.layout.chip2_label);
        assert_eq!(
            masked_digest(&buf, w, h, &hb),
            home,
            "{mode:?} home held+lens"
        );
    }
}

/// Spec v1.5: the selection switched to the Lens equals the oracle — the
/// window view with Arthur unfocused and the Lens chip lit, and Home with the
/// keyboard on the Spine at the Lens chip. Unselected it is v1.4 bit for bit,
/// and only a shown Lens can be selected (ending it clears the selection).
#[test]
fn lens_selection_matches_independent_python_oracle() {
    use lens_digests::*;
    let (w, h) = (1920, 1080);
    for (mode, window, home) in [
        (
            Mode::Glass,
            DS1C_WINDOW_LENS_GLASS_1920X1080_SELECTED,
            DS1C_HOME_LENS_GLASS_1920X1080_CHIP_SELECTED,
        ),
        (
            Mode::ReduceTransparency,
            DS1C_WINDOW_LENS_RT_1920X1080_SELECTED,
            DS1C_HOME_LENS_RT_1920X1080_CHIP_SELECTED,
        ),
    ] {
        let mut scene = BaseScene::with_lens(w, h, 640, 400, mode).unwrap();
        assert!(!scene.set_lens_selected(true), "no Lens, no selection");
        assert!(scene.set_lens(true));
        let mut unselected = vec![0u32; w * h];
        scene.paint(&mut unselected, w, false);
        assert!(scene.set_lens_selected(true));
        let mut buf = vec![0u32; w * h];
        scene.paint(&mut buf, w, false);
        composite_glow(&scene, &mut buf, w, false);
        assert_eq!(
            masked_digest(&buf, w, h, &lens_mask_boxes(&scene.layout)),
            window,
            "{mode:?} window, Lens selected"
        );
        // The selection repaints only the Spine (it is in every focus repaint).
        let mut moved = unselected.clone();
        scene.paint_rect(&mut moved, w, false, scene.layout.spine);
        let mut full = vec![0u32; w * h];
        scene.paint(&mut full, w, false);
        assert!(moved == full, "the selection must stay inside the Spine");
        assert!(moved != unselected, "the selection must be visible");
        let view = crate::View::Home {
            focused: true,
            held: true,
            chip_selected: true,
        };
        scene.paint_home_view(&mut buf, w, view, "masked", scene.layout.screen);
        let mut hb = scene.layout.home_label_boxes().to_vec();
        hb.push(scene.layout.chip2_label);
        assert_eq!(
            masked_digest(&buf, w, h, &hb),
            home,
            "{mode:?} home, chip at the Lens"
        );
        scene.set_lens(false);
        assert!(
            !scene.lens_selected(),
            "ending the Lens clears its selection"
        );
    }
}

fn composite_glow(scene: &BaseScene, buf: &mut [u32], w: usize, focused: bool) {
    let g = scene.layout.glow;
    for y in g.y..g.bottom() {
        for x in g.x..g.right() {
            let s = crate::scene::glow_pixel(&scene.layout, focused, x, y);
            let a = s >> 24;
            if a != 0 {
                let d = buf[y * w + x];
                let ch =
                    |sh: u32| (((s >> sh) & 0xFF) * a + ((d >> sh) & 0xFF) * (255 - a) + 127) / 255;
                buf[y * w + x] = (ch(16) << 16) | (ch(8) << 8) | ch(0);
            }
        }
    }
}

/// Blooming the Lens (and ending it) repaints only `lens_regions()` and lands
/// exactly on a full paint; the Lens off is the DS-1a/1b scene bit for bit;
/// there is no Lens without room for one (1280×800) or without Home.
#[test]
fn lens_bloom_damage_and_kill_switch() {
    let (w, h) = (1920, 1080);
    let mut scene = BaseScene::with_lens(w, h, 640, 400, Mode::Glass).unwrap();
    let mut before = vec![0u32; w * h];
    scene.paint(&mut before, w, true);
    let plain = BaseScene::new(w, h, 640, 400, Mode::Glass).unwrap();
    let mut s3 = vec![0u32; w * h];
    plain.paint(&mut s3, w, true);
    assert!(before == s3, "Lens off must be the S3 window view");

    let mut bloomed = before.clone();
    assert!(scene.set_lens(true));
    for r in scene.layout.lens_regions() {
        scene.paint_rect(&mut bloomed, w, true, r);
    }
    let mut full = vec![0u32; w * h];
    scene.paint(&mut full, w, true);
    assert!(bloomed == full, "bloom damage must cover every Lens pixel");
    assert!(bloomed != before, "the Lens must be visible");

    scene.set_lens(false);
    for r in scene.layout.lens_regions() {
        scene.paint_rect(&mut bloomed, w, true, r);
    }
    assert!(bloomed == before, "ending the Lens restores the scene");

    let mut small = BaseScene::with_lens(1280, 800, 640, 400, Mode::Glass).unwrap();
    assert!(small.layout.lens.is_none());
    assert!(!small.set_lens(true));
    let mut no_home = BaseScene::new(w, h, 640, 400, Mode::Glass).unwrap();
    assert!(!no_home.set_lens(true));
    // DS-1c off: a Home scene built without the Lens can never show one, and
    // its layout carries none (its view switches repaint no Lens region).
    let mut ds1b = BaseScene::with_home(w, h, 640, 400, Mode::Glass).unwrap();
    assert!(ds1b.layout.lens.is_none());
    assert!(!ds1b.set_lens(true));
    assert!(ds1b.layout.view_regions()[4].is_empty());
}

mod move_digests {
    include!("../../../fixtures/desktop-di1b/move_digests.rs");
}

/// DI-1b: a scene whose frame was moved by a title-band drag equals the
/// independent Python oracle rendered with the frame at that place, bit for
/// bit outside the (moved) label boxes, in both materials and focus states.
#[test]
fn moved_scene_matches_independent_python_oracle() {
    use move_digests::*;
    for (at, boxes, digests) in [
        (
            (500, 200),
            MOVED_500_200_LABEL_BOXES_1920X1080,
            [
                MOVED_500_200_GLASS_1920X1080_UNFOCUSED,
                MOVED_500_200_GLASS_1920X1080_FOCUSED,
                MOVED_500_200_RT_1920X1080_UNFOCUSED,
                MOVED_500_200_RT_1920X1080_FOCUSED,
            ],
        ),
        (
            (98, 50),
            MOVED_98_50_LABEL_BOXES_1920X1080,
            [
                MOVED_98_50_GLASS_1920X1080_UNFOCUSED,
                MOVED_98_50_GLASS_1920X1080_FOCUSED,
                MOVED_98_50_RT_1920X1080_UNFOCUSED,
                MOVED_98_50_RT_1920X1080_FOCUSED,
            ],
        ),
    ] {
        let mut i = 0;
        for mode in [Mode::Glass, Mode::ReduceTransparency] {
            for focused in [false, true] {
                let mut scene = BaseScene::new(1920, 1080, 640, 400, mode).unwrap();
                assert!(scene.move_frame(at.0, at.1));
                assert_eq!((scene.layout.frame.x, scene.layout.frame.y), at);
                let got = scene.layout.label_boxes().map(|r| tuple(&r));
                assert_eq!(got, boxes);
                let (buf, l) = render_scene(&scene, focused);
                assert_eq!(
                    masked_digest(&buf, 1920, 1080, &l.label_boxes()),
                    digests[i],
                    "{at:?} {mode:?} focused={focused}"
                );
                i += 1;
            }
        }
    }
}

/// DI-1b: moving the frame equals building the scene with the frame there
/// (both views, every pixel, labels included), so a move leaves nothing of
/// the old place behind and touches nothing frame-independent.
#[test]
fn moved_scene_equals_scene_built_there() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let mut moved = BaseScene::with_home(1280, 800, 640, 400, mode).unwrap();
        for at in [(300, 120), (98, 50), (300, 120)] {
            assert!(moved.move_frame(at.0, at.1));
            let built = BaseScene::with_home_at(1280, 800, 640, 400, mode, at).unwrap();
            assert_eq!(moved.layout, built.layout);
            for focused in [false, true] {
                let mut a = vec![0u32; 1280 * 800];
                let mut b = vec![0u32; 1280 * 800];
                moved.paint(&mut a, 1280, focused);
                built.paint(&mut b, 1280, focused);
                assert!(a == b, "window view {at:?} {mode:?} focused={focused}");
                let full = moved.layout.screen;
                moved.paint_home_view(&mut a, 1280, crate::scene::View::home(focused), "", full);
                built.paint_home_view(&mut b, 1280, crate::scene::View::home(focused), "", full);
                assert!(a == b, "home view {at:?} {mode:?} focused={focused}");
            }
        }
    }
}

/// DI-1b: a completed move costs exactly one region blur in Glass (the
/// frame's glass at its new place); a move that lands where the frame
/// already is costs none, and Reduce-Transparency never blurs.
#[test]
fn a_move_blurs_exactly_once() {
    let mut glass = BaseScene::with_home(1280, 800, 640, 400, Mode::Glass).unwrap();
    let before = blur_calls();
    assert!(glass.move_frame(300, 120));
    assert_eq!(blur_calls() - before, 1);
    assert!(!glass.move_frame(300, 120));
    // Far outside: clamps to the corner, one move.
    assert!(glass.move_frame(0, 0));
    assert!(!glass.move_frame(10, 10));
    assert_eq!(blur_calls() - before, 2);
    let mut rt = BaseScene::with_home(1280, 800, 640, 400, Mode::ReduceTransparency).unwrap();
    let before = blur_calls();
    assert!(rt.move_frame(300, 120));
    assert_eq!(blur_calls(), before);
}

/// DI-1b: the frame and its focus light stay inside the workfield. At
/// 1920×1080 with a 640×400 client the frame is 656×442, so the origin
/// range is x ∈ [84+14, 1920−14−656] = [98, 1250] and
/// y ∈ [36+14, 1080−14−442] = [50, 624]; the centred origin is unchanged.
#[test]
fn frame_origin_is_clamped_to_the_workfield() {
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    assert_eq!(
        (l.frame.x, l.frame.y, l.frame.w, l.frame.h),
        (674, 337, 656, 442)
    );
    assert_eq!(l.clamp_frame(-500, -500), (98, 50));
    assert_eq!(l.clamp_frame(5000, 5000), (1250, 624));
    assert_eq!(l.clamp_frame(500, 200), (500, 200));
    let p = Layout::placed(1920, 1080, 640, 400, 674, 337).unwrap();
    assert_eq!(p, l);
    let corner = Layout::placed(1920, 1080, 640, 400, 0, 2000).unwrap();
    assert_eq!((corner.frame.x, corner.frame.y), (98, 624));
    assert!(corner.glow.x >= corner.workfield.x);
    assert!(corner.glow.bottom() <= corner.screen.bottom());
    assert_eq!(l.title_band(), Rect::new(674, 337, 656, 34));
    assert_eq!(l.client.y, l.title_band().bottom());
}

mod resize_digests {
    include!("../../../fixtures/desktop-di1b/resize_digests.rs");
}

/// DI-1b resize: a scene resized by an edge/corner drag (through
/// [`Layout::resized`], as the WM does) equals the independent Python oracle
/// rendered with that frame and client, bit for bit outside the label boxes,
/// in both materials and focus states: the centred frame's corner dragged by
/// (+120, +60); the largest client at the clamp corner; the smallest.
#[test]
fn resized_scene_matches_independent_python_oracle() {
    use crate::layout::edge::{BOTTOM, RIGHT};
    use resize_digests::*;
    for (start, drag, want, boxes, digests) in [
        (
            None,
            (120, 60),
            (674, 337, 760, 460),
            RESIZED_674_337_760X460_LABEL_BOXES_1920X1080,
            [
                RESIZED_674_337_760X460_GLASS_1920X1080_UNFOCUSED,
                RESIZED_674_337_760X460_GLASS_1920X1080_FOCUSED,
                RESIZED_674_337_760X460_RT_1920X1080_UNFOCUSED,
                RESIZED_674_337_760X460_RT_1920X1080_FOCUSED,
            ],
        ),
        (
            Some((98, 50)),
            (5000, 5000),
            (98, 50, 1280, 800),
            RESIZED_98_50_1280X800_LABEL_BOXES_1920X1080,
            [
                RESIZED_98_50_1280X800_GLASS_1920X1080_UNFOCUSED,
                RESIZED_98_50_1280X800_GLASS_1920X1080_FOCUSED,
                RESIZED_98_50_1280X800_RT_1920X1080_UNFOCUSED,
                RESIZED_98_50_1280X800_RT_1920X1080_FOCUSED,
            ],
        ),
        (
            None,
            (-5000, -5000),
            (674, 337, 320, 200),
            RESIZED_674_337_320X200_LABEL_BOXES_1920X1080,
            [
                RESIZED_674_337_320X200_GLASS_1920X1080_UNFOCUSED,
                RESIZED_674_337_320X200_GLASS_1920X1080_FOCUSED,
                RESIZED_674_337_320X200_RT_1920X1080_UNFOCUSED,
                RESIZED_674_337_320X200_RT_1920X1080_FOCUSED,
            ],
        ),
    ] {
        let mut i = 0;
        for mode in [Mode::Glass, Mode::ReduceTransparency] {
            for focused in [false, true] {
                let mut scene = BaseScene::new(1920, 1080, 640, 400, mode).unwrap();
                if let Some((x, y)) = start {
                    assert!(scene.move_frame(x, y));
                }
                let r = scene.layout.resized(RIGHT | BOTTOM, drag.0, drag.1);
                assert_eq!(r, want);
                assert!(scene.place_frame(r.0, r.1, r.2, r.3));
                let c = scene.layout.client;
                assert_eq!((scene.layout.frame.x, scene.layout.frame.y, c.w, c.h), want);
                let got = scene.layout.label_boxes().map(|r| tuple(&r));
                assert_eq!(got, boxes);
                let (buf, l) = render_scene(&scene, focused);
                assert_eq!(
                    masked_digest(&buf, 1920, 1080, &l.label_boxes()),
                    digests[i],
                    "{want:?} {mode:?} focused={focused}"
                );
                i += 1;
            }
        }
    }
}

/// DI-1b resize: resizing equals building the scene with that client and
/// frame (both views, every pixel, labels included), through a grow, a
/// shrink from the left/top (the origin moves) and back to the start.
#[test]
fn resized_scene_equals_scene_built_there() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let mut resized = BaseScene::with_home(1280, 800, 640, 400, mode).unwrap();
        let start = resized.layout;
        for (x, y, cw, ch) in [
            (start.frame.x, start.frame.y, 900, 520),
            (400, 250, 480, 300),
            (start.frame.x, start.frame.y, 640, 400),
        ] {
            assert!(resized.place_frame(x, y, cw, ch));
            let built = BaseScene::with_home_at(1280, 800, cw, ch, mode, (x, y)).unwrap();
            assert_eq!(resized.layout, built.layout);
            for focused in [false, true] {
                let mut a = vec![0u32; 1280 * 800];
                let mut b = vec![0u32; 1280 * 800];
                resized.paint(&mut a, 1280, focused);
                built.paint(&mut b, 1280, focused);
                assert!(a == b, "window view {cw}x{ch} {mode:?} focused={focused}");
                let full = resized.layout.screen;
                let home = crate::scene::View::home(focused);
                resized.paint_home_view(&mut a, 1280, home, "", full);
                built.paint_home_view(&mut b, 1280, home, "", full);
                assert!(a == b, "home view {cw}x{ch} {mode:?} focused={focused}");
                // The strided focus-light plane equals the tight one.
                let g = resized.layout.glow;
                let mut tight = vec![0u32; g.w * g.h];
                let mut wide = vec![0u32; (g.w + 7) * g.h];
                resized.fill_glow(&mut tight, focused);
                resized.fill_glow_strided(&mut wide, g.w + 7, focused);
                for y in 0..g.h {
                    assert_eq!(tight[y * g.w..][..g.w], wide[y * (g.w + 7)..][..g.w]);
                }
            }
        }
    }
}

/// DI-1b resize: a completed resize costs exactly one region blur in Glass
/// (the frame's glass at its new size), the same as a move; a resize to the
/// current frame costs none, and Reduce-Transparency never blurs.
#[test]
fn a_resize_blurs_exactly_once() {
    let mut glass = BaseScene::with_home(1280, 800, 640, 400, Mode::Glass).unwrap();
    let f = glass.layout.frame;
    let before = blur_calls();
    assert!(glass.place_frame(f.x, f.y, 700, 450));
    assert_eq!(blur_calls() - before, 1);
    assert!(!glass.place_frame(f.x, f.y, 700, 450));
    assert_eq!(blur_calls() - before, 1);
    let mut rt = BaseScene::with_home(1280, 800, 640, 400, Mode::ReduceTransparency).unwrap();
    let before = blur_calls();
    assert!(rt.place_frame(f.x, f.y, 700, 450));
    assert_eq!(blur_calls(), before);
}

/// DI-1b resize handles: the side and bottom borders, the top rows of the
/// title band, and the corners; never the client or the rest of the band.
/// Centred 640×400 at 1920×1080: frame (674, 337, 656, 442).
#[test]
fn resize_handles_cover_the_borders_and_corners_only() {
    use crate::layout::edge::{BOTTOM, LEFT, RIGHT, TOP};
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    let (x0, y0, x1, y1) = (674, 337, 674 + 656, 337 + 442);
    assert_eq!(l.resize_edges(x0, 500), LEFT);
    assert_eq!(l.resize_edges(x0 + 7, 500), LEFT);
    assert_eq!(l.resize_edges(x0 + 8, 500), 0, "the client's first column");
    assert_eq!(l.resize_edges(x1 - 1, 500), RIGHT);
    assert_eq!(l.resize_edges(1000, y1 - 1), BOTTOM);
    assert_eq!(l.resize_edges(1000, y1 - 9), 0, "the client's last row");
    assert_eq!(l.resize_edges(1000, y0), TOP);
    assert_eq!(l.resize_edges(1000, y0 + 3), TOP);
    assert_eq!(l.resize_edges(1000, y0 + 4), 0, "the title band moves");
    assert_eq!(l.resize_edges(x0, y0), LEFT | TOP);
    assert_eq!(l.resize_edges(x0 + 15, y0 + 1), LEFT | TOP);
    assert_eq!(l.resize_edges(x0 + 2, y0 + 15), LEFT | TOP);
    assert_eq!(l.resize_edges(x1 - 1, y1 - 1), RIGHT | BOTTOM);
    assert_eq!(l.resize_edges(x1 - 16, y1 - 1), RIGHT | BOTTOM);
    assert_eq!(l.resize_edges(x1 - 17, y1 - 1), BOTTOM);
    assert_eq!(l.resize_edges(x0, y1 - 1), LEFT | BOTTOM);
    assert_eq!(l.resize_edges(x1 - 1, y0), RIGHT | TOP);
    assert_eq!(l.resize_edges(x0 - 1, 500), 0, "outside the frame");
    assert_eq!(l.resize_edges(x1, 500), 0);
    for y in l.client.y..l.client.bottom() {
        for x in [l.client.x, l.client.right() - 1] {
            assert_eq!(l.resize_edges(x, y), 0);
        }
    }
}

/// DI-1b resize geometry: a dragged edge moves by its delta and the
/// opposite edge never moves; the client stays within 320×200 and the
/// output's maximum (1280×800 at 1920×1080; less at 1280×800, where the
/// frame must still fit), and the frame plus its focus light stay inside the
/// workfield.
#[test]
fn resize_geometry_is_bounded_and_anchors_the_opposite_edge() {
    use crate::layout::edge::{BOTTOM, LEFT, RIGHT, TOP};
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    assert_eq!(l.max_client(), (1280, 800));
    assert_eq!(l.resized(0, 50, 50), (674, 337, 640, 400));
    // Left edge out by 100: the origin moves, the right edge stays.
    assert_eq!(l.resized(LEFT, -100, 0), (574, 337, 740, 400));
    assert_eq!(l.resized(TOP, 0, -100), (674, 237, 640, 500));
    // Past the minimum from either side: stops at 320×200, anchored.
    assert_eq!(l.resized(LEFT, 5000, 0), (674 + 320, 337, 320, 400));
    assert_eq!(l.resized(TOP | LEFT, 5000, 5000), (994, 537, 320, 200));
    // Past the workfield to the left: the frame's left stops at x = 98.
    let (x, _, w, _) = l.resized(LEFT, -5000, 0);
    assert_eq!((x, x + w + 16), (98, 674 + 656));
    // Grown right/down to the workfield edge (glow inside): right ≤ 1906.
    let (x, y, w, h) = l.resized(RIGHT | BOTTOM, 5000, 5000);
    assert_eq!((x, y), (674, 337));
    assert_eq!(x + w + 16, 1906);
    assert_eq!(y + h + 42, 1066);
    let small = Layout::new(1280, 800, 640, 400).unwrap();
    let (mw, mh) = small.max_client();
    assert_eq!((mw, mh), (1280 - 84 - 60 - 16, 800 - 36 - 60 - 42));
    for edges in [LEFT | TOP, RIGHT | BOTTOM, LEFT | BOTTOM, RIGHT | TOP] {
        for (dx, dy) in [(-5000, -5000), (5000, 5000), (-5000, 5000), (5000, -5000)] {
            let (x, y, w, h) = small.resized(edges, dx, dy);
            assert!((320..=mw).contains(&w) && (200..=mh).contains(&h));
            let placed = Layout::placed(1280, 800, w, h, x, y).expect("fits");
            assert_eq!((placed.frame.x, placed.frame.y), (x, y), "unclamped");
            assert!(placed.glow.x >= placed.workfield.x);
            assert!(placed.glow.right() <= placed.workfield.right());
            assert!(placed.glow.y >= placed.workfield.y);
            assert!(placed.glow.bottom() <= placed.workfield.bottom());
        }
    }
}

/// DS-1c x DI-1b, DL-1: the Lens sits beside the frame, docked. A shown
/// Lens follows a move (the frame's blur plus the Lens's: two), and the moved
/// scene equals one built there with its Lens shown; a place the Lens would
/// not fit beside is refused. With the Lens down a move costs its one blur and
/// the next Lens shown re-blurs at the new place exactly once; a scene that
/// cannot show a Lens never gains its geometry.
#[test]
fn a_shown_lens_follows_the_window_and_a_moved_lens_reblurs_once() {
    use crate::layout::{LENS_GAP, LENS_H, LENS_W};
    let (w, h) = (1920, 1080);
    let mut glass = BaseScene::with_lens(w, h, 640, 400, Mode::Glass).unwrap();
    assert!(glass.set_lens(true));
    let b0 = blur_calls();
    assert!(glass.move_frame(300, 200));
    assert_eq!(
        blur_calls() - b0,
        2,
        "a docked move blurs the frame and the Lens"
    );
    let f = glass.layout.frame;
    let lens = glass.layout.lens.expect("the Lens stays docked");
    assert_eq!((lens.frame.x, lens.frame.y), (f.right() + LENS_GAP, f.y));
    assert_eq!((lens.client.w, lens.client.h), (LENS_W, LENS_H));
    let mut built = BaseScene::with_lens_at(w, h, 640, 400, Mode::Glass, (300, 200)).unwrap();
    assert!(built.set_lens(true));
    assert_eq!(glass.layout, built.layout);
    let (mut a, mut b) = (vec![0u32; w * h], vec![0u32; w * h]);
    glass.paint(&mut a, w, true);
    built.paint(&mut b, w, true);
    assert!(a == b, "a moved docked scene must equal one built there");
    // Where the Lens would not fit: refused, nothing changes.
    let before = (glass.layout, blur_calls());
    assert!(!glass.move_frame(1200, 200));
    assert_eq!((glass.layout, blur_calls()), before);
    // With the Lens down the window may go there (the Lens has no room).
    assert!(glass.set_lens(false));
    let b1 = blur_calls();
    assert!(glass.move_frame(1200, 200));
    assert_eq!(blur_calls() - b1, 1, "a move blurs the frame only");
    assert_eq!(glass.layout.lens, None);
    assert!(!glass.set_lens(true), "no room: the WM slides first");
    assert!(glass.move_frame(300, 200));
    assert!(glass.set_lens(true));
    assert_eq!(blur_calls() - b1, 3, "the Lens re-blurs at its new place");
    assert!(glass.set_lens(false));
    assert!(glass.set_lens(true));
    assert_eq!(blur_calls() - b1, 3, "and only once");
    let mut plain = BaseScene::with_home(w, h, 640, 400, Mode::Glass).unwrap();
    assert!(plain.move_frame(300, 200));
    assert_eq!(plain.layout.lens, None);
    assert!(!plain.set_lens(true));
}

// ── DL-1 docked Lens ────────────────────────────────────────────────────────

mod dock_digests {
    include!("../../../fixtures/desktop-dl1/dock_digests.rs");
}

/// DL-1: a window with the Lens docked, moved and resized, equals the
/// independent oracle (spec v1.6) outside the mask boxes, in both materials.
#[test]
fn docked_lens_matches_independent_python_oracle() {
    use dock_digests::*;
    let (w, h) = (1920, 1080);
    for (i, &(fx, fy, cw, ch)) in DOCK_CASES.iter().enumerate() {
        for (mode, want) in [
            (Mode::Glass, DOCK_GLASS_FOCUSED[i]),
            (Mode::ReduceTransparency, DOCK_RT_FOCUSED[i]),
        ] {
            let mut scene = BaseScene::with_lens_at(w, h, cw, ch, mode, (fx, fy)).unwrap();
            assert!(scene.set_lens(true), "case {i}: the Lens fits");
            let lens = scene.layout.lens.unwrap();
            let l = &scene.layout;
            let boxes = [
                l.bar_label,
                l.title_label,
                l.chip_label,
                l.chip2_label,
                lens.title_label,
                l.client,
                lens.client,
            ];
            let as_tuple: Vec<_> = boxes.iter().map(tuple).collect();
            assert_eq!(as_tuple, DOCK_MASKS[i], "case {i} mask boxes");
            let mut buf = vec![0u32; w * h];
            scene.paint(&mut buf, w, true);
            composite_glow(&scene, &mut buf, w, true);
            assert_eq!(
                masked_digest(&buf, w, h, &boxes),
                want,
                "case {i} {mode:?} docked"
            );
        }
    }
}

/// DL-1: the docked group's clamps equal the spec's, written independently
/// (`scene_oracle.py`): moves, bloom origins and resizes. The same requests
/// undocked land elsewhere (the Lens is what bounds them).
#[test]
fn docked_group_clamps_match_the_spec() {
    use crate::layout::{PAD, TITLE_H};
    use dock_digests::*;
    let (w, h) = (1920, 1080);
    let docked = |x: usize, y: usize, cw: usize, ch: usize| {
        let mut l = Layout::placed(w, h, cw, ch, x, y).unwrap();
        assert!(l.lens.is_some(), "the Lens fits at {x},{y}");
        l.lens_docked = true;
        l
    };
    for &((fw, fh, x, y), want) in &DOCK_MOVES {
        let l = docked(98, 50, fw - 2 * PAD, fh - TITLE_H - PAD);
        assert_eq!(l.clamp_frame(x, y), want, "move {fw}x{fh} to {x},{y}");
        let (lx, ly) = want;
        let landed = docked(lx, ly, fw - 2 * PAD, fh - TITLE_H - PAD);
        assert_eq!(
            (landed.frame.x, landed.frame.y),
            want,
            "the Lens still fits"
        );
    }
    let mut free = Layout::placed(w, h, 640, 400, 98, 50).unwrap();
    assert_eq!(
        free.clamp_frame(974, 397),
        (974, 397),
        "undocked: the frame alone"
    );
    free.lens_docked = true;
    assert_eq!(free.clamp_frame(974, 397), (826, 397));
    for &((x, y, cw, ch), want) in &BLOOM_ORIGINS {
        let l = Layout::placed(w, h, cw, ch, x, y).unwrap();
        assert_eq!(l.bloom_origin(), want, "bloom from {x},{y} {cw}x{ch}");
        if let Some((bx, by)) = want {
            let at = Layout::placed(w, h, cw, ch, bx, by).unwrap();
            assert!(at.lens.is_some(), "a Lens fits at the bloom origin");
            assert_eq!(
                l.lens.is_some(),
                (bx, by) == (x, y),
                "slide only when needed"
            );
        }
    }
    for &((x, y, cw, ch, edges, dx, dy), want) in &DOCK_RESIZES {
        let l = docked(x, y, cw, ch);
        let got = l.resized(edges, dx, dy);
        assert_eq!(
            got, want,
            "resize {edges} by {dx},{dy} from {x},{y} {cw}x{ch}"
        );
        let (rx, ry, rw, rh) = got;
        assert!(
            Layout::placed(w, h, rw, rh, rx, ry).unwrap().lens.is_some(),
            "the Lens still fits after the resize"
        );
    }
    let undocked = Layout::placed(w, h, 640, 400, 826, 397).unwrap();
    assert_eq!(
        undocked.resized(crate::layout::edge::RIGHT, 500, 0),
        (826, 397, 1064, 400),
        "undocked: the right edge runs to the workfield"
    );
}

/// DL-1 kill-switch: a scene that never shows a Lens (DI-1b) clamps and
/// resizes exactly as before, and the docked flag without a Lens is inert.
#[test]
fn undocked_layout_is_di1b_exactly() {
    let (w, h) = (1920, 1080);
    let mut plain = BaseScene::with_home(w, h, 640, 400, Mode::Glass).unwrap();
    assert!(!plain.layout.lens_docked);
    assert_eq!(plain.layout.clamp_frame(5000, 5000), (1250, 624));
    assert!(plain.move_frame(1250, 624));
    assert_eq!(plain.layout.docked_reach(), Rect::default());
    let mut no_lens = plain.layout;
    no_lens.lens_docked = true;
    assert_eq!(no_lens.clamp_frame(0, 0), plain.layout.clamp_frame(0, 0));
    assert_eq!(
        no_lens.resized(crate::layout::edge::TOP, 0, 300),
        plain.layout.resized(crate::layout::edge::TOP, 0, 300)
    );
}

/// DL-1 AAG: a configure published before the Lens bloomed (undocked bounds,
/// pre-slide origin) is re-placed where the docked group fits at that size.
#[test]
fn stale_configure_is_replaced_inside_the_docked_group() {
    let (w, h) = (1920, 1080);
    let mut l = Layout::placed(w, h, 640, 400, 826, 397).unwrap();
    l.lens_docked = true;
    // Undocked, a right-edge drag could have reached x 1906 (client 1064).
    let (x, y) = l.clamp_frame_for(1064, 400, 826, 397);
    let placed = Layout::placed(w, h, 1064, 400, x, y).unwrap();
    assert!(
        placed.lens.is_some(),
        "the Lens fits beside the re-placed frame"
    );
    assert_eq!((x, y), (402, 397));
    // A configure that already fits stays where it was published.
    assert_eq!(l.clamp_frame_for(760, 460, 626, 357), (626, 357));
}

/// DL-1 fix-forward (Janus DL1-X1): on 1280x800 an undocked frame at the left
/// clamp could be resized to client width 1120 while `list /` waited on its
/// proof; once the Lens docks, that group (1560 wide) cannot fit the 1196-px
/// workfield. The clamp must not underflow, and the WM's refit size
/// (`docked_max_client`) must always be placeable inside the docked group.
#[test]
fn stale_configure_too_wide_for_the_docked_group_refits() {
    use crate::layout::{GLOW, edge};
    const MIN_MARGIN_PX: usize = 16;
    let (w, h) = (1280, 800);
    let mut l = Layout::placed(w, h, 640, 400, 98, 300).unwrap();
    assert_eq!(l.frame.x, 98, "the frame sits at the left clamp");
    // Undocked, a right-edge drag reaches client width 1120.
    assert_eq!(l.resized(edge::RIGHT, 10_000, 0).2, 1120);
    assert!(
        l.bloom_origin().is_some(),
        "the 640x400 frame can take a Lens"
    );
    l.lens_docked = true;
    // Janus's repro: no overflow; the impossible group lands at the low edge.
    assert_eq!(l.clamp_frame_for(1120, 400, 98, 300).0, 98);
    // The refit: the largest client the docked group holds.
    let (mw, mh) = l.docked_max_client();
    assert_eq!(mw, 728);
    assert!(mh >= 400);
    let (x, y) = l.clamp_frame_for(1120.min(mw), 400.min(mh), 98, 300);
    let placed = Layout::placed(w, h, 1120.min(mw), 400.min(mh), x, y).unwrap();
    assert!(
        placed.lens.is_some(),
        "the Lens fits beside the refitted frame"
    );
    // AAG: a client already at the docked maximum, at the clamp, refits to
    // exactly the frame it has (the WM's ack path takes that as unchanged).
    let mut at_max = Layout::placed(w, h, mw, 400, 98, 300).unwrap();
    assert!(at_max.bloom_origin().is_some());
    at_max.lens_docked = true;
    assert_eq!(at_max.clamp_frame_for(mw, 400, 98, 300), (98, 300));
    // Every refit size of every supported output is placeable docked: the
    // whole group (frame, gap, Lens, right margin) inside with its light.
    for (w, h) in [
        (1280, 800),
        (1366, 768),
        (1440, 900),
        (1920, 1080),
        (2560, 1440),
    ] {
        let mut l = Layout::placed(w, h, 640, 400, 0, 0).unwrap();
        if l.bloom_origin().is_none() {
            continue;
        }
        l.lens_docked = true;
        let (mw, mh) = l.docked_max_client();
        for (cw, ch) in [(mw, mh), (mw, 400), (640, mh)] {
            let (x, y) = l.clamp_frame_for(cw, ch, 10_000, 10_000);
            let placed = Layout::placed(w, h, cw, ch, x, y).unwrap();
            let lens = placed.lens.expect("the Lens fits beside the refit");
            assert_eq!(
                (placed.frame.x, placed.frame.y),
                (x, y),
                "{w}x{h} {cw}x{ch}"
            );
            assert!(lens.frame.right() + MIN_MARGIN_PX + GLOW <= placed.workfield.right());
            assert!(
                placed.frame.bottom().max(lens.frame.bottom()) + GLOW <= placed.workfield.bottom()
            );
        }
    }
}

// ── DM-1 motion ─────────────────────────────────────────────────────────────

mod motion_digests {
    include!("../../../fixtures/desktop-dm1/motion_digests.rs");
}

const MOTION_KINDS: [crate::motion::Kind; 6] = {
    use crate::motion::Kind::*;
    [Open, Close, View, Bloom, Unbloom, Focus]
};

/// DM-1: every plan and every weight table equals the independent oracle's,
/// written from the spec text (`scene_oracle.py motion-fixture`).
#[test]
fn motion_plans_and_weights_match_independent_python_oracle() {
    use crate::motion::{Profile, Shape, plan, weight};
    use motion_digests::*;
    let profiles = [Profile::Full, Profile::Reduced, Profile::Off];
    for &((p, k), (frames, shape)) in PLANS.iter() {
        let got = plan(profiles[p as usize], MOTION_KINDS[k as usize]);
        let shape = [Shape::Dissolve, Shape::Grow, Shape::Shrink][shape as usize];
        assert_eq!((got.frames, got.shape), (frames, shape), "plan ({p}, {k})");
    }
    for (n, table) in [
        (6, &WEIGHTS_6[..]),
        (8, &WEIGHTS_8[..]),
        (10, &WEIGHTS_10[..]),
        (12, &WEIGHTS_12[..]),
    ] {
        let ours: Vec<u32> = (1..=n).map(|k| weight(k, n)).collect();
        assert_eq!(ours, table, "a(k) for n = {n}");
        // Monotone, and 256 only at the end: no frame repeats the end frame.
        assert!(
            ours.windows(2).all(|w| w[0] < w[1]),
            "n = {n} strictly rising"
        );
        assert_eq!(ours.iter().filter(|&&a| a == 256).count(), 1);
    }
}

/// The blend, two bytes per lane, equals the per-byte formula on a grid of
/// values; at `a = 256` it is the target and at `a = 0` the source, exactly.
///   by hand: f = 0x00_10_80_FF, t = 0x00_30_00_01, a = 64:
///   R (0x10·192 + 0x30·64 + 128) >> 8 = (3072 + 3072 + 128) >> 8 = 24 = 0x18
///   G (0x80·192 + 0·64 + 128) >> 8 = (24576 + 128) >> 8 = 96 = 0x60
///   B (0xFF·192 + 1·64 + 128) >> 8 = (48960 + 192) >> 8 = 192 = 0xC0
#[test]
fn motion_blend_byte_oracle_and_identity_at_full_weight() {
    use crate::motion::blend;
    assert_eq!(blend(0x0010_80FF, 0x0030_0001, 64), 0x0018_60C0);
    let scalar = |f: u32, t: u32, a: u32| {
        let mut out = 0;
        for sh in [0, 8, 16, 24] {
            let (fb, tb) = ((f >> sh) & 0xFF, (t >> sh) & 0xFF);
            out |= (((fb * (256 - a) + tb * a + 128) >> 8) & 0xFF) << sh;
        }
        out
    };
    let values = [0u32, 1, 0x7F, 0x80, 0xFE, 0xFF];
    for &f in &values {
        for &t in &values {
            let (f, t) = (f * 0x0101_0101, (t * 0x0001_0101) | (f << 24));
            for a in [0, 1, 58, 128, 224, 255, 256] {
                assert_eq!(blend(f, t, a), scalar(f, t, a), "{f:08x} {t:08x} a={a}");
            }
            assert_eq!(blend(f, t, 256), t);
            assert_eq!(blend(f, t, 0), f);
            assert_eq!(blend(f, f, 137), f, "equal pixels never move");
        }
    }
}

fn lcg_plane(seed: u64, w: usize, h: usize) -> Vec<u32> {
    let mut v = seed;
    (0..w * h)
        .map(|_| {
            v = v
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (v >> 32) as u32
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn motion_frame_over(
    rects: &[Rect],
    anchor: Rect,
    from: &[u32],
    to: &[u32],
    dst: &mut [u32],
    w: usize,
    shape: crate::motion::Shape,
    k: u32,
    n: u32,
) {
    use crate::motion::{frame, iris};
    let bounds = rects.iter().fold(Rect::default(), |b, r| b.union(r));
    let iris = iris(shape, anchor, bounds, k, n);
    for &r in rects {
        frame(dst, from, to, w, r, shape, iris, k, n);
    }
}

/// Every shape's frames over a two-rect damage set equal the oracle's, and
/// nothing outside the rects is written (`dst` starts as a fill the oracle
/// hashes too). Frame 12 of 12 is the target, for every shape.
#[test]
fn motion_frames_match_independent_python_oracle() {
    use crate::motion::Shape;
    use motion_digests::*;
    let (w, h) = (64, 48);
    let (from, to) = (lcg_plane(1, w, h), lcg_plane(2, w, h));
    let rects: Vec<Rect> = SYN_RECTS.iter().map(rect).collect();
    let (ax, ay, aw, ah) = SYN_ANCHOR;
    let shapes = [Shape::Dissolve, Shape::Grow, Shape::Shrink];
    for &(shape, k, want) in SYN_DIGESTS.iter() {
        let mut dst = vec![0xDEAD_BEEFu32; w * h];
        let shape = shapes[shape as usize];
        let anchor = Rect::new(ax, ay, aw, ah);
        motion_frame_over(&rects, anchor, &from, &to, &mut dst, w, shape, k, 12);
        assert_eq!(masked_digest(&dst, w, h, &[]), want, "{shape:?} k={k}");
        if k == 12 {
            let mut want_px = vec![0xDEAD_BEEFu32; w * h];
            for r in &rects {
                for y in r.y..r.bottom() {
                    for x in r.x..r.right() {
                        want_px[y * w + x] = to[y * w + x];
                    }
                }
            }
            assert_eq!(dst, want_px, "{shape:?}: the end frame is the target");
        }
    }
}

/// The iris: a Grow starts as a point-sized box at the anchor's centre and
/// ends covering the bounds; a Shrink is its reverse and ends empty.
///   anchor (20,14,10,8): c = (25,18); bounds (4,4)-(60,40): hw = max(21, 35)
///   = 35, hh = max(14, 22) = 22. Grow, k = 1 of 12, a = 58:
///   ⌈35·58/256⌉ = ⌈7.93⌉ = 8, ⌈22·58/256⌉ = ⌈4.98⌉ = 5 → (17,13)-(33,23).
#[test]
fn motion_iris_byte_oracle() {
    use crate::motion::{Shape, iris};
    let anchor = Rect::new(20, 14, 10, 8);
    let bounds = Rect::new(4, 4, 56, 36);
    assert_eq!(
        iris(Shape::Grow, anchor, bounds, 1, 12),
        Rect::new(17, 13, 16, 10)
    );
    let full = iris(Shape::Grow, anchor, bounds, 12, 12);
    assert_eq!(
        full.intersect(&bounds),
        bounds,
        "Grow ends covering the bounds"
    );
    assert!(iris(Shape::Shrink, anchor, bounds, 12, 12).is_empty());
    assert!(iris(Shape::Dissolve, anchor, bounds, 3, 12).is_empty());
}

/// Kill-switch: the Off profile is one frame — the state change's own
/// present — for every kind.
#[test]
fn motion_off_is_the_present_alone() {
    use crate::motion::{Profile, plan};
    for kind in MOTION_KINDS {
        assert_eq!(plan(Profile::Off, kind).frames, 1);
    }
}

/// DM-1's mid-hold frame (the boot test's capture): the window view focused
/// blended toward Home with Arthur held, at the frame the WM holds at, equals
/// the oracle's — Full and Reduced (whose weights differ there).
#[test]
fn motion_mid_hold_matches_independent_python_oracle() {
    use crate::motion::{Kind, Profile, blend, plan, weight};
    use motion_digests::*;
    let (w, h) = (1920, 1080);
    let window = BaseScene::new(w, h, 640, 400, Mode::Glass).unwrap();
    let (from, l) = render_scene(&window, true);
    let home = BaseScene::with_home(w, h, 640, 400, Mode::Glass).unwrap();
    let mut to = vec![0u32; w * h];
    let view = crate::View::Home {
        focused: true,
        held: true,
        chip_selected: false,
    };
    home.paint_home_view(&mut to, w, view, "", home.layout.screen);
    let boxes = [
        l.bar_label,
        l.title_label,
        l.chip_label,
        home.layout.field_text,
        l.client,
    ];
    let tuples: Vec<_> = boxes.iter().map(tuple).collect();
    assert_eq!(tuples, HOLD_MASK_BOXES);
    for (profile, k, want) in [
        (Profile::Full, HOLD_MID_FULL_FRAME, HOLD_MID_GLASS_FULL),
        (
            Profile::Reduced,
            HOLD_MID_REDUCED_FRAME,
            HOLD_MID_GLASS_REDUCED,
        ),
    ] {
        let n = plan(profile, Kind::View).frames;
        let a = weight(k, n);
        let mid: Vec<u32> = from
            .iter()
            .zip(&to)
            .map(|(&f, &t)| blend(f, t, a))
            .collect();
        assert_eq!(masked_digest(&mid, w, h, &boxes), want, "{profile:?}");
    }
}

// ── desktop-intro: the first-boot introduction and the help card ────────────

/// B1b's sentences, from the file Arthur itself compiles (pure, no deps).
#[path = "../../neo-arthur/src/model_state_logic.rs"]
#[allow(dead_code)]
mod b1b;

/// Every variant the WM can show: the four model states and B2's (two
/// sample sizes), each with and without the USB-only line.
fn intro_variants() -> Vec<(String, Vec<String>)> {
    use crate::intro::{ModelState, intro_lines};
    let states = [
        ModelState::Ready,
        ModelState::Absent,
        ModelState::Damaged,
        ModelState::LoadFailed,
        ModelState::TooLittleMemory {
            have_gb: 4,
            need_gb: 8,
        },
        ModelState::TooLittleMemory {
            have_gb: 16,
            need_gb: 32,
        },
    ];
    let mut out = Vec::new();
    for state in states {
        for usb in [false, true] {
            out.push((format!("{state:?} usb={usb}"), intro_lines(state, usb)));
        }
    }
    out
}

/// The persona rules (docs/VISION-arthur-persona.md) on every row the WM can
/// draw: the intro variants, the help card and the hint. Every character
/// covered by the face (S6b: this was "ASCII only"; the Spanish table needs
/// `¿áéíñóú`, all in Inter, and an emoji is still refused because the face
/// has none), no `!` (nor `¡`), no
/// time-of-day greeting (the kernel has no wall clock), the NOTE's length
/// bounds (new rows <= 80, the variant-A greeting <= 90, B1b's rows <= 100),
/// at most `CARD_MAX_ROWS` rows, and every row inside the card's text box at
/// its size (measured with the face the WM draws with).
#[test]
fn intro_persona_kat_every_variant_and_the_help_card() {
    use crate::intro::{GREETING, HELP, HINT, state_row};
    use crate::layout::CARD_MAX_ROWS;
    use crate::scene::{CARD_TEXT_PX, HINT_PX};
    use neo_text::Font;
    use neo_text::fonts::INTER_REGULAR;
    use neo_text::layout::measure;
    let b1b_rows: Vec<&str> = [
        crate::intro::ModelState::Absent,
        crate::intro::ModelState::Damaged,
        crate::intro::ModelState::LoadFailed,
    ]
    .iter()
    .filter_map(|s| state_row(*s))
    .collect();
    let font = Font::from_bytes(INTER_REGULAR).unwrap();
    for (w, h) in [(1920, 1080), (1280, 800)] {
        let room = Layout::new(w, h, 640, 400).unwrap().card_text(1).w as f32;
        let mut cards = intro_variants();
        cards.push((
            "help".into(),
            HELP.iter().map(|s| String::from(*s)).collect(),
        ));
        for (name, rows) in &cards {
            assert!(rows.len() <= CARD_MAX_ROWS, "{name}: {} rows", rows.len());
            for row in rows {
                assert!(covered(&font, row), "{name}: not in the face: {row:?}");
                assert!(!row.contains(['!', '¡']), "{name}: exclamation: {row:?}");
                for t in ["morning", "afternoon", "evening"] {
                    assert!(!row.contains(t), "{name}: time of day: {row:?}");
                }
                let bound = if row == GREETING {
                    90
                } else if b1b_rows.contains(&row.as_str()) {
                    100
                } else {
                    80
                };
                assert!(
                    row.len() <= bound,
                    "{name}: {} > {bound}: {row:?}",
                    row.len()
                );
                let wide = measure(&font, CARD_TEXT_PX, row);
                assert!(
                    wide <= room,
                    "{name} at {w}x{h}: {wide} px > {room}: {row:?}"
                );
            }
        }
        let hint = measure(&font, HINT_PX, HINT);
        assert!(hint <= room, "hint {hint} px > {room}");
    }
    let font = Font::from_bytes(INTER_REGULAR).unwrap();
    assert!(covered(&font, HINT) && !HINT.contains('!'));
    assert!(HINT.len() <= 80);
}

/// The persona rules' character rule (S6b): every character is one the face
/// draws (`glyph_id` read from Inter's cmap), and none is a control.
fn covered(font: &neo_text::Font<'_>, row: &str) -> bool {
    row.chars()
        .all(|c| !c.is_control() && font.glyph_id(c) != 0)
}

/// S6b: the persona KAT's rules on every card in Spanish: the intro variants
/// (with and without the clock), the help card, the hint, and the power
/// cards. Every character in the face, no `!` or `¡`, at most
/// `CARD_MAX_ROWS` rows, each row inside the card at its size. (The NOTE's
/// byte bounds are English's; Spanish rows are held by their measured width,
/// as `i18n-core`'s KAT holds every row.) The frame holds too: the greeting
/// first, the exit last.
#[test]
fn persona_kat_in_spanish() {
    use crate::clock::{Now, greeting_in};
    use crate::intro::{help_lines_in, hint_in, intro_lines_in, intro_lines_now_in};
    use crate::layout::CARD_MAX_ROWS;
    use crate::power::{PowerCommand, confirm_lines_in, going_lines_in, refused_lines_in};
    use crate::scene::{CARD_TEXT_PX, CLOCK_DATE_PX, CLOCK_TIME_PX, HINT_PX};
    use i18n_core::{Lang, Msg, text};
    use neo_text::Font;
    use neo_text::fonts::INTER_REGULAR;
    use neo_text::layout::measure;
    let es = Lang::Es;
    let font = Font::from_bytes(INTER_REGULAR).unwrap();
    let mut cards: Vec<(String, Vec<String>)> = Vec::new();
    for (model, usb) in intro_states() {
        let rows = intro_lines_in(es, model, usb);
        assert_eq!(rows[0], text(es, Msg::IntroGreeting));
        assert_eq!(
            rows.last().map(String::as_str),
            Some(text(es, Msg::IntroExit))
        );
        cards.push((format!("{model:?} usb={usb}"), rows));
        let now = Now {
            hour: 17,
            time: "12:59 p. m.",
            date: "miércoles 30 de septiembre de 2026",
        };
        let rows = intro_lines_now_in(es, model, usb, &now);
        assert_eq!(
            rows[0],
            "Buenas tardes. Ahora: 12:59 p. m., miércoles 30 de septiembre de 2026."
        );
        assert_eq!(rows[1], text(es, Msg::IntroName));
        cards.push((format!("{model:?} usb={usb} clock"), rows));
    }
    cards.push(("help".into(), help_lines_in(es)));
    for cmd in [PowerCommand::ShutDown, PowerCommand::Restart] {
        cards.push((format!("{cmd:?}"), confirm_lines_in(es, cmd)));
        cards.push((format!("{cmd:?} going"), going_lines_in(es, cmd)));
    }
    for errno in [-16, -38, -5] {
        cards.push((format!("refused {errno}"), refused_lines_in(es, errno)));
    }
    assert_eq!(
        (greeting_in(es, 5), greeting_in(es, 12), greeting_in(es, 18)),
        ("Buenos días", "Buenas tardes", "Buenas noches")
    );
    for (w, h) in [(1920, 1080), (1280, 800)] {
        let room = Layout::new(w, h, 640, 400).unwrap().card_text(1).w as f32;
        for (name, rows) in &cards {
            assert!(rows.len() <= CARD_MAX_ROWS, "{name}: {} rows", rows.len());
            for row in rows {
                assert!(covered(&font, row), "{name}: not in the face: {row:?}");
                assert!(!row.contains(['!', '¡']), "{name}: exclamation: {row:?}");
                let wide = measure(&font, CARD_TEXT_PX, row);
                assert!(
                    wide <= room,
                    "{name} at {w}x{h}: {wide} px > {room}: {row:?}"
                );
            }
        }
        let hint = measure(&font, HINT_PX, hint_in(es));
        assert!(hint <= room, "hint {hint} px > {room}");
        // Home's readout: the longest time with the not-set mark, and the
        // longest date, inside the readout's box.
        let clock_room = Layout::new(w, h, 640, 400).unwrap().clock_box().w as f32;
        for lang in [Lang::En, es] {
            let time = format!(
                "12:59{}{}",
                text(lang, Msg::Pm),
                text(lang, Msg::ClockReadoutZoneNotSet)
            );
            let t = measure(&font, CLOCK_TIME_PX, &time);
            assert!(t <= clock_room, "{time:?} {t} px > {clock_room}");
        }
        let date = "miércoles 30 de septiembre de 2026";
        let d = measure(&font, CLOCK_DATE_PX, date);
        assert!(d <= clock_room, "{date:?} {d} px > {clock_room}");
    }
}

/// S6b: the power card's button row in Spanish, and its two labels found by
/// the hit test (`card_buttons`) on a row with accents elsewhere on the card.
#[test]
fn spanish_button_row_and_its_labels() {
    use crate::intro::button_row_in;
    use i18n_core::Lang;
    let row = button_row_in(Lang::Es, "Apagar");
    assert_eq!(row, format!("[ Apagar ]{}[ Volver ]", " ".repeat(17)));
    // Columns count characters: an accented label lines up as an ASCII one
    // (Back at column 27; the byte offset is 28, as `í` is two bytes).
    let row = button_row_in(Lang::Es, "Sí");
    assert!(row.ends_with("[ Volver ]"));
    assert_eq!(row.chars().count(), 27 + "[ Volver ]".len());
    assert_eq!(
        button_row_in(Lang::En, "Shut down"),
        crate::intro::button_row("Shut down")
    );
}

/// The model-state rows are B1b's sentences verbatim (one row each: every
/// one fits the card, the persona KAT measures it), so the card never disagrees with the
/// line Arthur answers with later; and each variant keeps its frame: the
/// greeting first, the exit (help, Esc) last, the Lens line (it needs no
/// model) in every no-model variant, the example question only with a model.
#[test]
fn intro_model_state_rows_are_b1b_verbatim_and_every_variant_offers_the_exit() {
    use crate::intro::{
        ASK, EXIT, GREETING, LIST, LIST_STILL, ModelState, PRIVACY, USB_ONLY, intro_lines,
        state_row,
    };
    for (ours, theirs) in [
        (ModelState::Absent, b1b::ModelState::Absent),
        (ModelState::Damaged, b1b::ModelState::Damaged),
        (ModelState::LoadFailed, b1b::ModelState::LoadFailed),
    ] {
        let want = core::str::from_utf8(b1b::beginner_line(theirs).unwrap()).unwrap();
        assert_eq!(state_row(ours), Some(want), "{ours:?}");
        for usb in [false, true] {
            let rows = intro_lines(ours, usb);
            assert!(rows.iter().any(|r| r == LIST_STILL), "{ours:?}");
            assert!(!rows.iter().any(|r| r == ASK || r == PRIVACY), "{ours:?}");
        }
    }
    assert!(b1b::beginner_line(b1b::ModelState::Ready).is_none());
    assert!(state_row(ModelState::Ready).is_none());
    for (name, rows) in intro_variants() {
        assert_eq!(rows.first().map(String::as_str), Some(GREETING), "{name}");
        assert_eq!(rows.last().map(String::as_str), Some(EXIT), "{name}");
        assert_eq!(
            rows.iter().any(|r| r == USB_ONLY),
            name.ends_with("usb=true"),
            "{name}"
        );
    }
    // The model-ready intro, as the PL approved it (variant A; seven rows
    // on a USB-only machine, not merged).
    assert_eq!(
        intro_lines(ModelState::Ready, true),
        [
            "Good day. I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.",
            "The household side comes later; for now, I look after this computer for you.",
            "I work entirely on this machine, and I don't send what you tell me anywhere.",
            "On this machine I can't keep anything once it's switched off, not yet.",
            "Ask in your own words: \"What does a CPU do?\", for instance.",
            "\"list /\" shows you the files on this machine.",
            "Type help for what you can do yourself. Esc always brings you back here.",
        ]
    );
    assert_eq!(intro_lines(ModelState::Ready, false).len(), 6);
    assert!(intro_lines(ModelState::Ready, true).contains(&String::from(LIST)));
    assert_eq!(
        intro_lines(
            ModelState::TooLittleMemory {
                have_gb: 4,
                need_gb: 8
            },
            false
        )[2],
        "This computer has 4 GB of memory; my smallest brain needs 8 GB."
    );
}

/// When the intro shows (the NOTE's rule, hand-tabled): a machine that keeps
/// nothing shows it every session; one that persists shows it until the
/// flag says it was seen. And what opens the help card from the field.
#[test]
fn intro_show_rule_and_help_command() {
    use crate::intro::{is_help, show_intro};
    //            persistent introduced -> shows
    assert!(show_intro(false, false));
    assert!(show_intro(false, true));
    assert!(show_intro(true, false));
    assert!(!show_intro(true, true));
    for yes in [&b"help"[..], b"Help", b"HELP", b"  help ", b"hElP"] {
        assert!(is_help(yes), "{yes:?}");
    }
    for no in [
        &b""[..],
        b"   ",
        b"hel",
        b"helps",
        b"help me",
        b"list /",
        b"?",
        b"h elp",
    ] {
        assert!(!is_help(no), "{no:?}");
    }
}

/// The kill-switch: a scene that never had `set_intro`, and one that had it
/// on and off again, paint the Home view and the window view bit for bit as
/// DS-1 (no card slot, no hint, the five view regions only).
#[test]
fn intro_off_is_ds1_home_bit_for_bit() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let plain = BaseScene::with_home(1920, 1080, 640, 400, mode).unwrap();
        let mut toggled = BaseScene::with_home(1920, 1080, 640, 400, mode).unwrap();
        assert!(toggled.set_intro(true));
        toggled.set_card(crate::intro::help_lines());
        assert!(toggled.set_intro(false));
        assert_eq!(toggled.card_rows(), 0);
        assert!(plain.layout.view_regions()[5].is_empty());
        assert!(toggled.layout.view_regions()[5].is_empty());
        assert!(plain.intro_text_box().is_empty());
        for focused in [false, true] {
            let mut a = vec![0u32; 1920 * 1080];
            let mut b = vec![0u32; 1920 * 1080];
            plain.paint_home_rect(&mut a, 1920, focused, "", plain.layout.screen);
            toggled.paint_home_rect(&mut b, 1920, focused, "", toggled.layout.screen);
            assert!(a == b, "{mode:?} home focused={focused}");
            plain.paint(&mut a, 1920, focused);
            toggled.paint(&mut b, 1920, focused);
            assert!(a == b, "{mode:?} window focused={focused}");
        }
    }
    // A scene without Home cannot take the slot.
    let mut s3 = BaseScene::new(1280, 800, 640, 400, Mode::Glass).unwrap();
    assert!(!s3.set_intro(true));
}

/// With the intro on: the card slot is one more view region (hand-laid-out
/// at 1920x1080 below), the window view is unchanged by it, the card and
/// hint differ from plain Home only inside the slot, and a view switch or a
/// card change repainting the slot lands exactly on a full paint.
///   field (642, 411, 720, 56): slot y = 411 + 56 + 14 (GLOW) + 18 = 499,
///   h = 8·24 + 20 = 212; intro card (7 rows) h = 188, text box
///   (664, 509, 676, 168); hint box (664, 509, 676, 24).
#[test]
fn intro_card_slot_geometry_and_repaints() {
    use crate::intro::{ModelState, help_lines, intro_lines};
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    assert_eq!(l.card(7), Rect::new(642, 499, 720, 188));
    assert_eq!(l.card_text(7), Rect::new(664, 509, 676, 168));
    assert_eq!(l.hint(), Rect::new(664, 509, 676, 24));
    assert_eq!(l.card(8), Rect::new(642, 499, 720, 212));
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let plain = BaseScene::with_lens(1280, 800, 640, 400, mode).unwrap();
        let mut scene = BaseScene::with_lens(1280, 800, 640, 400, mode).unwrap();
        assert!(scene.set_intro(true));
        let slot = scene.layout.card_slot();
        assert_eq!(scene.layout.view_regions()[5], slot);
        let mut a = vec![0u32; 1280 * 800];
        let mut b = vec![0u32; 1280 * 800];
        plain.paint(&mut a, 1280, true);
        scene.paint(&mut b, 1280, true);
        assert!(a == b, "{mode:?}: the window view never shows the slot");
        for rows in [
            Vec::new(),
            intro_lines(ModelState::Ready, true),
            help_lines(),
        ] {
            scene.set_card(rows.clone());
            plain.paint_home_rect(&mut a, 1280, true, "", plain.layout.screen);
            scene.paint_home_rect(&mut b, 1280, true, "", scene.layout.screen);
            let mut differs = 0usize;
            for (i, (pa, pb)) in a.iter().zip(&b).enumerate() {
                if pa != pb {
                    assert!(
                        slot.contains(i % 1280, i / 1280),
                        "{mode:?}: outside the slot"
                    );
                    differs += 1;
                }
            }
            assert!(differs > 0, "{mode:?} {} rows: nothing drawn", rows.len());
            // Window -> Home over the view regions lands on Home.
            let mut live = vec![0u32; 1280 * 800];
            scene.paint(&mut live, 1280, true);
            for r in scene.layout.view_regions() {
                scene.paint_home_rect(&mut live, 1280, true, "", r);
            }
            assert!(live == b, "{mode:?} {} rows: window -> home", rows.len());
            // Home -> window clears the slot.
            for r in scene.layout.view_regions() {
                scene.paint_rect(&mut live, 1280, true, r);
            }
            let mut window = vec![0u32; 1280 * 800];
            scene.paint(&mut window, 1280, true);
            assert!(
                live == window,
                "{mode:?} {} rows: home -> window",
                rows.len()
            );
        }
        // A card change repaints the slot alone (the WM's dismiss / help).
        let mut live = vec![0u32; 1280 * 800];
        scene.set_card(intro_lines(ModelState::Ready, true));
        scene.paint_home_rect(&mut live, 1280, true, "", scene.layout.screen);
        scene.set_card(Vec::new());
        scene.paint_home_rect(&mut live, 1280, true, "", slot);
        let mut hint = vec![0u32; 1280 * 800];
        scene.paint_home_rect(&mut hint, 1280, true, "", scene.layout.screen);
        assert!(live == hint, "{mode:?}: dismissing repaints the slot alone");
    }
}

/// A frame move keeps the slot (it hangs from the field, which never moves).
#[test]
fn intro_slot_survives_a_frame_move() {
    let mut scene = BaseScene::with_home(1920, 1080, 640, 400, Mode::ReduceTransparency).unwrap();
    scene.set_intro(true);
    let slot = scene.layout.card_slot();
    assert!(scene.move_frame(300, 200));
    assert_eq!(scene.layout.card_slot(), slot);
}

#[allow(dead_code)] // FIELD_WINDOW / HINT_FIELD_* are the kernel's
mod intro_fixture {
    include!("../../../fixtures/desktop-intro/intro.rs");
}

/// The column-pattern check the QEMU captures are held to (xtask, kernel):
/// per row, a column is inked when a pixel of its band reaches the row's
/// green threshold; `#` must be inked, `.` must not, `?` may be either.
/// Returns the first mismatching column.
fn row_pattern_mismatch(
    px: &[u32],
    stride: usize,
    band: (usize, usize, usize, usize),
    thr: u8,
    pattern: &str,
) -> Option<usize> {
    let (bx, by, bw, bh) = band;
    assert_eq!(pattern.len(), bw);
    pattern.bytes().enumerate().find_map(|(c, want)| {
        let inked = (by..by + bh).any(|y| (px[y * stride + bx + c] >> 8) & 0xFF >= u32::from(thr));
        match (want, inked) {
            (b'#', false) | (b'.', true) => Some(c),
            _ => None,
        }
    })
}

/// The oracle states the approved wording again; it must equal the Rust.
#[test]
fn intro_fixture_wording_matches_the_rust() {
    use crate::intro::{HELP, HINT, ModelState, intro_lines};
    assert_eq!(
        intro_lines(ModelState::Ready, true),
        intro_fixture::INTRO_ROWS
    );
    assert_eq!(HELP, intro_fixture::HELP_ROWS);
    assert_eq!(HINT, intro_fixture::HINT);
}

/// The hint, the intro card and the help card on Home equal the independent
/// oracle (scene_oracle.py + spec v1.8) outside the Home label boxes and the
/// state's text box, in both materials and at both resolutions; the boxes
/// and card rects are the oracle's.
#[test]
fn intro_cards_match_independent_python_oracle() {
    use crate::intro::{ModelState, help_lines, intro_lines};
    use intro_fixture::*;
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    assert_eq!(tuple(&l.hint()), HINT_TEXT_BOX);
    assert_eq!(tuple(&l.card_text(7)), INTRO_TEXT_BOX);
    assert_eq!(tuple(&l.card_text(6)), HELP_TEXT_BOX);
    assert_eq!(tuple(&l.card(7)), INTRO_CARD);
    assert_eq!(tuple(&l.card(6)), HELP_CARD);
    let cases = [
        ((1920, 1080), Mode::Glass, 0, HINT_GLASS_1920X1080),
        ((1920, 1080), Mode::ReduceTransparency, 0, HINT_RT_1920X1080),
        ((1920, 1080), Mode::Glass, 7, INTRO_GLASS_1920X1080),
        (
            (1920, 1080),
            Mode::ReduceTransparency,
            7,
            INTRO_RT_1920X1080,
        ),
        ((1920, 1080), Mode::Glass, 6, HELP_GLASS_1920X1080),
        ((1920, 1080), Mode::ReduceTransparency, 6, HELP_RT_1920X1080),
        ((1280, 800), Mode::Glass, 0, HINT_GLASS_1280X800),
        ((1280, 800), Mode::ReduceTransparency, 0, HINT_RT_1280X800),
        ((1280, 800), Mode::Glass, 7, INTRO_GLASS_1280X800),
        ((1280, 800), Mode::ReduceTransparency, 7, INTRO_RT_1280X800),
        ((1280, 800), Mode::Glass, 6, HELP_GLASS_1280X800),
        ((1280, 800), Mode::ReduceTransparency, 6, HELP_RT_1280X800),
    ];
    for ((w, h), mode, rows, want) in cases {
        let mut scene = BaseScene::with_lens(w, h, 640, 400, mode).unwrap();
        scene.set_intro(true);
        scene.set_card(match rows {
            0 => Vec::new(),
            7 => intro_lines(ModelState::Ready, true),
            _ => help_lines(),
        });
        let mut buf = vec![0u32; w * h];
        scene.paint_home_rect(&mut buf, w, true, "", scene.layout.screen);
        let l = scene.layout;
        let hb = l.home_label_boxes();
        let boxes = [hb[0], hb[1], hb[2], scene.intro_text_box()];
        assert_eq!(
            masked_digest(&buf, w, h, &boxes),
            want,
            "{w}x{h} {mode:?} rows={rows}"
        );
    }
    // Arthur held behind Home: the hint shows there too.
    for (mode, want) in [
        (Mode::Glass, HINT_GLASS_1920X1080_HELD),
        (Mode::ReduceTransparency, HINT_RT_1920X1080_HELD),
    ] {
        let mut scene = BaseScene::with_lens(1920, 1080, 640, 400, mode).unwrap();
        scene.set_intro(true);
        let mut buf = vec![0u32; 1920 * 1080];
        let view = crate::View::Home {
            focused: true,
            held: true,
            chip_selected: false,
        };
        scene.paint_home_view(&mut buf, 1920, view, "", scene.layout.screen);
        let hb = scene.layout.home_label_boxes();
        let boxes = [hb[0], hb[1], hb[2], scene.intro_text_box()];
        assert_eq!(
            masked_digest(&buf, 1920, 1080, &boxes),
            want,
            "{mode:?} held"
        );
    }
}

/// The words on screen: neo-text's rows (the hint, all seven intro rows, all
/// six help rows) meet the oracle's exact-area column patterns — the same
/// check the QEMU captures get. And it bites: the help rows fail the intro's
/// patterns, a row drawn one pixel right fails, a missing row fails.
#[test]
fn intro_rows_meet_the_exact_area_column_patterns() {
    use crate::intro::{ModelState, help_lines, intro_lines};
    use intro_fixture::*;
    let render = |rows: Vec<String>| {
        let mut scene = BaseScene::with_lens(1920, 1080, 640, 400, Mode::Glass).unwrap();
        scene.set_intro(true);
        scene.set_card(rows);
        let mut buf = vec![0u32; 1920 * 1080];
        scene.paint_home_rect(&mut buf, 1920, true, "", scene.layout.screen);
        buf
    };
    let hint = render(Vec::new());
    let intro = render(intro_lines(ModelState::Ready, true));
    let help = render(help_lines());
    let all = |buf: &[u32], pats: &[intro_fixture::Row]| {
        pats.iter()
            .map(|&(band, thr, pat)| row_pattern_mismatch(buf, 1920, band, thr, pat))
            .collect::<Vec<_>>()
    };
    assert!(all(&hint, &HINT_PATTERNS).iter().all(Option::is_none));
    assert!(
        all(&intro, &INTRO_PATTERNS).iter().all(Option::is_none),
        "{:?}",
        all(&intro, &INTRO_PATTERNS)
    );
    assert!(
        all(&help, &HELP_PATTERNS).iter().all(Option::is_none),
        "{:?}",
        all(&help, &HELP_PATTERNS)
    );
    // Negative controls.
    assert!(all(&help, &INTRO_PATTERNS[..6]).iter().all(Option::is_some));
    let mut shifted = intro_lines(ModelState::Ready, true);
    shifted[2].insert(0, ' ');
    assert!(all(&render(shifted), &INTRO_PATTERNS)[2].is_some());
    let mut short = intro_lines(ModelState::Ready, true);
    short.truncate(6);
    assert!(
        all(&render(short), &INTRO_PATTERNS)[..6]
            .iter()
            .all(Option::is_none)
    );
    assert!(all(&hint, &INTRO_PATTERNS).iter().all(Option::is_some));
}

// ── desktop-clock (OT-85): the Home readout and the time-aware intro ────────

/// Sample "now"s: every hour of a day, with the texts libneo::civil makes for
/// them (written out here, not computed), including the longest date and
/// time the formatter can produce.
fn clock_samples() -> Vec<(u8, &'static str, &'static str)> {
    const TIMES: [&str; 24] = [
        "12:00 AM", "1:05 AM", "2:10 AM", "3:15 AM", "4:59 AM", "5:00 AM", "6:30 AM", "7:45 AM",
        "8:00 AM", "9:07 AM", "10:10 AM", "11:59 AM", "12:00 PM", "1:01 PM", "2:22 PM", "3:33 PM",
        "4:44 PM", "5:48 PM", "6:00 PM", "7:15 PM", "8:30 PM", "9:45 PM", "10:50 PM", "11:59 PM",
    ];
    let mut out = Vec::new();
    for (h, t) in TIMES.iter().enumerate() {
        out.push((h as u8, *t, "Wednesday, October 21, 2026"));
    }
    out.push((12, "12:59 PM", "Wednesday, September 30, 2026"));
    out.push((0, "12:00 AM", "Thursday, January 1, 1970"));
    out
}

/// The greeting's boundaries (hand-tabled from the code's stated rule:
/// morning 05:00-11:59, afternoon 12:00-17:59, evening 18:00-04:59).
#[test]
fn clock_greeting_hour_table() {
    use crate::clock::greeting;
    let want = [
        "Good evening",
        "Good evening",
        "Good evening",
        "Good evening",
        "Good evening",
        "Good morning",
        "Good morning",
        "Good morning",
        "Good morning",
        "Good morning",
        "Good morning",
        "Good morning",
        "Good afternoon",
        "Good afternoon",
        "Good afternoon",
        "Good afternoon",
        "Good afternoon",
        "Good afternoon",
        "Good evening",
        "Good evening",
        "Good evening",
        "Good evening",
        "Good evening",
        "Good evening",
    ];
    for (h, w) in want.iter().enumerate() {
        assert_eq!(greeting(h as u8), *w, "hour {h}");
    }
}

/// The persona KAT over every clock variant: each intro variant (all model
/// states, USB-only or not) at every hour, with the longest texts. The rules
/// of `intro_persona_kat_every_variant_and_the_help_card` hold unchanged:
/// every character in the face, no `!`, new rows <= 80 (B1b's <= 100), at most
/// `CARD_MAX_ROWS` rows, each inside the card at its size. The one rule that
/// existed only because there was no clock, "no time-of-day word", becomes
/// stricter, not looser: exactly one row carries one, the first, and it is
/// the right word for the hour.
#[test]
fn clock_persona_kat_every_variant_every_hour() {
    use crate::clock::{Now, greeting};
    use crate::intro::{EXIT, GREETING, NAME, intro_lines, intro_lines_now, state_row};
    use crate::layout::CARD_MAX_ROWS;
    use crate::scene::{CARD_TEXT_PX, CLOCK_DATE_PX, CLOCK_TIME_PX};
    use neo_text::Font;
    use neo_text::fonts::INTER_REGULAR;
    use neo_text::layout::measure;
    assert_eq!(
        GREETING,
        format!("Good day. {NAME}"),
        "the name row is line 1's"
    );
    let b1b_rows: Vec<&str> = [
        crate::intro::ModelState::Absent,
        crate::intro::ModelState::Damaged,
        crate::intro::ModelState::LoadFailed,
    ]
    .iter()
    .filter_map(|s| state_row(*s))
    .collect();
    let font = Font::from_bytes(INTER_REGULAR).unwrap();
    let words = ["morning", "afternoon", "evening"];
    for (w, h) in [(1920, 1080), (1280, 800)] {
        let l = Layout::new(w, h, 640, 400).unwrap();
        let room = l.card_text(1).w as f32;
        let clock_room = l.clock_box().w as f32;
        for (hour, time, date) in clock_samples() {
            let now = Now { hour, time, date };
            // The readout's own lines fit its box.
            assert!(measure(&font, CLOCK_TIME_PX, time) <= clock_room, "{time}");
            assert!(measure(&font, CLOCK_DATE_PX, date) <= clock_room, "{date}");
            for (model, usb) in intro_states() {
                let name = format!("{model:?} usb={usb}");
                let plain = intro_lines(model, usb);
                let rows = intro_lines_now(model, usb, &now);
                assert_eq!(rows, intro_lines_now(model, usb, &now));
                assert_eq!(rows.len(), plain.len() + 1, "{name}");
                assert_eq!(&rows[2..], &plain[1..], "{name}: after line 1, unchanged");
                assert_eq!(rows[1], NAME);
                assert_eq!(rows.last().map(String::as_str), Some(EXIT));
                assert!(rows.len() <= CARD_MAX_ROWS, "{name}: {} rows", rows.len());
                assert_eq!(
                    rows[0],
                    format!("{}. It's {time} on {date}.", greeting(hour)),
                    "{name}"
                );
                for (i, row) in rows.iter().enumerate() {
                    assert!(covered(&font, row), "{name}: not in the face: {row:?}");
                    assert!(!row.contains(['!', '¡']), "{name}: exclamation: {row:?}");
                    let said: Vec<&str> =
                        words.iter().copied().filter(|t| row.contains(t)).collect();
                    if i == 0 {
                        assert_eq!(said.len(), 1, "{row:?}");
                        assert!(greeting(hour).ends_with(said[0]), "hour {hour}: {row:?}");
                    } else {
                        assert!(said.is_empty(), "{name}: time of day: {row:?}");
                    }
                    let bound = if b1b_rows.contains(&row.as_str()) {
                        100
                    } else {
                        80
                    };
                    assert!(
                        row.len() <= bound,
                        "{name}: {} > {bound}: {row:?}",
                        row.len()
                    );
                    let wide = measure(&font, CARD_TEXT_PX, row);
                    assert!(
                        wide <= room,
                        "{name} at {w}x{h}: {wide} px > {room}: {row:?}"
                    );
                }
            }
        }
        // The clock-less variants are the ones the old KAT covers, unchanged
        // (its no-time-of-day rule stands for them).
        let old: Vec<Vec<String>> = intro_variants().into_iter().map(|(_, r)| r).collect();
        let new: Vec<Vec<String>> = intro_states()
            .into_iter()
            .map(|(m, u)| intro_lines(m, u))
            .collect();
        assert_eq!(old, new);
    }
}

/// The states `intro_variants` covers, each with and without USB-only.
fn intro_states() -> Vec<(crate::intro::ModelState, bool)> {
    use crate::intro::ModelState;
    let mut out = Vec::new();
    for state in [
        ModelState::Ready,
        ModelState::Absent,
        ModelState::Damaged,
        ModelState::LoadFailed,
        ModelState::TooLittleMemory {
            have_gb: 4,
            need_gb: 8,
        },
        ModelState::TooLittleMemory {
            have_gb: 16,
            need_gb: 32,
        },
    ] {
        for usb in [false, true] {
            out.push((state, usb));
        }
    }
    out
}

/// The readout's box (hand-laid-out at 1920x1080): the field is (642, 411,
/// 720, 56), its focus light starts at 411 - 14 = 397; the box is 84 tall
/// and ends 28 above that: y = 397 - 28 - 84 = 285. At 1280x800: the field is
/// at y = 36 + (764 - 56)·38/100 = 305, light 291, box y = 291 - 112 = 179,
/// x = 84 + (1196 - 720)/2 = 322.
#[test]
fn clock_box_geometry() {
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    assert_eq!(l.clock_box(), Rect::new(642, 285, 720, 84));
    assert!(l.clock_slot().is_empty());
    assert!(l.view_regions()[6].is_empty());
    let l = Layout::new(1280, 800, 640, 400).unwrap();
    assert_eq!(l.clock_box(), Rect::new(322, 179, 720, 84));
    // Clear of the bar, the field's light and the card slot.
    for (w, h) in [(1920, 1080), (1280, 800), (800, 538)] {
        let mut l = Layout::new(w, h, 640, 400).unwrap();
        l.intro = true;
        l.clock = true;
        let b = l.clock_box();
        assert_eq!(l.view_regions()[6], b);
        assert!(b.y >= l.bar.bottom(), "{w}x{h}");
        assert!(b.intersect(&l.field_glow).is_empty(), "{w}x{h}");
        assert!(b.intersect(&l.card_slot()).is_empty(), "{w}x{h}");
        assert!(b.intersect(&l.spine).is_empty(), "{w}x{h}");
    }
}

/// The kill-switch: a Home scene that never had `set_clock`, and one that had
/// it on and off again, paint Home and the window view bit for bit alike (no
/// readout, the seventh view region empty). On, the readout draws ink only
/// inside its box; the window view never shows it; a minute change repaints
/// the box alone; Window -> Home over the view regions lands on Home.
#[test]
fn clock_off_is_home_bit_for_bit_and_on_draws_only_in_its_box() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let plain = BaseScene::with_lens(1280, 800, 640, 400, mode).unwrap();
        let mut toggled = BaseScene::with_lens(1280, 800, 640, 400, mode).unwrap();
        assert!(toggled.set_clock(Some(("5:48 PM", "Wednesday, October 21, 2026"))));
        assert!(toggled.set_clock(None));
        assert!(toggled.layout.view_regions()[6].is_empty());
        assert!(toggled.clock_text_box().is_empty());
        let mut a = vec![0u32; 1280 * 800];
        let mut b = vec![0u32; 1280 * 800];
        for focused in [false, true] {
            plain.paint_home_rect(&mut a, 1280, focused, "", plain.layout.screen);
            toggled.paint_home_rect(&mut b, 1280, focused, "", toggled.layout.screen);
            assert!(a == b, "{mode:?} home focused={focused}");
            plain.paint(&mut a, 1280, focused);
            toggled.paint(&mut b, 1280, focused);
            assert!(a == b, "{mode:?} window focused={focused}");
        }
        let mut scene = BaseScene::with_lens(1280, 800, 640, 400, mode).unwrap();
        assert!(scene.set_clock(Some(("5:48 PM", "Wednesday, October 21, 2026"))));
        assert_eq!(
            scene.clock_text(),
            ("5:48 PM", "Wednesday, October 21, 2026")
        );
        let boxed = scene.clock_text_box();
        assert_eq!(boxed, Rect::new(322, 179, 720, 84));
        plain.paint(&mut a, 1280, true);
        scene.paint(&mut b, 1280, true);
        assert!(a == b, "{mode:?}: the window view never shows the readout");
        plain.paint_home_rect(&mut a, 1280, true, "", plain.layout.screen);
        scene.paint_home_rect(&mut b, 1280, true, "", scene.layout.screen);
        let mut differs = 0usize;
        for (i, (pa, pb)) in a.iter().zip(&b).enumerate() {
            if pa != pb {
                assert!(
                    boxed.contains(i % 1280, i / 1280),
                    "{mode:?}: outside the box"
                );
                differs += 1;
            }
        }
        assert!(differs > 400, "{mode:?}: the readout drew {differs} px");
        // A minute later: repainting the box alone equals a full paint.
        assert!(scene.set_clock(Some(("5:49 PM", "Wednesday, October 21, 2026"))));
        let mut live = b.clone();
        scene.paint_home_rect(&mut live, 1280, true, "", boxed);
        scene.paint_home_rect(&mut b, 1280, true, "", scene.layout.screen);
        assert!(live == b, "{mode:?}: minute repaint");
        // Window -> Home over the view regions lands on Home.
        scene.paint(&mut live, 1280, true);
        for r in scene.layout.view_regions() {
            scene.paint_home_rect(&mut live, 1280, true, "", r);
        }
        assert!(live == b, "{mode:?}: window -> home");
    }
    // A scene without Home cannot show it.
    let mut s3 = BaseScene::new(1280, 800, 640, 400, Mode::Glass).unwrap();
    assert!(!s3.set_clock(Some(("5:48 PM", "x"))));
    assert!(s3.set_clock(None));
}

#[allow(dead_code)] // FIELD_WINDOW / HINT_FIELD_CLOCK_* are the kernel's
mod clock_fixture {
    include!("../../../fixtures/desktop-clock/clock.rs");
}

/// The independent oracle (scripts/desktop/clock_oracle.py, no neo-atrium
/// code): the readout's box at both sizes, and the intro session's final
/// Home with the readout drawn, digested with the readout box masked as well
/// as the labels and the hint: equal to the oracle's clock-less scene. The
/// kernel holds the QEMU capture to the same digest.
#[test]
fn clock_matches_the_independent_oracle_outside_its_box() {
    use clock_fixture::*;
    let tuple = |r: Rect| (r.x, r.y, r.w, r.h);
    assert_eq!(
        tuple(Layout::new(1920, 1080, 640, 400).unwrap().clock_box()),
        CLOCK_BOX_1920X1080
    );
    assert_eq!(
        tuple(Layout::new(1280, 800, 640, 400).unwrap().clock_box()),
        CLOCK_BOX_1280X800
    );
    for (mode, want) in [
        (Mode::Glass, HINT_CLOCK_GLASS_1920X1080),
        (Mode::ReduceTransparency, HINT_CLOCK_RT_1920X1080),
    ] {
        let mut scene = BaseScene::with_lens(1920, 1080, 640, 400, mode).unwrap();
        assert!(scene.set_intro(true));
        assert!(scene.set_clock(Some(("5:48 PM", "Wednesday, October 21, 2026"))));
        let mut buf = vec![0u32; 1920 * 1080];
        scene.paint_home_rect(&mut buf, 1920, true, "", scene.layout.screen);
        let hb = scene.layout.home_label_boxes();
        let boxes = [
            hb[0],
            hb[1],
            hb[2],
            scene.intro_text_box(),
            scene.clock_text_box(),
        ];
        assert_eq!(masked_digest(&buf, 1920, 1080, &boxes), want, "{mode:?}");
    }
    // The session's captures: the eight-row clock intro and the help card.
    use crate::clock::Now;
    use crate::intro::{ModelState, help_lines, intro_lines_now};
    let now = Now {
        hour: 17,
        time: "5:48 PM",
        date: "Wednesday, October 21, 2026",
    };
    for (rows, text_box, want) in [
        (
            intro_lines_now(ModelState::Ready, true, &now),
            INTRO_CLOCK_TEXT_BOX,
            INTRO_CLOCK_GLASS_1920X1080,
        ),
        (
            help_lines(),
            HELP_CLOCK_TEXT_BOX,
            HELP_CLOCK_GLASS_1920X1080,
        ),
    ] {
        let mut scene = BaseScene::with_lens(1920, 1080, 640, 400, Mode::Glass).unwrap();
        assert!(scene.set_intro(true));
        assert!(scene.set_clock(Some((now.time, now.date))));
        scene.set_card(rows);
        assert_eq!(tuple(scene.intro_text_box()), text_box);
        let mut buf = vec![0u32; 1920 * 1080];
        scene.paint_home_rect(&mut buf, 1920, true, "", scene.layout.screen);
        let hb = scene.layout.home_label_boxes();
        let boxes = [
            hb[0],
            hb[1],
            hb[2],
            scene.intro_text_box(),
            scene.clock_text_box(),
        ];
        assert_eq!(
            masked_digest(&buf, 1920, 1080, &boxes),
            want,
            "{text_box:?}"
        );
    }
}

// ── App icons (`app-icons`): the chip's letter is its icon ──────────────────

/// Every pixel of a chip's icon box (above the name) is the pinned blob
/// blended over the bare chip pixel, exactly: `out = (s·a + d·(255 − a) +
/// 127)/255` per channel. `bare` is the base view pixel (labels are drawn
/// over it); the box's top two rows lie above the label box and stay bare.
#[cfg(feature = "app-icons")]
fn check_icon_box(
    buf: &[u32],
    stride: usize,
    chip: Rect,
    label: Rect,
    blob: &[u8; crate::icons::SIZE * crate::icons::SIZE * 4],
    bare: impl Fn(usize, usize) -> u32,
) -> Result<usize, String> {
    let at = crate::icons::rect(chip);
    let mut inked = 0;
    // The name's glyphs start at row chip.y + 45 at the earliest (baseline
    // 53, cap height under 8 px at 10 px); the icon's ink ends above it.
    for y in at.y..chip.y + 45 {
        for x in at.x..at.right() {
            let i = ((y - at.y) * crate::icons::SIZE + (x - at.x)) * 4;
            let px = [blob[i], blob[i + 1], blob[i + 2], blob[i + 3]];
            let d = bare(x, y);
            let want = if label.contains(x, y) {
                crate::icons::blend_pixel(d, px)
            } else {
                d
            };
            if buf[y * stride + x] != want {
                return Err(format!(
                    "({x},{y}) 0x{:06x} want 0x{want:06x}",
                    buf[y * stride + x]
                ));
            }
            inked += usize::from(px[3] != 0);
        }
    }
    Ok(inked)
}

/// Home, both materials, both focus states: Arthur's chip shows the Arthur
/// icon exactly, and its name is drawn below it, never over it.
#[cfg(feature = "app-icons")]
#[test]
fn icon_chips_show_the_pinned_blob_over_the_chip() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        for focused in [false, true] {
            let (w, h) = (1920, 1080);
            let scene = BaseScene::with_home(w, h, 640, 400, mode).unwrap();
            let mut buf = vec![0u32; w * h];
            scene.paint_home_rect(&mut buf, w, focused, "", scene.layout.screen);
            let l = scene.layout;
            let view = crate::View::Home {
                focused,
                held: false,
                chip_selected: false,
            };
            let bare = |x, y| crate::wide::dither(scene.view_pixel(x, y, view), x, y);
            let inked = check_icon_box(&buf, w, l.chip, l.chip_label, crate::icons::ARTHUR, bare)
                .unwrap_or_else(|e| panic!("{mode:?} focused={focused}: {e}"));
            assert!(inked > 200, "{mode:?}: the icon has ink ({inked})");
            // The name: drawn in the label box's rows below the icon.
            let below: usize = (l.chip.y + 45..l.chip_label.bottom())
                .map(|y| {
                    (l.chip_label.x..l.chip_label.right())
                        .filter(|&x| buf[y * w + x] != bare(x, y))
                        .count()
                })
                .sum();
            assert!(
                below > 20,
                "{mode:?}: the name is drawn below the icon ({below})"
            );
        }
    }
}

/// The Lens chip (DS-1c), in the window view where it shows: the Lens icon,
/// exactly, in both materials.
#[cfg(feature = "app-icons")]
#[test]
fn icon_chips_show_the_lens_icon_on_the_lens_chip() {
    for mode in [Mode::Glass, Mode::ReduceTransparency] {
        let (w, h) = (1920, 1080);
        let mut scene = BaseScene::with_lens(w, h, 640, 400, mode).unwrap();
        assert!(scene.set_lens(true));
        let mut buf = vec![0u32; w * h];
        scene.paint(&mut buf, w, true);
        let l = scene.layout;
        let bare = |x, y| crate::wide::dither(scene.wide_pixel(x, y, true), x, y);
        for (chip, label, blob) in [
            (l.chip, l.chip_label, crate::icons::ARTHUR),
            (l.chip2, l.chip2_label, crate::icons::LENS),
        ] {
            check_icon_box(&buf, w, chip, label, blob, bare)
                .unwrap_or_else(|e| panic!("{mode:?}: {e}"));
        }
        // Negative control: the other icon on the Lens chip fails.
        assert!(
            check_icon_box(&buf, w, l.chip2, l.chip2_label, crate::icons::ARTHUR, bare).is_err()
        );
    }
}

/// The blend's hand-computed values: a = 0 keeps the chip, a = 255 is the
/// icon, and a = 128 of white over 0x203040 is (255·128 + d·127 + 127)/255
/// per channel: 0x20 → 0x90, 0x30 → 0x98, 0x40 → 0xa0.
#[cfg(feature = "app-icons")]
#[test]
fn icon_blend_byte_oracle() {
    use crate::icons::blend_pixel;
    assert_eq!(blend_pixel(0x0020_3040, [0xff, 0xff, 0xff, 0]), 0x0020_3040);
    assert_eq!(
        blend_pixel(0x0020_3040, [0x3c, 0x8e, 0xf4, 0xff]),
        0x003c_8ef4
    );
    assert_eq!(
        blend_pixel(0x0020_3040, [0xff, 0xff, 0xff, 0x80]),
        0x0090_98a0
    );
}

#[cfg(feature = "app-icons")]
mod icon_oracle {
    include!("../../../fixtures/desktop-ds1/icon_digests.rs");
}

/// FNV-1a over `(x, y, w, h)` of `buf`, each pixel as 4 little-endian bytes
/// (the kernel's `fb::pixel_digest` and scene_oracle.py's byte format).
#[cfg(feature = "app-icons")]
fn box_digest(buf: &[u32], stride: usize, (x0, y0, w, h): (usize, usize, usize, usize)) -> u64 {
    let mut hsh: u64 = 0xCBF2_9CE4_8422_2325;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            for b in (buf[y * stride + x] & 0x00FF_FFFF).to_le_bytes() {
                hsh ^= u64::from(b);
                hsh = hsh.wrapping_mul(0x0000_0100_0000_01B3);
            }
        }
    }
    hsh
}

/// The Rust Home view's icon box equals the independent oracle's (the
/// pinned blob over scene_oracle.py's own chip pixels), in both materials:
/// the digest the `desktop_icons_boot` scenario reads back.
#[cfg(feature = "app-icons")]
#[test]
fn icon_box_matches_independent_python_oracle() {
    use icon_oracle::*;
    let l = Layout::new(1920, 1080, 640, 400).unwrap();
    let r = crate::icons::rect(l.chip);
    assert_eq!(ICON_BOX_1920X1080, (r.x, r.y, r.w, l.chip.y + 45 - r.y));
    for (mode, focused, want) in [
        (Mode::Glass, false, ARTHUR_ICON_GLASS_1920X1080_UNFOCUSED),
        (Mode::Glass, true, ARTHUR_ICON_GLASS_1920X1080_FOCUSED),
        (
            Mode::ReduceTransparency,
            false,
            ARTHUR_ICON_RT_1920X1080_UNFOCUSED,
        ),
        (
            Mode::ReduceTransparency,
            true,
            ARTHUR_ICON_RT_1920X1080_FOCUSED,
        ),
    ] {
        let (buf, _) = render_home(1920, 1080, mode, focused, "");
        assert_eq!(
            box_digest(&buf, 1920, ICON_BOX_1920X1080),
            want,
            "{mode:?} focused={focused}"
        );
    }
    assert_ne!(
        ARTHUR_ICON_GLASS_1920X1080_FOCUSED,
        WRONG_BLOB_GLASS_1920X1080_FOCUSED
    );
}

// ── OT-87 R4.4b: the Install chip ───────────────────────────────────────────

#[cfg(feature = "install-chip")]
mod install_chip {
    use super::*;

    // The icon box's digests are read only with `app-icons`.
    #[allow(dead_code)]
    mod install_digests {
        include!("../../../fixtures/desktop-ds1/install_digests.rs");
    }

    fn install_mask_boxes(l: &Layout) -> Vec<Rect> {
        let mut b = l.home_label_boxes().to_vec();
        b.push(l.chip3_label);
        b
    }

    fn paint_install_home(scene: &BaseScene, view: crate::View) -> Vec<u32> {
        let (w, h) = (scene.layout.screen.w, scene.layout.screen.h);
        let mut buf = vec![0u32; w * h];
        scene.paint_home_view(&mut buf, w, view, "", scene.layout.screen);
        buf
    }

    /// Home with the Install chip equals the independent oracle outside its
    /// label boxes: the field lit, Arthur held, the selection on the Install
    /// chip, and on Arthur's chip with the Install chip shown, in both
    /// materials.
    #[test]
    fn install_chip_matches_independent_python_oracle() {
        use install_digests::*;
        let l = Layout::new(1920, 1080, 640, 400).unwrap();
        let boxes: Vec<_> = install_mask_boxes(&l).iter().map(tuple).collect();
        assert_eq!(boxes, R44B_INSTALL_MASK_BOXES_1920X1080);
        let home = |held, chip_selected| crate::View::Home {
            focused: true,
            held,
            chip_selected,
        };
        for (mode, field, held, selected, arthur) in [
            (
                Mode::Glass,
                R44B_HOME_INSTALL_GLASS_1920X1080_FOCUSED,
                R44B_HOME_INSTALL_GLASS_1920X1080_HELD,
                R44B_HOME_INSTALL_GLASS_1920X1080_CHIP_SELECTED,
                R44B_HOME_INSTALL_GLASS_1920X1080_ARTHUR_SELECTED,
            ),
            (
                Mode::ReduceTransparency,
                R44B_HOME_INSTALL_RT_1920X1080_FOCUSED,
                R44B_HOME_INSTALL_RT_1920X1080_HELD,
                R44B_HOME_INSTALL_RT_1920X1080_CHIP_SELECTED,
                R44B_HOME_INSTALL_RT_1920X1080_ARTHUR_SELECTED,
            ),
        ] {
            let mut scene = BaseScene::with_home(1920, 1080, 640, 400, mode).unwrap();
            assert!(scene.set_install(true));
            let masks = install_mask_boxes(&scene.layout);
            let d = |buf: &[u32]| masked_digest(buf, 1920, 1080, &masks);
            assert_eq!(
                d(&paint_install_home(&scene, home(false, false))),
                field,
                "{mode:?}"
            );
            assert_eq!(
                d(&paint_install_home(&scene, home(true, false))),
                held,
                "{mode:?}"
            );
            assert_eq!(
                d(&paint_install_home(&scene, home(true, true))),
                arthur,
                "{mode:?}"
            );
            assert!(scene.set_install_selected(true));
            assert_eq!(
                d(&paint_install_home(&scene, home(true, true))),
                selected,
                "{mode:?}"
            );
        }
    }

    /// With the Lens alive as well: both chips, against the oracle.
    #[test]
    fn install_chip_beside_the_lens_chip_matches_independent_python_oracle() {
        use install_digests::*;
        let l = Layout::new(1920, 1080, 640, 400).unwrap();
        let mut boxes = install_mask_boxes(&l);
        boxes.push(l.chip2_label);
        let tuples: Vec<_> = boxes.iter().map(tuple).collect();
        assert_eq!(tuples, R44B_INSTALL_LENS_MASK_BOXES_1920X1080);
        for (mode, want) in [
            (Mode::Glass, R44B_HOME_INSTALL_LENS_GLASS_1920X1080_HELD),
            (
                Mode::ReduceTransparency,
                R44B_HOME_INSTALL_LENS_RT_1920X1080_HELD,
            ),
        ] {
            let mut scene = BaseScene::with_lens(1920, 1080, 640, 400, mode).unwrap();
            assert!(scene.set_lens(true));
            assert!(scene.set_install(true));
            let buf = paint_install_home(
                &scene,
                crate::View::Home {
                    focused: true,
                    held: true,
                    chip_selected: false,
                },
            );
            assert_eq!(masked_digest(&buf, 1920, 1080, &boxes), want, "{mode:?}");
        }
    }

    /// Off (the default, and after it was shown) is DS-1's Home, bit for bit,
    /// labels included; and toggling the chip or its selection changes only the
    /// Spine.
    #[test]
    fn install_chip_off_is_home_bit_for_bit_and_stays_in_the_spine() {
        for mode in [Mode::Glass, Mode::ReduceTransparency] {
            let plain_scene = BaseScene::with_home(1280, 800, 640, 400, mode).unwrap();
            let view = crate::View::Home {
                focused: true,
                held: true,
                chip_selected: true,
            };
            let plain = paint_install_home(&plain_scene, view);
            let mut scene = BaseScene::with_home(1280, 800, 640, 400, mode).unwrap();
            assert!(scene.set_install(true));
            assert!(scene.set_install_selected(true));
            let shown = paint_install_home(&scene, view);
            let l = scene.layout;
            for y in 0..800 {
                for x in 0..1280 {
                    if plain[y * 1280 + x] != shown[y * 1280 + x] {
                        assert!(
                            l.spine.contains(x, y),
                            "{mode:?} ({x},{y}) outside the Spine"
                        );
                    }
                }
            }
            assert_ne!(plain, shown, "{mode:?}: the chip shows");
            assert!(scene.set_install(false));
            assert!(
                !scene.install_selected(),
                "ending the chip ends its selection"
            );
            assert_eq!(
                paint_install_home(&scene, view),
                plain,
                "{mode:?}: off again"
            );
        }
    }

    /// Only a Home scene shows the chip, and only a shown chip is selected.
    #[test]
    fn install_chip_needs_home_and_selection_needs_the_chip() {
        let mut window = BaseScene::new(1280, 800, 640, 400, Mode::Glass).unwrap();
        assert!(!window.set_install(true));
        assert!(!window.install_shown());
        let mut home = BaseScene::with_home(1280, 800, 640, 400, Mode::Glass).unwrap();
        assert!(!home.set_install_selected(true), "no chip, no selection");
        assert!(home.set_install(true));
        assert!(home.install_shown());
        assert!(home.set_install_selected(true));
        assert!(home.install_selected());
        assert_eq!(home.layout.chip3, Rect::new(14, 206, 56, 56));
    }

    /// `app-icons`: the Install chip shows #185's Install blob, exactly, over
    /// the chip; the oracle's icon box digest agrees; another blob fails.
    #[cfg(feature = "app-icons")]
    #[test]
    fn install_chip_shows_the_install_icon() {
        use install_digests::*;
        for (mode, want) in [
            (Mode::Glass, R44B_INSTALL_ICON_GLASS_1920X1080_FOCUSED),
            (
                Mode::ReduceTransparency,
                R44B_INSTALL_ICON_RT_1920X1080_FOCUSED,
            ),
        ] {
            let mut scene = BaseScene::with_home(1920, 1080, 640, 400, mode).unwrap();
            assert!(scene.set_install(true));
            let view = crate::View::home(true);
            let buf = paint_install_home(&scene, view);
            let l = scene.layout;
            let r = crate::icons::rect(l.chip3);
            assert_eq!(
                R44B_INSTALL_ICON_BOX_1920X1080,
                (r.x, r.y, r.w, l.chip3.y + 45 - r.y)
            );
            assert_eq!(
                box_digest(&buf, 1920, R44B_INSTALL_ICON_BOX_1920X1080),
                want,
                "{mode:?}"
            );
            let bare = |x, y| crate::wide::dither(scene.view_pixel(x, y, view), x, y);
            let inked = check_icon_box(
                &buf,
                1920,
                l.chip3,
                l.chip3_label,
                crate::icons::INSTALL,
                bare,
            )
            .unwrap_or_else(|e| panic!("{mode:?}: {e}"));
            assert!(inked > 100, "{mode:?}: the icon has ink ({inked})");
            assert!(
                check_icon_box(
                    &buf,
                    1920,
                    l.chip3,
                    l.chip3_label,
                    crate::icons::ARTHUR,
                    bare
                )
                .is_err()
            );
        }
    }

    /// Without `app-icons`: the letter and the name are drawn in the chip's
    /// label box, and nothing of it outside.
    #[cfg(not(feature = "app-icons"))]
    #[test]
    fn install_chip_shows_its_letter() {
        let mut scene = BaseScene::with_home(1920, 1080, 640, 400, Mode::Glass).unwrap();
        assert!(scene.set_install(true));
        let view = crate::View::home(true);
        let buf = paint_install_home(&scene, view);
        let l = scene.layout;
        let bare = |x: usize, y: usize| crate::wide::dither(scene.view_pixel(x, y, view), x, y);
        let mut inked = 0;
        for y in l.chip3.y..l.chip3.bottom() {
            for x in l.chip3.x..l.chip3.right() {
                if buf[y * 1920 + x] != bare(x, y) {
                    assert!(
                        l.chip3_label.contains(x, y),
                        "({x},{y}) ink outside the label box"
                    );
                    inked += 1;
                }
            }
        }
        assert!(inked > 30, "the letter and the name are drawn ({inked})");
    }

    /// The intro session's Home end view with the chip (the hint under the
    /// field): the oracle's digest the `install_atrium` kernel reads back.
    #[test]
    fn install_chip_with_the_hint_matches_independent_python_oracle() {
        use install_digests::*;
        for (mode, want) in [
            (Mode::Glass, R44B_HOME_INSTALL_HINT_GLASS_1920X1080),
            (
                Mode::ReduceTransparency,
                R44B_HOME_INSTALL_HINT_RT_1920X1080,
            ),
        ] {
            let mut scene = BaseScene::with_lens(1920, 1080, 640, 400, mode).unwrap();
            scene.set_intro(true);
            assert!(scene.set_install(true));
            let mut buf = vec![0u32; 1920 * 1080];
            scene.paint_home_rect(&mut buf, 1920, true, "", scene.layout.screen);
            let l = scene.layout;
            let hb = l.home_label_boxes();
            let boxes = [hb[0], hb[1], hb[2], scene.intro_text_box(), l.chip3_label];
            let tuples: Vec<_> = boxes.iter().map(tuple).collect();
            assert_eq!(tuples, R44B_INSTALL_HINT_MASK_BOXES_1920X1080);
            assert_eq!(masked_digest(&buf, 1920, 1080, &boxes), want, "{mode:?}");
        }
    }
}

/// #148 F1: each installer button is hit on its own drawn label. The button
/// row keeps its pre-#148 text, the two label boxes are disjoint with a dead
/// zone between them, and the old equal-halves split would have sent the
/// drawn Back label to Go (both labels are left of the card's midpoint).
#[test]
fn installer_buttons_hit_their_drawn_labels() {
    use crate::intro::button_row;
    assert_eq!(button_row("Choose"), "[ Choose ]                 [ Back ]");
    assert_eq!(button_row("Install"), "[ Install ]                [ Back ]");
    assert_eq!(
        button_row("Finish installing"),
        "[ Finish installing ]                [ Back ]"
    );
    let mut scene = BaseScene::with_lens(1920, 1080, 640, 400, Mode::Glass).unwrap();
    assert!(scene.set_intro(true));
    // The points `install_atrium` clicks (xtask's `HMP_TO_*` moves): Choose
    // on the list, then Back's centre on the confirmation and on the list.
    let centre = |r: Rect| (r.x + r.w / 2, r.y + r.h / 2);
    let clicks = [
        ((700, 617), None),
        ((819, 593), Some(3)),
        ((836, 617), Some(4)),
    ];
    for (rows, at) in [
        (
            vec![
                "Install LogansOS: choose a disk".into(),
                "x boot".into(),
                "  other".into(),
                "> target".into(),
                button_row("Choose"),
                "hint".into(),
            ],
            4,
        ),
        (
            vec![
                "Install LogansOS on target?".into(),
                "detail".into(),
                String::new(),
                button_row("Install"),
                "Enter goes ahead. Esc goes back.".into(),
            ],
            3,
        ),
    ] {
        scene.set_card(rows);
        let card = scene.layout.card(scene.card_rows());
        let [go, back] = scene.card_buttons(at).unwrap();
        if at == 4 {
            assert!(go.contains(clicks[0].0.0, clicks[0].0.1), "{go:?}");
        }
        for (point, row) in clicks {
            if row == Some(at) {
                assert_eq!(centre(back), point, "{back:?}");
            }
        }
        let text = scene.layout.card_text(scene.card_rows());
        assert_eq!(go.x, text.x, "Go starts where the row is drawn");
        assert_eq!(
            (go.y, go.h),
            (
                text.y + at * crate::layout::CARD_LINE_H,
                crate::layout::CARD_LINE_H
            )
        );
        assert_eq!((back.y, back.h), (go.y, go.h));
        assert!(go.w > 0 && back.w > 0);
        assert!(go.x + go.w + 40 < back.x, "a dead zone between the labels");
        assert!(
            back.x + back.w < card.x + card.w / 2,
            "#148: Back left of the midpoint"
        );
        assert!(
            scene.card_buttons(at - 1).is_none(),
            "a row without buttons"
        );
    }
}

/// OT-88 S1: Reduce Transparency applied live. A scene switched to a mode
/// paints what a scene built in it paints: Home and the window view, at the
/// built place and after a move, both ways.
#[test]
fn set_mode_live_equals_a_scene_built_in_that_mode() {
    let (w, h) = (1280, 800);
    for (from, to) in [
        (Mode::ReduceTransparency, Mode::Glass),
        (Mode::Glass, Mode::ReduceTransparency),
    ] {
        let mut live = BaseScene::with_lens(w, h, 640, 400, from).unwrap();
        assert!(live.set_mode(to));
        assert!(!live.set_mode(to), "a second switch changes nothing");
        let built = BaseScene::with_lens(w, h, 640, 400, to).unwrap();
        let paint = |s: &BaseScene| {
            let mut home = vec![0u32; w * h];
            s.paint_home_rect(&mut home, w, false, "", s.layout.screen);
            let mut window = vec![0u32; w * h];
            s.paint_rect(&mut window, w, true, s.layout.screen);
            (home, window)
        };
        assert!(paint(&live) == paint(&built), "{from:?} -> {to:?}");
        // After a move: the live scene against one built at that place.
        let (x, y) = (live.layout.frame.x + 40, live.layout.frame.y + 24);
        assert!(live.move_frame(x, y));
        let mut moved = BaseScene::with_lens(w, h, 640, 400, from).unwrap();
        assert!(moved.move_frame(x, y));
        assert!(moved.set_mode(to));
        let placed = BaseScene::with_lens_at(w, h, 640, 400, to, (x, y)).unwrap();
        assert!(paint(&moved) == paint(&placed), "{from:?} -> {to:?} moved");
    }
}

/// OT-90 P5: which lines ask to shut down or restart, and the confirm card.
mod power {
    use crate::power::*;

    #[test]
    fn the_typed_commands_are_recognised() {
        for line in [
            &b"shut down"[..],
            b"Shut Down",
            b"  shut   down  ",
            b"shutdown",
            b"POWER OFF",
            b"turn off",
        ] {
            assert_eq!(
                power_command(line),
                Some(PowerCommand::ShutDown),
                "{line:?}"
            );
        }
        for line in [&b"restart"[..], b"Reboot", b" restart "] {
            assert_eq!(power_command(line), Some(PowerCommand::Restart), "{line:?}");
        }
        for line in [
            &b""[..],
            b"shut",
            b"shut down now",
            b"please restart",
            b"restart the router",
            b"help",
            b"shut-down",
        ] {
            assert_eq!(power_command(line), None, "{line:?}");
        }
    }

    /// Jarvis's #14 review, item 1: on the confirm card only Enter goes
    /// ahead; Esc and every other key the view can receive go back.
    #[test]
    fn only_enter_confirms_a_power_action() {
        use crate::keys::Key;
        assert!(confirm_key_goes_ahead(Key::Enter));
        for key in [
            Key::Escape,
            Key::Char(b'a'),
            Key::Char(b'y'),
            Key::Char(b' '),
            Key::Backspace,
            Key::Tab,
            Key::Up,
            Key::Down,
        ] {
            assert!(!confirm_key_goes_ahead(key), "{key:?}");
        }
        assert!(confirm_lines(PowerCommand::ShutDown)[2].contains("any other key goes back"));
    }

    #[test]
    fn the_actions_are_the_abi_numbers() {
        assert_eq!(PowerCommand::ShutDown.action(), 1);
        assert_eq!(PowerCommand::Restart.action(), 2);
    }

    #[test]
    fn the_confirm_card_ends_in_its_buttons() {
        for (cmd, go) in [
            (PowerCommand::ShutDown, "[ Shut down ]"),
            (PowerCommand::Restart, "[ Restart ]"),
        ] {
            let rows = confirm_lines(cmd);
            assert_eq!(rows.len(), CONFIRM_BUTTON_ROW + 1);
            assert!(rows[CONFIRM_BUTTON_ROW].starts_with(go), "{rows:?}");
            assert!(rows[CONFIRM_BUTTON_ROW].ends_with("[ Back ]"), "{rows:?}");
        }
        assert!(refused_lines(-16)[0].contains("already"));
        assert!(refused_lines(-38)[0].contains("can't"));
    }
}
