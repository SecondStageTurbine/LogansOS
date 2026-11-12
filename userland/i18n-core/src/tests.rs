// SPDX-License-Identifier: MPL-2.0
//! The catalog's host KATs (S6a, `Plans/DISPATCH-2026-11-09-seat-c-setup-s6a-i18n-catalog.md`;
//! S6b, `Plans/DISPATCH-2026-11-12-seat-c-setup-s6b-language.md`):
//!
//! 1. every id has an entry in both tables, with the same `{}` count, and no
//!    lookup falls back;
//! 2. the English table equals the pinned snapshot (`fixtures/en.txt`), and
//!    the code that shows these surfaces holds no user-visible literal
//!    outside the catalog (`stray.rs`);
//! 3. every character of every entry is in the face the text engine draws
//!    with (Inter Regular, read from its cmap, not assumed);
//! 4. every row, filled with its widest realistic arguments, fits the card's
//!    text box at both reference screens, in both languages, measured with
//!    that face at the card's size.

extern crate std;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use std::format;

use super::*;

// ---- 1. Both tables complete ----

#[test]
fn every_id_has_an_entry_in_both_tables_and_none_falls_back() {
    assert_eq!(Msg::ALL.len(), MSG_COUNT);
    for (i, &m) in Msg::ALL.iter().enumerate() {
        assert_eq!(m as usize, i, "{m:?} is out of table order");
        for lang in Lang::ALL {
            let (s, fell_back) = lookup(lang, m);
            assert!(!fell_back, "{m:?} falls back to English in {lang:?}");
            assert!(!s.is_empty(), "{m:?} is empty in {lang:?}");
        }
        assert_eq!(
            arg_count(text(Lang::En, m)),
            arg_count(text(Lang::Es, m)),
            "{m:?}: the two tables take different argument counts"
        );
    }
    assert_eq!(Lang::ALL.len(), LANGUAGE_COUNT);
    for (i, lang) in Lang::ALL.iter().enumerate() {
        assert_eq!(Lang::from_index(i), Some(*lang));
    }
    assert_eq!(Lang::from_index(LANGUAGE_COUNT), None);
}

#[test]
fn the_fallback_takes_english_for_an_empty_entry() {
    // No entry is empty today (the test above), so check the rule on a
    // stand-in table of the same shape.
    let en = ["One", "Two"];
    let other = ["Uno", ""];
    let pick = |i: usize| {
        if other[i].is_empty() {
            (en[i], true)
        } else {
            (other[i], false)
        }
    };
    assert_eq!(pick(0), ("Uno", false));
    assert_eq!(pick(1), ("Two", true));
    // And the real lookup's English is its own table.
    assert_eq!(lookup(Lang::En, Msg::SetupBegin), ("Begin", false));
    assert_eq!(text(Lang::Es, Msg::SetupBegin), "Comenzar");
}

#[test]
fn fill_is_positional_and_leaves_a_missing_argument_visible() {
    assert_eq!(
        fill("{} ({} of {})", &["Time zone", "2", "4"]),
        "Time zone (2 of 4)"
    );
    assert_eq!(
        fill("{} ({} de {})", &["Zona horaria", "2", "4"]),
        "Zona horaria (2 de 4)"
    );
    assert_eq!(fill("Search: {}", &[]), "Search: {}");
    assert_eq!(fill("no arguments", &["spare"]), "no arguments");
    assert_eq!(
        format(Lang::Es, Msg::PowerError, &["-38"]),
        "(error -38) Presiona cualquier tecla."
    );
}

/// `{N}` takes argument `N`, so a language can order them its own way (the
/// date); text braces that are neither slot stay text.
#[test]
fn indexed_slots_reorder_and_count() {
    let args = ["miércoles", "octubre", "21", "2026"];
    assert_eq!(
        fill(text(Lang::Es, Msg::DateLong), &args),
        "miércoles 21 de octubre de 2026"
    );
    assert_eq!(
        fill(
            text(Lang::En, Msg::DateLong),
            &["Wednesday", "October", "21", "2026"]
        ),
        "Wednesday, October 21, 2026"
    );
    assert_eq!(arg_count("{0} {2} de {1} de {3}"), 4);
    assert_eq!(arg_count("{} and {}"), 2);
    assert_eq!(arg_count("{x} {"), 0);
    assert_eq!(fill("{x} {", &["a"]), "{x} {");
    assert_eq!(fill("{1}{0}", &["a"]), "{}a");
    assert_eq!(fill("é{}ñ", &["x"]), "éxñ");
    assert_eq!(
        pieces("a{}b{1}").collect::<Vec<_>>(),
        [
            Piece::Text("a"),
            Piece::Arg(0),
            Piece::Text("b"),
            Piece::Arg(1)
        ]
    );
}

// ---- 2. English is pinned ----

