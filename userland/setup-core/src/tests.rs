// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the page machine. Every row is written out by hand from
//! the THINK's wording (§2.1, §4.2), not taken from the code.

use super::*;

/// The rows, for comparing with string literals (a test leaks them).
fn rows(w: &Wizard) -> Vec<&'static str> {
    w.rows().into_iter().map(|s| &*s.leak()).collect()
}

#[test]
fn welcome_is_first_with_begin_focused() {
    let w = Wizard::new(false);
    assert_eq!(w.page(), Page::Welcome);
    assert_eq!(
        rows(&w),
        [
            "Set up LogansOS",
            "A few short screens. You can change any of these later.",
            "> Begin",
            "  Skip setup",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
}

#[test]
fn the_pages_in_order_and_no_network_page() {
    assert_eq!(
        Wizard::new(false).pages(),
        [Page::Welcome, Page::TimeZone, Page::Comfort, Page::Try]
    );
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

/// S1's gate walk (§5 S1): Begin, Time zone (UTC, Continue), Comfort
/// (toggle both), Try.
#[test]
fn the_gate_walk() {
    let mut w = Wizard::new(false);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
    assert_eq!(
        rows(&w),
        [
            "Time zone (1 of 3)",
            "Where are you? At Home: timezone Area/City.",
            "> [x] UTC",
            "  [ ] America/New_York",
            "  [ ] Europe/London",
            "  [ ] Australia/Sydney",
            "  Continue",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    // UTC is already chosen; Enter on it keeps it.
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(w.choices().zone, 0);
    // Up from the first option wraps to Continue.
    assert_eq!(w.on(Input::Up), Outcome::Redraw);
    assert_eq!(w.focus(), 4);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Comfort));
    assert_eq!(
        rows(&w),
        [
            "Comfort (2 of 3)",
            "Make the screen calmer, if you like.",
            "> [ ] Reduce motion",
            "  [ ] Reduce transparency",
            "  Continue",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(
        rows(&w)[2..5],
        [
            "  [x] Reduce motion",
            "> [x] Reduce transparency",
            "  Continue"
        ]
    );
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Try));
    assert_eq!(
        rows(&w),
        [
            "Try LogansOS (3 of 3)",
            "Time zone: UTC",
            "Reduce motion: on. Reduce transparency: on.",
            "> Try LogansOS",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Done(Finish::Try));
    assert_eq!(
        w.choices(),
        Choices {
            zone: 0,
            reduce_motion: true,
            reduce_transparency: true
        }
    );
}

/// The control: Esc on Welcome asks, and Skip ends with the defaults.
#[test]
fn esc_on_welcome_asks_then_skips() {
    let mut w = Wizard::new(false);
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::SkipConfirm));
    assert_eq!(
        rows(&w),
        [
            "Skip setup?",
            "The defaults are UTC, with full motion and transparency.",
            "> Skip setup and use the defaults",
            "  Back",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    // Back (the option, or Esc) returns to Welcome, Begin focused.
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Welcome));
    assert_eq!(w.focus(), 0);
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::SkipConfirm));
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Welcome));
    // "Skip setup" on Welcome asks too; then skip.
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::SkipConfirm));
    assert_eq!(w.on(Input::Choose), Outcome::Done(Finish::Skipped));
    assert_eq!(w.choices(), Choices::default());
}

/// Esc goes back one page; the stub pages sit between Welcome and Time
/// zone and count in `N of M`.
#[test]
fn back_and_the_stub_pages() {
    let mut w = Wizard::new(true);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Language));
    assert_eq!(
        rows(&w),
        [
            "Language (1 of 5)",
            "English is the only language for now.",
            "> Continue",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Keyboard));
    assert_eq!(rows(&w)[0], "Keyboard (2 of 5)");
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
    assert_eq!(rows(&w)[0], "Time zone (3 of 5)");
    // A zone choice survives going back and forth.
    w.on(Input::Down);
    w.on(Input::Down);
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(w.choices().zone, 2);
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Keyboard));
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Language));
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Welcome));
    assert_eq!(w.choices().zone, 2);
    let mut w2 = Wizard::new(false);
    w2.on(Input::Choose);
    assert_eq!(w2.on(Input::Back), Outcome::Page(Page::Welcome));
}

/// The eight-row budget (§4.5), on every page of both builds, with the
/// focus on every option.
#[test]
fn every_page_fits_the_card() {
    for stub in [false, true] {
        let mut w = Wizard::new(stub);
        let mut seen = Vec::new();
        for _ in 0..64 {
            let n = w.options().len();
            for _ in 0..n {
                assert!(w.rows().len() <= MAX_ROWS, "{:?}: {:?}", w.page(), w.rows());
                w.on(Input::Down);
            }
            seen.push(w.page());
            // Onward: Begin on Welcome, else the last option (Continue,
            // or the page's only one).
            let onward = if w.page() == Page::Welcome {
                0
            } else {
                w.options().len() - 1
            };
            while w.focus() != onward {
                w.on(Input::Down);
            }
            match w.on(Input::Choose) {
                Outcome::Done(_) => break,
                Outcome::Redraw => panic!("{:?}: onward only redrew", w.page()),
                Outcome::Page(_) => {}
            }
        }
        assert!(seen.contains(&Page::Try), "{seen:?}");
    }
}

#[test]
fn page_names_for_the_log() {
    let names: Vec<&str> = [
        Page::Welcome,
        Page::SkipConfirm,
        Page::Language,
        Page::Keyboard,
        Page::TimeZone,
        Page::Comfort,
        Page::Try,
    ]
    .iter()
    .map(|p| p.name())
    .collect();
    assert_eq!(
        names,
        [
            "welcome", "skip", "language", "keyboard", "timezone", "comfort", "try"
        ]
    );
}
