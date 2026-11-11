// SPDX-License-Identifier: MPL-2.0
//! S6a's host KATs (`Plans/DISPATCH-2026-11-09-seat-c-setup-s6a-i18n-catalog.md`):
//!
//! 1. every id has an entry in both tables, with the same `{}` count, and no
//!    lookup falls back;
//! 2. the English table is today's literals, byte for byte, read from the
//!    source files that draw them (until S6b swaps the literals for ids);
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

// ---- 2. English is today's literals ----

const SETUP_CORE: &str = include_str!("../../setup-core/src/lib.rs");
const KEYBOARD: &str = include_str!("../../setup-core/src/keyboard.rs");
const LAYOUTS: &str = include_str!("../../../kernel/src/drivers/keymap_layout_logic.rs");
const SETUP_VIEW: &str = include_str!("../../neo-init/src/desktop_session/setup.rs");
const DESKTOP: &str = include_str!("../../neo-init/src/desktop_session.rs");
const INTRO: &str = include_str!("../../neo-atrium/src/intro.rs");
const CLOCK: &str = include_str!("../../neo-atrium/src/clock.rs");
const POWER: &str = include_str!("../../neo-atrium/src/power.rs");
const CIVIL: &str = include_str!("../../libneo/src/civil.rs");

/// Where a literal sits in its file.
#[derive(Clone, Copy)]
enum At {
    /// A whole string literal: `"…"`.
    Whole,
    /// The end of a longer format string: `…"`.
    Tail,
}

