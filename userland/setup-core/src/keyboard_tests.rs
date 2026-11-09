// SPDX-License-Identifier: MPL-2.0
//! F11 S5 (`keyboard-layouts`): the Keyboard page. Every row is written out
//! by hand from the wording settled here, not taken from the code.

use super::*;

fn rows(w: &Wizard) -> Vec<String> {
    w.rows()
}

/// Begin, then the Keyboard page is the first one, with US chosen.
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
            "  [ ] English (UK)",
            "  Continue",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    assert_eq!(w.choices().layout, 0);
}

/// The gate's walk on the page: choose UK, type the four characters the WM
/// decodes in UK, erase one, Continue; the summary names UK.
#[test]
fn choose_uk_type_in_the_field_and_continue() {
    let mut w = on_keyboard();
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(w.choices().layout, 1);
    for c in *b"\"@#\\" {
        assert_eq!(w.on(Input::Char(c)), Outcome::Redraw);
    }
    assert_eq!(w.field(), "\"@#\\");
    assert_eq!(
        rows(&w),
        [
            "Keyboard (1 of 4)",
            "Choose the layout printed on your keys.",
            "Try it: \"@#\\_",
            "  [ ] English (US)",
            "> [x] English (UK)",
            "  Continue",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    assert_eq!(w.on(Input::Erase), Outcome::Redraw);
    assert_eq!(w.field(), "\"@#");
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
            "Keyboard: English (UK)",
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

/// Characters do nothing off the Keyboard page.
#[test]
fn characters_off_the_page_change_nothing() {
    let mut w = Wizard::new(false);
    let before = w.clone();
    assert_eq!(w.on(Input::Char(b'q')), Outcome::Redraw);
    assert_eq!(w.on(Input::Erase), Outcome::Redraw);
    assert_eq!(w, before);
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
    assert_eq!(LAYOUT_NAMES, ["English (US)", "English (UK)"]);
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
    for c in *b"\"@#\\" {
        w.on(Input::Char(c));
    }
    assert_eq!(rows(&w), oracle::KEYBOARD_UK_ROWS);
}
