// SPDX-License-Identifier: MPL-2.0
//! OT-61 L0 + L1 — `layout-smoke` (`docs/THINK-2026-11-01-ot61-first-page-layout.md`
//! §5; J.c.'s option A, `Plans/DECISION-2026-11-02-ot61-first-page-option-a.md`).
//!
//! L0: this binary links patched `servo-layout` over `neo-layout-dom`, with no
//! `servo-script` and no SpiderMonkey, so building it for the target is the
//! rung's gate. It names layout's entry point and parses a document into the
//! host DOM to show both are present.
//!
//! L1: servo-fonts' `neoplutarch` backend reads the bundled Roboto Slab, and
//! every value it reports at 16 px must equal one derived by hand from the
//! font's own tables. Those tables were read by `oracle/font_tables.py`, an
//! independent parser of the raw file (no font library), not by skrifa, the
//! code under test:
//!
//! ```text
//! upem=2048 ascender=2146 descender=-555 lineGap=0 advanceWidthMax=2543
//! underlinePosition=-150 underlineThickness=100
//! xAvgCharWidth=1240 yStrikeoutSize=100 yStrikeoutPosition=649 sxHeight=1082
//! ```
//!
//! At 16 px a font unit is 16 / 2048 = 1/128 px, and an `Au` is 1/60 px,
//! rounded half away from zero (`Au::from_f64_px`). Each expected `Au` below
//! shows its arithmetic.
//!
//! Exit 0 on a pass; a failed check prints `FAIL(n)` and exits `n`.

#![no_main]
#![feature(random)]

mod l3;
mod l4;

use app_units::Au;
use fonts::PlatformFontMethods;
use fonts::platform::font::PlatformFont;
use fonts_traits::LocalFontIdentifier;
use style::Atom;

// The heap is servo-allocator's global allocator, which on this target is
// neo-std-pal's `SyscallHeap` (its patch, OT-61 L3).
const TAG: &str = "[ layout-smoke ]";

const ROBOTO_SLAB: &str = "neo-font:/robotoslab/RobotoSlab-wght.ttf";

/// "Hello, LogansOS 0": each character's glyph id (cmap format 4) and advance
/// (hmtx), in font units, from `oracle/font_tables.py`.
const GLYPHS: &[(char, u32, u32)] = &[
    ('H', 63, 1578),
    ('e', 227, 1059),
    ('l', 275, 653),
    ('o', 290, 1124),
    (',', 977, 404),
    (' ', 958, 509),
    ('L', 83, 1210),
    ('g', 247, 1157),
    ('a', 190, 1132),
    ('n', 283, 1343),
    ('s', 322, 1022),
    ('O', 97, 1351),
    ('S', 129, 1206),
    ('0', 906, 1168),
];

fn check(code: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok({code}) {what}");
    } else {
        println!("{TAG} FAIL({code}) {what}");
        std::process::exit(code);
    }
}

fn au(v: Au) -> i32 {
    v.0
}