/// Each id, the file that shows it today and the literal as written there
/// (named format arguments and `\"` escapes included). An id shown from two
/// places is listed twice.
const SOURCES: &[(Msg, &str, &str, At)] = {
    use At::*;
    use Msg::*;
    &[
        (
            SetupHint,
            SETUP_CORE,
            "Up/Down to move, Enter to choose, Esc to go back.",
            Whole,
        ),
        (
            SetupSearchHint,
            SETUP_CORE,
            "Type to search. Up/Down, Enter to choose.",
            Whole,
        ),
        (SetupBegin, SETUP_CORE, "Begin", Whole),
        (SetupSkip, SETUP_CORE, "Skip setup", Whole),
        (
            SetupSkipDefaults,
            SETUP_CORE,
            "Skip setup and use the defaults",
            Whole,
        ),
        (SetupBack, SETUP_CORE, "Back", Whole),
        (SetupContinue, SETUP_CORE, "Continue", Whole),
        (SetupTryOption, SETUP_CORE, "Try LogansOS", Whole),
        (
            SetupReduceMotionOption,
            SETUP_CORE,
            "{} Reduce motion",
            Whole,
        ),
        (
            SetupReduceTransparencyOption,
            SETUP_CORE,
            "{} Reduce transparency",
            Whole,
        ),
        (SetupTitleWelcome, SETUP_CORE, "Set up LogansOS", Whole),
        (SetupTitleSkip, SETUP_CORE, "Skip setup?", Whole),
        (SetupTitleLanguage, SETUP_CORE, "Language", Whole),
        (SetupTitleKeyboard, SETUP_CORE, "Keyboard", Whole),
        (SetupTitleTimeZone, SETUP_CORE, "Time zone", Whole),
        (SetupTitleComfort, SETUP_CORE, "Comfort", Whole),
        (SetupTitleTry, SETUP_CORE, "Try LogansOS", Whole),
        (SetupTitlePage, SETUP_CORE, "{name} ({n} of {total})", Whole),
        (
            SetupBodyWelcome,
            SETUP_CORE,
            "A few short screens. You can change any of these later.",
            Whole,
        ),
        (
            SetupBodySkip,
            SETUP_CORE,
            "The defaults are UTC, with full motion and transparency.",
            Whole,
        ),
        (
            SetupBodyLanguage,
            SETUP_CORE,
            "English is the only language for now.",
            Whole,
        ),
        (
            SetupBodyKeyboard,
            KEYBOARD,
            "US is the only keyboard layout for now.",
            Whole,
        ),
        (
            SetupKeyboardBodyChoose,
            KEYBOARD,
            "Choose the layout printed on your keys.",
            Whole,
        ),
        (SetupKeyboardTryIt, KEYBOARD, "Try it: {}_", Whole),
        (SetupSummaryKeyboard, KEYBOARD, "Keyboard: {}", Whole),
        (LayoutEnglishUs, LAYOUTS, "English (US)", Whole),
        (LayoutSpanishSpain, LAYOUTS, "Spanish (Spain)", Whole),
        (
            SetupBodyTimeZoneNoSearch,
            SETUP_CORE,
            "Where are you? At Home: timezone Area/City.",
            Whole,
        ),
        (
            SetupBodyZoneSet,
            SETUP_CORE,
            "Time zone: {name}. The clock is set.",
            Whole,
        ),
        (
            SetupBodyZoneLater,
            SETUP_CORE,
            "Change it later at Home: timezone Area/City.",
            Whole,
        ),
        (
            SetupBodySearchEmpty,
            SETUP_CORE,
            "Type a city to search, or pick one.",
            Whole,
        ),
        (
            SetupBodySearchNone,
            SETUP_CORE,
            "Search: {}  (no zone matches)",
            Whole,
        ),
        (SetupBodySearch, SETUP_CORE, "Search: {}", Whole),
        (
            SetupBodyComfort,
            SETUP_CORE,
            "Make the screen calmer, if you like.",
            Whole,
        ),
        (SetupNotSet, SETUP_CORE, "not set", Whole),
        (ZoneRow, SETUP_CORE, "Time zone: {}", Whole),
        (ZoneRow, DESKTOP, "Time zone: {}", Whole),
        (
            SetupSummaryZoneClockNotSet,
            SETUP_CORE,
            "Time zone: {name} (clock not set)",
            Whole,
        ),
        (ZoneRowNotSet, SETUP_CORE, "Time zone: not set", Whole),
        (ZoneRowNotSet, DESKTOP, "Time zone: not set", Whole),
        (
            SetupSummaryComfort,
            SETUP_CORE,
            "Reduce motion: {}. Reduce transparency: {}.",
            Whole,
        ),
        (On, SETUP_CORE, "on", Whole),
        (Off, SETUP_CORE, "off", Whole),
        (
            SetupNoticeZoneInUse,
            SETUP_VIEW,
            "Couldn't save the time zone. In use: {name}.",
            Whole,
        ),
        (
            SetupNoticeZoneRestart,
            SETUP_VIEW,
            "Couldn't save the time zone. Restart to check it.",
            Whole,
        ),
        (
            IntroGreeting,
            INTRO,
            "Good day. I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.",
            Whole,
        ),
        (
            IntroName,
            INTRO,
            "I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.",
            Whole,
        ),
        (
            IntroHousehold,
            INTRO,
            "The household side comes later; for now, I look after this computer for you.",
            Whole,
        ),
        (
            IntroPrivacy,
            INTRO,
            "I work entirely on this machine, and I don't send what you tell me anywhere.",
            Whole,
        ),
        (
            IntroUsbOnly,
            INTRO,
            "On this machine I can't keep anything once it's switched off, not yet.",
            Whole,
        ),
        (
            IntroAsk,
            INTRO,
            "Ask in your own words: \\\"What does a CPU do?\\\", for instance.",
            Whole,
        ),
        (
            IntroList,
            INTRO,
            "\\\"list /\\\" shows you the files on this machine.",
            Whole,
        ),
        (
            IntroListStill,
            INTRO,
            "\\\"list /\\\" still shows you the files on this machine.",
            Whole,
        ),
        (
            IntroExit,
            INTRO,
            "Type help for what you can do yourself. Esc always brings you back here.",
            Whole,
        ),
        (
            HelpTitle,
            INTRO,
            "What you can do yourself, without asking me:",
            Whole,
        ),
        (
            HelpEsc,
            INTRO,
            "Esc sets me aside and brings you back to Home.",
            Whole,
        ),
        (
            HelpList,
            INTRO,
            "\\\"list /\\\" opens the Lens: a read-only view of the files on this machine.",
            Whole,
        ),
        (
            HelpChips,
            INTRO,
            "The chips on the left switch between me and the Lens; Tab reaches them.",
            Whole,
        ),
        (
            HelpDrag,
            INTRO,
            "Drag a window by its title bar to move it, or by an edge to resize it.",
            Whole,
        ),
        (
            HelpType,
            INTRO,
            "Start typing to ask me something, or press Esc to close this card.",
            Whole,
        ),
        (
            IntroHint,
            INTRO,
            "Type help, or click here, for what you can do yourself.",
            Whole,
        ),
        (
            IntroModelAbsent,
            INTRO,
            "I can hear you, but my model isn't on this system yet. Boot the LogansOS USB it came with.",
            Whole,
        ),
        (
            IntroModelDamaged,
            INTRO,
            "My model file is damaged, so I won't use it. Re-create the USB from the original image.",
            Whole,
        ),
        (
            IntroModelLoadFailed,
            INTRO,
            "My model didn't load, so I'm running without it. The details are in /model.",
            Whole,
        ),
        (
            IntroTooLittleMemory,
            INTRO,
            "This computer has {have_gb} GB of memory; my smallest brain needs {need_gb} GB.",
            Whole,
        ),
        (
            IntroCantThink,
            INTRO,
            "I can't think yet, but \\\"list /\\\" still shows you the files on this machine.",
            Whole,
        ),
        (ButtonBack, INTRO, "[ Back ]", Tail),
        (GoodMorning, CLOCK, "Good morning", Whole),
        (GoodAfternoon, CLOCK, "Good afternoon", Whole),
        (GoodEvening, CLOCK, "Good evening", Whole),
        (GreetingRow, CLOCK, "{}. It's {} on {}.", Whole),
        (Sunday, CIVIL, "Sunday", Whole),
        (Monday, CIVIL, "Monday", Whole),
        (Tuesday, CIVIL, "Tuesday", Whole),
        (Wednesday, CIVIL, "Wednesday", Whole),
        (Thursday, CIVIL, "Thursday", Whole),
        (Friday, CIVIL, "Friday", Whole),
        (Saturday, CIVIL, "Saturday", Whole),
        (January, CIVIL, "January", Whole),
        (February, CIVIL, "February", Whole),
        (March, CIVIL, "March", Whole),
        (April, CIVIL, "April", Whole),
        (May, CIVIL, "May", Whole),
        (June, CIVIL, "June", Whole),
        (July, CIVIL, "July", Whole),
        (August, CIVIL, "August", Whole),
        (September, CIVIL, "September", Whole),
        (October, CIVIL, "October", Whole),
        (November, CIVIL, "November", Whole),
        (December, CIVIL, "December", Whole),
        (Am, CIVIL, " AM", Whole),
        (Pm, CIVIL, " PM", Whole),
        (PowerAskShutDown, POWER, "Shut down this computer?", Whole),
        (PowerShutDown, POWER, "Shut down", Whole),
        (PowerAskRestart, POWER, "Restart this computer?", Whole),
        (PowerRestart, POWER, "Restart", Whole),
        (
            PowerWarning,
            POWER,
            "Open apps close first; anything they haven't saved is lost.",
            Whole,
        ),
        (
            PowerConfirmHint,
            POWER,
            "Enter to go ahead; any other key goes back.",
            Whole,
        ),
        (PowerShuttingDown, POWER, "Shutting down...", Whole),
        (PowerRestarting, POWER, "Restarting...", Whole),
        (PowerBusy, POWER, "It's already shutting down.", Whole),
        (
            PowerUnsupported,
            POWER,
            "This system can't do that from here.",
            Whole,
        ),
        (PowerFailed, POWER, "That didn't work.", Whole),
        (PowerError, POWER, "(error {errno}) Press any key.", Whole),
        (ZoneRowNow, DESKTOP, "Time zone: {} ({})", Whole),
        (
            ClockKeepsLocal,
            DESKTOP,
            "This computer's clock keeps local time (as Windows does).",
            Whole,
        ),
        (
            ClockKeepsUtc,
            DESKTOP,
            "This computer's clock keeps UTC.",
            Whole,
        ),
        (
            ClockLegacy1,
            DESKTOP,
            "Saved. This system shows the computer's clock",
            Whole,
        ),
        (
            ClockLegacy2,
            DESKTOP,
            "as it reads, without a time zone.",
            Whole,
        ),
        (
            ClockSetZone,
            DESKTOP,
            "Set your zone: timezone America/New_York",
            Whole,
        ),
        (
            ClockChangeZone,
            DESKTOP,
            "Change it: timezone Area/City",
            Whole,
        ),
        (
            ClockRestart,
            DESKTOP,
            "Restart to read this computer's clock this way.",
            Whole,
        ),
        (
            ClockOffUtc,
            DESKTOP,
            "Off by whole hours? Type: rtc utc",
            Whole,
        ),
        (
            ClockOffLocal,
            DESKTOP,
            "Off by whole hours? Type: rtc local",
            Whole,
        ),
        (ClockTitle, DESKTOP, "The clock", Whole),
        (ClockZoneSet, DESKTOP, "Time zone set to {}.", Whole),
        (
            ClockNoZone,
            DESKTOP,
            "There's no time zone called \\\"{typed}\\\".",
            Whole,
        ),
        (
            ClockTypeZone,
            DESKTOP,
            "Type timezone Area/City, for example",
            Whole,
        ),
        (
            ClockTypeZoneExamples,
            DESKTOP,
            "timezone America/New_York or timezone Europe/London.",
            Whole,
        ),
        (ClockDidYouMean, DESKTOP, "Did you mean:", Whole),
        (
            ClockSavedLocal,
            DESKTOP,
            "Saved: this computer's clock keeps local time.",
            Whole,
        ),
        (
            ClockSavedUtc,
            DESKTOP,
            "Saved: this computer's clock keeps UTC.",
            Whole,
        ),
    ]
};

