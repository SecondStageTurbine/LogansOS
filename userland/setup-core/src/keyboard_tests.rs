// SPDX-License-Identifier: MPL-2.0
//! F11 S5 (`keyboard-layouts`): the Keyboard page. Every row is written out
//! by hand from the wording settled here, not taken from the code.

use super::*;

fn rows(w: &Wizard) -> Vec<String> {
    w.rows()
}

/// Begin, then the Keyboard page is the first one, with US chosen.
/// The gate's keys as the WM decodes them in Spanish: ñ, Shift+¡ (¿), ¡,
/// ç, the acute dead key, a.
fn gate_keys() -> [keyboard::Typed; 6] {
    use keyboard::{Dead, Typed};
    [
        Typed::Char('ñ'),
        Typed::Char('¿'),
        Typed::Char('¡'),
        Typed::Char('ç'),
        Typed::Dead(Dead::Acute),
        Typed::Char('a'),
    ]
}

fn on_keyboard() -> Wizard {
    let mut w = Wizard::new(false);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Keyboard));
    w
}

#[test]
fn the_keyboard_page_is_shown_without_the_stubs() {
    assert_eq!(
        Wizard::new(false).pages(),
        [
            Page::Welcome,
            Page::Keyboard,
            Page::TimeZone,
            Page::Comfort,
            Page::Try
        ]
    );
    // With the stubs, Language comes first, as in S1.
    assert_eq!(
        Wizard::new(true).pages(),
        [
            Page::Welcome,
            Page::Language,
            Page::Keyboard,
            Page::TimeZone,
            Page::Comfort,
            Page::Try
        ]
    );
}

#[test]
fn the_page_lists_the_layouts_with_us_chosen() {
    let w = on_keyboard();
    assert_eq!(
        rows(&w),
        [
            "Keyboard (1 of 4)",
            "Choose the layout printed on your keys.",
            "Try it: _",
            "> [x] English (US)",
            "  [ ] Spanish (Spain)",
            "  Continue",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    assert_eq!(w.choices().layout, 0);
}

/// The gate's walk on the page: choose Spanish, type `ñ` `¿` `¡` `ç` and the
/// acute dead key then `a` (as the WM decodes them in Spanish), erase one,
/// Continue; the summary names Spanish.
#[test]
fn choose_spanish_type_in_the_field_and_continue() {
    let mut w = on_keyboard();
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(w.choices().layout, 1);
    for t in gate_keys() {
        w.on(Input::Layout(t));
    }
    assert_eq!(w.field(), "ñ¿¡çá");
    assert_eq!(
        rows(&w),
        [
            "Keyboard (1 of 4)",
            "Choose the layout printed on your keys.",
            "Try it: ñ¿¡çá_",
            "  [ ] English (US)",
            "> [x] Spanish (Spain)",
            "  Continue",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    assert_eq!(w.on(Input::Erase), Outcome::Redraw);
    assert_eq!(w.field(), "ñ¿¡ç");
    w.on(Input::Down);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
    // Onward to Try: UTC's Continue, Comfort's Continue.
    w.on(Input::Up);
    w.on(Input::Choose);
    w.on(Input::Up);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Try));
    assert_eq!(
        rows(&w),
        [
            "Try LogansOS (4 of 4)",
            "Time zone: UTC",
            "Keyboard: Spanish (Spain)",
            "Reduce motion: off. Reduce transparency: off.",
            "> Try LogansOS",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Done(Finish::Try));
    assert_eq!(w.choices().layout, 1);
}

/// Choosing a layout clears the field (its text was typed in the other
/// one); the field is bounded and printable only; Back leaves the choice.
#[test]
fn the_field_clears_on_a_new_layout_and_is_bounded() {
    let mut w = on_keyboard();
    w.on(Input::Char(b'a'));
    w.on(Input::Down);
    w.on(Input::Choose);
    assert_eq!(w.field(), "");
    for _ in 0..FIELD_MAX + 5 {
        w.on(Input::Char(b'x'));
    }
    assert_eq!(w.field().len(), FIELD_MAX);
    w.on(Input::Char(0x07));
    w.on(Input::Char(0x80));
    assert_eq!(w.field().len(), FIELD_MAX);
    for _ in 0..FIELD_MAX + 1 {
        w.on(Input::Erase);
    }
    assert_eq!(w.field(), "");
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Welcome));
    assert_eq!(w.choices().layout, 1);
}

/// Characters do nothing off the Keyboard page (S3's rule: Space chooses,
/// anything else is ignored), and a key that changes the field changes
/// nothing else.
#[test]
fn characters_off_the_page_change_nothing() {
    let mut w = Wizard::new(false);
    assert_eq!(w.on(Input::Char(b'q')), Outcome::Nothing);
    assert_eq!(w.on(Input::Erase), Outcome::Nothing);
    assert_eq!(
        (w.page(), w.focus(), w.choices()),
        (Page::Welcome, 0, Choices::default())
    );
    let mut w = on_keyboard();
    assert_eq!(w.on(Input::Erase), Outcome::Nothing);
    assert_eq!(w.on(Input::Char(b'z')), Outcome::Redraw);
    assert_eq!(
        (w.page(), w.focus(), w.choices().layout),
        (Page::Keyboard, 0, 0)
    );
    // Space chooses (here: US, already chosen).
    assert_eq!(w.on(Input::Char(b' ')), Outcome::Redraw);
    assert_eq!(w.field(), "");
}

/// The longest page still fits the card, with a full field.
#[test]
fn the_page_fits_the_card() {
    let mut w = on_keyboard();
    for _ in 0..FIELD_MAX {
        w.on(Input::Char(b'W'));
    }
    assert!(w.rows().len() <= MAX_ROWS);
    assert_eq!(LAYOUT_COUNT, 2);
    assert_eq!(LAYOUT_NAMES, ["English (US)", "Spanish (Spain)"]);
}

/// The gate compares the two: `scripts/desktop/setup_oracle.py`'s Keyboard
/// page, typed again there, is these rows after the gate's keys.
#[test]
fn rows_are_the_oracles_wording() {
    #[allow(dead_code)]
    mod oracle {
        include!("../../../fixtures/desktop-setup/setup.rs");
    }
    let mut w = on_keyboard();
    w.on(Input::Down);
    w.on(Input::Choose);
    for t in gate_keys() {
        w.on(Input::Layout(t));
    }
    assert_eq!(rows(&w), oracle::KEYBOARD_ES_ROWS);
}

/// S3 and S5 together: the Keyboard page (Spanish, a key typed into the field)
/// comes before S3's searchable Time zone page, whose search still takes
/// every character (Space included), and the Try summary names both.
#[test]
fn the_keyboard_page_then_s3s_search() {
    struct Two;
    impl Zones for Two {
        fn shortlist(&self) -> Vec<u16> {
            alloc::vec![0, 300]
        }
        fn search(&self, typed: &str) -> Vec<u16> {
            if "europe/london".contains(&typed.to_ascii_lowercase()) {
                alloc::vec![300]
            } else {
                Vec::new()
            }
        }
        fn name(&self, id: u16) -> &'static str {
            if id == 300 { "Europe/London" } else { "UTC" }
        }
        fn detail(&self, _id: u16) -> String {
            String::from("fixed")
        }
    }
    let mut w = Wizard::new(false).with_zones(Box::new(Two), None);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Keyboard));
    w.on(Input::Down);
    w.on(Input::Choose);
    assert_eq!(
        w.on(Input::Layout(keyboard::Typed::Char('ñ'))),
        Outcome::Redraw
    );
    assert_eq!(w.field(), "ñ");
    w.on(Input::Down);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
    for c in *b"lon" {
        assert_eq!(w.on(Input::Char(c)), Outcome::Redraw);
    }
    assert_eq!(w.query(), "lon");
    assert_eq!(w.field(), "ñ");
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    w.on(Input::Down);
    assert_eq!(w.on(Input::Choose), Outcome::Confirm(300));
    assert_eq!(w.confirmed(true), Outcome::Page(Page::Comfort));
    w.on(Input::Up);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Try));
    let summary = w.summary();
    assert_eq!(summary[0], "Time zone: Europe/London");
    assert_eq!(summary[1], "Keyboard: Spanish (Spain)");
    assert_eq!(w.choices().layout, 1);
    assert_eq!(w.choices().tz, Some(300));
}