/// S6a's English table, as shipped (the literals the code printed before S6b
/// moved them here), plus the rows S6b and F1a added: one `Id<TAB>text` line per id,
/// in table order. Any English wording change edits this file on purpose.
const EN_PINNED: &str = include_str!("../fixtures/en.txt");

fn en_table() -> String {
    let mut out = String::new();
    for &m in Msg::ALL {
        out.push_str(&format!("{m:?}\t{}\n", text(Lang::En, m)));
    }
    out
}

#[test]
fn english_is_the_pinned_table() {
    let got = en_table();
    if got != EN_PINNED {
        let want: Vec<&str> = EN_PINNED.lines().collect();
        let diff: Vec<String> = got
            .lines()
            .enumerate()
            .filter(|&(i, l)| want.get(i) != Some(&l))
            .map(|(i, l)| format!("line {}: {l:?} (pinned: {:?})", i + 1, want.get(i)))
            .collect();
        panic!(
            "the English table differs from fixtures/en.txt:\n{}",
            diff.join("\n")
        );
    }
}

#[test]
#[ignore = "writes fixtures/en.txt's contents to stdout, for a deliberate wording change"]
fn print_en_table() {
    std::print!("{}", en_table());
}

// ---- 3. Every character is in the face ----

#[test]
fn every_character_is_in_the_face_the_text_engine_draws_with() {
    let font = neo_text::Font::from_bytes(neo_text::fonts::INTER_REGULAR).expect("Inter parses");
    let mut missing = Vec::new();
    let mut check = |where_: String, s: &str| {
        for c in s.chars() {
            if font.glyph_id(c) == 0 {
                missing.push(format!("U+{:04X} {c:?} in {where_}", c as u32));
            }
        }
    };
    for lang in Lang::ALL {
        check(format!("{lang:?} name"), lang.name());
        for &m in Msg::ALL {
            check(format!("{lang:?} {m:?}"), text(lang, m));
        }
    }
    assert!(
        missing.is_empty(),
        "glyphs missing from Inter Regular:\n{}",
        missing.join("\n")
    );
}

/// The non-ASCII characters the Spanish table uses, for the PR and the
/// notes (S6b relaxes the intro persona KAT's ASCII-only rule to these).
#[test]
fn spanish_uses_only_these_non_ascii_characters() {
    let mut seen: Vec<char> = Vec::new();
    for &m in Msg::ALL {
        for c in text(Lang::Es, m).chars().chain(Lang::Es.name().chars()) {
            if !c.is_ascii() && !seen.contains(&c) {
                seen.push(c);
            }
        }
    }
    seen.sort_unstable();
    let got: String = seen.into_iter().collect();
    assert_eq!(
        got, "¿áéíñóú",
        "update TRANSLATION-NOTES.md if this changes"
    );
}

// ---- 4. Every row fits the card ----

/// The widest realistic arguments: the longest IANA names `libneo::tz`
/// carries (30 bytes), a 32-byte typed name (`setup_core::QUERY_MAX`), the
/// widest offset label, the longest time and date the clock writes.
const ZONE: &str = "America/Argentina/Buenos_Aires";
const TYPED: &str = "America/Argentina/ComodRivadavia";
const OFFSET: &str = "AKDT, UTC-08:00";
/// A driver's name at the status call's 12 bytes (`libneo::net_if_status`).
const DRIVER: &str = "ax88179_178a";