/// The literal as the program prints it: `\"` unescaped, and each named
/// format argument (`{name}`) written as the catalog's positional `{}`.
fn as_printed(source: &str) -> String {
    let unescaped = source.replace("\\\"", "\"");
    let mut out = String::new();
    let mut rest = unescaped.as_str();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close)
                if after[..close]
                    .bytes()
                    .all(|b| b == b'_' || b.is_ascii_lowercase() || b.is_ascii_digit()) =>
            {
                out.push_str("{}");
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[test]
fn english_is_todays_literals_byte_for_byte() {
    let mut missing = Vec::new();
    for &(m, file, literal, at) in SOURCES {
        let needle = match at {
            At::Whole => format!("\"{literal}\""),
            At::Tail => format!("{literal}\""),
        };
        if !file.contains(needle.as_str()) {
            missing.push(format!("{m:?}: {needle}"));
        }
        assert_eq!(
            text(Lang::En, m),
            as_printed(literal),
            "{m:?}: the English entry isn't the literal the code prints"
        );
    }
    assert!(
        missing.is_empty(),
        "literals no longer in the source:\n{}",
        missing.join("\n")
    );
    for &m in Msg::ALL {
        assert!(
            SOURCES.iter().any(|s| s.0 == m),
            "{m:?} has no source literal to hold it to"
        );
    }
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
    let date = format!(
        "{}, {} 30, 2026",
        longest(&[
            Sunday, Monday, Tuesday, Wednesday, Thursday, Friday, Saturday,
        ]),
        longest(&[
            January, February, March, April, May, June, July, August, September, October, November,
            December,
        ])
    );
    let on_off = longest(&[On, Off]);
    let args: Vec<String> = match m {
        SetupReduceMotionOption | SetupReduceTransparencyOption => vec!["[x]".into()],
        SetupTitlePage => vec![
            longest(&[
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
        ZoneRowNow => vec![ZONE.into(), OFFSET.into()],
        // The test field at `setup_core::keyboard::FIELD_MAX` (24) of the
        // widest Latin capital.
        SetupKeyboardTryIt => vec!["W".repeat(24)],
        SetupSummaryKeyboard => vec![longest(&[LayoutEnglishUs, LayoutSpanishSpain]).into()],
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