/// getrandom 0.4's custom backend (`.cargo/config.toml` sets
/// `getrandom_backend="custom"`; uuid and rand take this path): the kernel
/// CSPRNG through std's own entropy source, as neo-crt's `arc4random_buf`
/// does, so it adds no syscall site.
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), getrandom04::Error> {
    use std::random::{DefaultRandomSource, RandomSource};
    if len != 0 {
        DefaultRandomSource.fill_bytes(unsafe { std::slice::from_raw_parts_mut(dest, len) });
    }
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // The C/C++ runtime HarfBuzz, OTS and zlib run on (and the reason it's
    // linked at all: an unnamed dependency would be dropped).
    neo_crt::init();
    println!("{TAG} start: servo-layout 0.7.0 over neo-layout-dom, no servo-script");

    // L0: layout's entry point and the host DOM are linked in.
    let _factory = layout::LayoutFactoryImpl();
    let url = style::stylesheets::UrlExtraData::from(
        url::Url::parse("about:blank").expect("about:blank"),
    );
    let doc = neo_layout_dom::Document::parse("<p id=a>Hello</p>", &url);
    check(
        1,
        "neo-layout-dom parsed a page (document, html, head, body, p, text)",
        doc.len() == 6 && doc.element_by_id("a").is_some(),
    );

    // L1: the bundled font through servo-fonts' neoplutarch backend.
    let id = LocalFontIdentifier {
        path: Atom::from(ROBOTO_SLAB),
        face_index: 0,
        named_instance_index: 0,
    };
    let font = PlatformFont::new_from_local_font_identifier(id, Some(Au::from_px(16)), false);
    check(2, "Roboto Slab opens from the bundled store", font.is_ok());
    let font = font.expect("checked");

    for (i, &(ch, glyph, advance)) in GLYPHS.iter().enumerate() {
        let code = 10 + i as i32;
        let got_glyph = font.glyph_index(ch);
        let got_advance = got_glyph.and_then(|g| font.glyph_h_advance(g));
        // advance / 128 is exact in binary for these values.
        let want_advance = advance as f64 / 128.0;
        println!("{TAG} {ch:?}: glyph {got_glyph:?} advance {got_advance:?} px");
        check(
            code,
            &format!("{ch:?} is glyph {glyph}, advance {advance}/128 = {want_advance} px"),
            got_glyph == Some(glyph) && got_advance == Some(want_advance),
        );
    }
    check(
        30,
        "U+6C34 (not in the font) has no glyph",
        font.glyph_index('\u{6C34}').is_none(),
    );

    let m = font.metrics();
    println!("{TAG} metrics: {m:?}");
    // Each expected value: font units / 128 = px, then px * 60 rounded = Au.
    let expect: &[(i32, &str, Au, i32)] = &[
        // 2048/128 = 16 px; 16 * 60 = 960.
        (31, "em size", m.em_size, 960),
        // 2146/128 = 16.765625 px; * 60 = 1005.9375 -> 1006.
        (32, "ascent", m.ascent, 1006),
        // 555/128 = 4.3359375 px; * 60 = 260.15625 -> 260.
        (33, "descent", m.descent, 260),
        // (2146 + 555 + 0)/128 = 21.1015625 px; * 60 = 1266.09375 -> 1266.
        (34, "line gap (FreeType's face height)", m.line_gap, 1266),
        // 21.1015625 - (16.765625 + 4.3359375) = 0.
        (35, "leading", m.leading, 0),
        // 1082/128 = 8.453125 px; * 60 = 507.1875 -> 507.
        (36, "x-height (OS/2 sxHeight)", m.x_height, 507),
        // 2543/128 = 19.8671875 px; * 60 = 1192.03125 -> 1192.
        (37, "max advance", m.max_advance, 1192),
        // 1240/128 = 9.6875 px; * 60 = 581.25 -> 581.
        (
            38,
            "average advance (OS/2 xAvgCharWidth)",
            m.average_advance,
            581,
        ),
        // 100/128 = 0.78125 px; * 60 = 46.875 -> 47.
        (39, "underline size", m.underline_size, 47),
        // -150/128 + 0.5 = -0.671875 px; * 60 = -40.3125 -> -40.
        (40, "underline offset", m.underline_offset, -40),
        // 100/128 = 0.78125 px -> 47.
        (41, "strikeout size", m.strikeout_size, 47),
        // 649/128 = 5.0703125 px; * 60 = 304.21875 -> 304.
        (42, "strikeout offset", m.strikeout_offset, 304),
        // ' ' 509/128 = 3.9765625 px; * 60 = 238.59375 -> 239.
        (43, "space advance", m.space_advance, 239),
    ];
    for &(code, what, got, want) in expect {
        check(code, &format!("{what} = {want} Au"), au(got) == want);
    }
    // '0' 1168/128 = 9.125 px; * 60 = 547.5 -> 548 (half away from zero).
    check(
        44,
        "zero advance = 548 Au",
        m.zero_horizontal_advance.map(au) == Some(548),
    );
    check(
        45,
        "no U+6C34 advance (the glyph isn't there)",
        m.ic_horizontal_advance.is_none(),
    );

    // L2 runs on its own thread with the 8 MiB stack Servo gives the script
    // thread layout runs on (servo-script 0.7.0, script_thread.rs:490). This
    // thread stands in for script: it creates the page, so its thread state
    // is SCRIPT. On the process's 64 KiB main stack the first reflow
    // overflowed into the guard page (ring-3 #PF 83 KiB below the stack top,
    // the third on-target L2 run, 2026-11-03).
    let l2 = std::thread::Builder::new()
        .name("static-page".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            // L2: one reflow of a block-only page through neo-static-page (layout
            // driven without script), its border boxes checked against the
            // hand-worked oracle (neo-static-page/src/oracle.rs, CSS 2.1 8.3.1 and
            // 10.3.3; no text, so no font metric enters the numbers).
            let (w, h) = neo_static_page::oracle::VIEWPORT;
            let mut page = neo_static_page::StaticPage::new(neo_static_page::oracle::PAGE, w, h);
            check(
                50,
                "one reflow of the oracle page produced a result",
                page.reflow(w, h),
            );
            for (i, &(id, want)) in neo_static_page::oracle::EXPECT.iter().enumerate() {
                let got = page.border_box(id);
                println!("{TAG} #{id} border box: {got:?}");
                check(
                    51 + i as i32,
                    &format!("#{id} border box = {want:?} (x, y, w, h px)"),
                    got == Some(want),
                );
            }
            check(
                56,
                "layout sent a display list to the in-process paint host",
                page.display_lists() >= 1,
            );
            // L3: that display list, rendered by WebRender on SWGL.
            l3::render_and_check(&page);
            drop(page);
            // L4: a line of text in the bundled font, through the same path.
            let mut text = neo_static_page::StaticPage::new(l4::PAGE, w, h);
            check(
                70,
                "one reflow of the text page produced a result",
                text.reflow(w, h),
            );
            l4::render_and_check(&text);
        })
        .expect("spawn the static-page thread");
    check(57, "the static-page thread finished", l2.join().is_ok());

    println!("{TAG} PASS");
    std::process::exit(0)
}
