// SPDX-License-Identifier: MPL-2.0
//! F1a: the Files catalog's KATs, the same rules as the main catalog's
//! (`tests.rs`), built only with the `files` feature (`files-core` turns it
//! on; CI also runs `cargo test -p i18n-core --features files`):
//!
//! 1. every id has an entry in both tables, with the same argument count,
//!    and no lookup falls back;
//! 2. the English table equals the pinned snapshot (`fixtures/en-files.txt`);
//! 3. every character is in Inter Regular, and the Spanish table uses no
//!    character beyond the main catalog's (`¿áéíñóú`);
//! 4. every row, filled with its widest numbers, fits the card's text box at
//!    both reference screens, in both languages. Files' window is at least
//!    the card's width, so the card bounds these rows too.

extern crate std;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use std::format;

use super::files::{MSG_COUNT, Msg, lookup, text};
use super::{Lang, arg_count, fill};

#[test]
fn every_files_id_has_an_entry_in_both_tables_and_none_falls_back() {
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
}

/// The Files rows' English: one `Id<TAB>text` line per id, in table order.
/// Any English wording change edits this file on purpose.
const EN_FILES_PINNED: &str = include_str!("../fixtures/en-files.txt");

fn en_files_table() -> String {
    let mut out = String::new();
    for &m in Msg::ALL {
        out.push_str(&format!("{m:?}\t{}\n", text(Lang::En, m)));
    }
    out
}

#[test]
fn the_files_english_is_the_pinned_table() {
    assert_eq!(
        en_files_table(),
        EN_FILES_PINNED,
        "the Files English table differs from fixtures/en-files.txt"
    );
}

#[test]
#[ignore = "writes fixtures/en-files.txt's contents to stdout, for a deliberate wording change"]
fn print_en_files_table() {
    std::print!("{}", en_files_table());
}

#[test]
fn every_files_character_is_in_the_face() {
    let font = neo_text::Font::from_bytes(neo_text::fonts::INTER_REGULAR).expect("Inter parses");
    let mut missing = Vec::new();
    for lang in Lang::ALL {
        for &m in Msg::ALL {
            for c in text(lang, m).chars() {
                if font.glyph_id(c) == 0 {
                    missing.push(format!("U+{:04X} {c:?} in {lang:?} {m:?}", c as u32));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "glyphs missing from Inter Regular:\n{}",
        missing.join("\n")
    );
    // No character beyond the main catalog's (TRANSLATION-NOTES.md).
    for &m in Msg::ALL {
        for c in text(Lang::Es, m).chars() {
            assert!(c.is_ascii() || "¿áéíñóú".contains(c), "{m:?}: {c:?}");
        }
    }
}

/// `m`'s row filled with its widest numbers (a size column, the limits, an
/// errno).
fn widest_row(lang: Lang, m: Msg) -> String {
    use Msg::*;
    let args: Vec<String> = match m {
        FilesSizeBytes => vec!["1023".into()],
        FilesSizeKb | FilesSizeMb => vec![format!("9{}9", text(lang, FilesDecimalMark))],
        FilesItemsMany | FilesFolderFullNote | FilesErrFolderFull => vec!["64".into()],
        FilesNameTooLong => vec!["47".into()],
        FilesCopyTooBig => vec!["32".into()],
        FilesErrOther => vec!["-116".into()],
        _ => Vec::new(),
    };
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let row = fill(text(lang, m), &refs);
    assert_eq!(arg_count(&row), 0, "{m:?}: an argument is left unfilled");
    row
}

#[test]
fn every_files_row_fits_the_card_in_both_languages() {
    use neo_atrium::Layout;
    use neo_atrium::scene::CARD_TEXT_PX;
    use neo_text::layout::measure;
    let font = neo_text::Font::from_bytes(neo_text::fonts::INTER_REGULAR).expect("Inter parses");
    let mut over = Vec::new();
    for (w, h) in [(1920, 1080), (1280, 800)] {
        let room = Layout::new(w, h, 640, 400).expect("layout").card_text(1).w as f32;
        for lang in Lang::ALL {
            for &m in Msg::ALL {
                let row = widest_row(lang, m);
                let px = measure(&font, CARD_TEXT_PX, &row);
                if px > room {
                    over.push(format!(
                        "{w}x{h} {lang:?} {m:?}: {px:.1} > {room} px: {row:?}"
                    ));
                }
            }
        }
    }
    assert!(
        over.is_empty(),
        "rows wider than the card:\n{}",
        over.join("\n")
    );
}