/// Dead keys in the field: a waiting dead key changes nothing (no redraw);
/// with a key it doesn't compose with, both; Backspace drops a waiting dead
/// key before it erases; a non-ASCII field counts characters, not bytes.
#[test]
fn dead_keys_in_the_field() {
    use keyboard::{Dead, Typed};
    let mut w = on_keyboard();
    w.on(Input::Down);
    w.on(Input::Choose);
    assert_eq!(
        w.on(Input::Layout(Typed::Dead(Dead::Diaeresis))),
        Outcome::Nothing
    );
    assert_eq!(w.on(Input::Layout(Typed::Char('u'))), Outcome::Redraw);
    assert_eq!(w.field(), "ü");
    w.on(Input::Layout(Typed::Dead(Dead::Acute)));
    assert_eq!(w.on(Input::Layout(Typed::Char('n'))), Outcome::Redraw);
    assert_eq!(w.field(), "ü´n");
    w.on(Input::Layout(Typed::Dead(Dead::Grave)));
    assert_eq!(w.on(Input::Erase), Outcome::Nothing);
    assert_eq!(w.on(Input::Layout(Typed::Char('a'))), Outcome::Redraw);
    assert_eq!(w.field(), "ü´na");
    assert_eq!(w.on(Input::Erase), Outcome::Redraw);
    assert_eq!(w.field(), "ü´n");
    assert_eq!(w.on(Input::Layout(Typed::None)), Outcome::Nothing);
    for _ in 0..FIELD_MAX {
        w.on(Input::Layout(Typed::Char('ñ')));
    }
    assert_eq!(w.field().chars().count(), FIELD_MAX);
    // Off the Keyboard page a layout key does nothing (Spanish is focused;
    // Down is Continue).
    w.on(Input::Down);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
    assert_eq!(w.on(Input::Layout(Typed::Char('ñ'))), Outcome::Nothing);
}