/// `msg`'s row in `lang` as a card draws it: the focus mark on an option,
/// and the widest realistic arguments filled in.
fn widest_row(lang: Lang, m: Msg) -> String {
    use Msg::*;
    let t = |m: Msg| text(lang, m);
    let longest = |ms: &[Msg]| -> &'static str {
        ms.iter()
            .map(|&m| text(lang, m))
            .max_by_key(|s| s.chars().count())
            .unwrap()
    };
    let time = format!("12:59{}", longest(&[Am, Pm]));
    let date = fill(
        t(DateLong),
        &[
            longest(&[
                Sunday, Monday, Tuesday, Wednesday, Thursday, Friday, Saturday,
            ]),
            longest(&[
                January, February, March, April, May, June, July, August, September, October,
                November, December,
            ]),
            "30",
            "2026",
        ],
    );
    let on_off = longest(&[On, Off]);
    let args: Vec<String> = match m {
        SetupReduceMotionOption | SetupReduceTransparencyOption => vec!["[x]".into()],
        SetupTitlePage => vec![
            longest(&[
                SetupTitleNetwork,
                SetupTitleLanguage,
                SetupTitleKeyboard,
                SetupTitleTimeZone,
                SetupTitleComfort,
                SetupTitleTry,
            ])
            .into(),
            "5".into(),
            "5".into(),
        ],
        SetupBodyZoneSet
        | ZoneRow
        | SetupSummaryZoneClockNotSet
        | SetupNoticeZoneInUse
        | ClockZoneSet => vec![ZONE.into()],
        SetupBodySearchNone | SetupBodySearch | ClockNoZone => vec![TYPED.into()],
        SetupSummaryComfort => vec![on_off.into(), on_off.into()],
        IntroTooLittleMemory => vec!["16".into(), "32".into()],
        GreetingRow => vec![
            longest(&[GoodMorning, GoodAfternoon, GoodEvening]).into(),
            time,
            date,
        ],
        PowerError => vec!["-38".into()],
        DateLong => vec![
            "miércoles".into(),
            "septiembre".into(),
            "30".into(),
            "2026".into(),
        ],
        ZoneRowNow => vec![ZONE.into(), OFFSET.into()],
        // The test field at `setup_core::keyboard::FIELD_MAX` (24) of the
        // widest Latin capital.
        SetupKeyboardTryIt => vec!["W".repeat(24)],
        SetupSummaryKeyboard => vec![longest(&[LayoutEnglishUs, LayoutSpanishSpain]).into()],
        NetDevice => vec![DRIVER.into()],
        NetConnected => vec!["255.255.255.255".into()],
        NetDeviceLease => vec![
            DRIVER.into(),
            [
                t(LeaseUnderMinute).into(),
                fill(t(LeaseMinutes), &["59"]),
                fill(t(LeaseHours), &["23", "59"]),
            ]
            .into_iter()
            .max_by_key(|s: &String| s.chars().count())
            .unwrap(),
        ],
        LeaseMinutes => vec!["59".into()],
        // F1a: Files' numbers at their widest (a size column, the limits,
        // an errno).
        FilesSizeBytes => vec!["1023".into()],
        FilesSizeKb | FilesSizeMb => vec![format!("9{}9", t(FilesDecimalMark))],
        FilesItemsMany => vec!["64".into()],
        FilesFolderFullNote | FilesErrFolderFull => vec!["64".into()],
        FilesNameTooLong => vec!["47".into()],
        FilesCopyTooBig => vec!["32".into()],
        FilesErrOther => vec!["-116".into()],
        LeaseHours => vec!["23".into(), "59".into()],
        _ => Vec::new(),
    };
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let row = fill(t(m), &refs);
    assert_eq!(arg_count(&row), 0, "{m:?}: an argument is left unfilled");
    let option = matches!(
        m,
        SetupBegin
            | SetupSkip
            | SetupSkipDefaults
            | SetupBack
            | SetupContinue
            | SetupTryOption
            | SetupContinueNoNetwork
            | SetupReduceMotionOption
            | SetupReduceTransparencyOption
    );
    // A layout is a check-box option on the Keyboard page.
    let layout = matches!(m, LayoutEnglishUs | LayoutSpanishSpain);
    if layout {
        format!("> [x] {row}")
    } else if option {
        format!("> {row}")
    } else {
        row
    }
}

#[test]
fn every_row_fits_the_card_in_both_languages() {
    use neo_atrium::Layout;
    use neo_atrium::scene::CARD_TEXT_PX;
    use neo_text::layout::measure;
    let font = neo_text::Font::from_bytes(neo_text::fonts::INTER_REGULAR).expect("Inter parses");
    let mut over = Vec::new();
    let mut widest = (0.0f32, String::new());
    for (w, h) in [(1920, 1080), (1280, 800)] {
        let room = Layout::new(w, h, 640, 400).expect("layout").card_text(1).w as f32;
        for lang in Lang::ALL {
            for &m in Msg::ALL {
                let row = widest_row(lang, m);
                let px = measure(&font, CARD_TEXT_PX, &row);
                if px / room > widest.0 {
                    widest = (
                        px / room,
                        format!("{w}x{h} {lang:?} {m:?}: {px:.1} of {room} px"),
                    );
                }
                if px > room {
                    over.push(format!(
                        "{w}x{h} {lang:?} {m:?}: {px:.1} > {room} px: {row:?}"
                    ));
                }
            }
        }
    }
    std::eprintln!("widest row: {} ({:.1}%)", widest.1, widest.0 * 100.0);
    assert!(
        over.is_empty(),
        "rows wider than the card:\n{}",
        over.join("\n")
    );
}

#[test]
#[ignore = "a report for TRANSLATION-NOTES.md, not a check"]
fn report_widths() {
    use neo_atrium::Layout;
    use neo_atrium::scene::CARD_TEXT_PX;
    use neo_text::layout::measure;
    let font = neo_text::Font::from_bytes(neo_text::fonts::INTER_REGULAR).unwrap();
    let room = Layout::new(1280, 800, 640, 400).unwrap().card_text(1).w as f32;
    for &m in Msg::ALL {
        let en = measure(&font, CARD_TEXT_PX, &widest_row(Lang::En, m));
        let es = measure(&font, CARD_TEXT_PX, &widest_row(Lang::Es, m));
        std::println!(
            "WIDTH {m:?} {:.0} {:.0} {:.1}",
            en / room * 100.0,
            es / room * 100.0,
            room
        );
    }
}
