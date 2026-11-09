// SPDX-License-Identifier: MPL-2.0
//! Host KATs for S3's Time zone page (setup-menu THINK §2.2, §3.4, §5 S3).
//! The zone directory is a fixed table typed here; every row is written out
//! by hand.

use super::*;

/// A four-zone directory: the ids are libneo's (UTC 0, New York 105), the
/// details fixed strings (the view computes them from the clock).
struct Fixed;

const TABLE: [(u16, &str, &str); 4] = [
    (0, "UTC", "UTC, UTC+0"),
    (105, "America/New_York", "EDT, UTC-4"),
    (120, "America/Newfoundland", "NDT, UTC-2:30"),
    (300, "Europe/London", "BST, UTC+1"),
];

impl Zones for Fixed {
    fn shortlist(&self) -> Vec<u16> {
        alloc::vec![0, 105, 300]
    }
    fn search(&self, typed: &str) -> Vec<u16> {
        let typed = typed.to_ascii_lowercase().replace(' ', "_");
        TABLE
            .iter()
            .filter(|(_, n, _)| n.to_ascii_lowercase().contains(&typed))
            .map(|&(id, _, _)| id)
            .collect()
    }
    fn name(&self, id: u16) -> &'static str {
        TABLE.iter().find(|z| z.0 == id).map_or("?", |z| z.1)
    }
    fn detail(&self, id: u16) -> String {
        String::from(TABLE.iter().find(|z| z.0 == id).map_or("?", |z| z.2))
    }
}

fn rows(w: &Wizard) -> Vec<&'static str> {
    w.rows().into_iter().map(|s| &*s.leak()).collect()
}

fn wizard(confirmed: Option<Option<u16>>) -> Wizard {
    Wizard::new(false).with_zones(Box::new(Fixed), confirmed)
}

fn type_str(w: &mut Wizard, s: &str) {
    for c in s.bytes() {
        assert_eq!(w.on(Input::Char(c)), Outcome::Redraw, "typing {c}");
    }
}

/// The gate's walk (§5 S3): Begin, type `new y`, choose New York, leave
/// the page: one confirm, then Comfort; back to the page is review-only.
#[test]
fn search_choose_confirm_then_review() {
    let mut w = wizard(None);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
    assert_eq!(
        rows(&w),
        [
            "Time zone (1 of 3)",
            "Type a city to search, or pick one.",
            "> [ ] UTC (UTC, UTC+0)",
            "  [ ] America/New_York (EDT, UTC-4)",
            "  [ ] Europe/London (BST, UTC+1)",
            "  Continue",
            "Type to search. Up/Down, Enter to choose.",
        ]
    );
    // Space types here (it chooses on every other page).
    type_str(&mut w, "new y");
    assert_eq!(w.query(), "new y");
    assert_eq!(
        rows(&w),
        [
            "Time zone (1 of 3)",
            "Search: new y",
            "> [ ] America/New_York (EDT, UTC-4)",
            "  Continue",
            "Type to search. Up/Down, Enter to choose.",
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Redraw);
    assert_eq!(w.choices().tz, Some(105));
    assert_eq!(rows(&w)[2], "> [x] America/New_York (EDT, UTC-4)");
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    // Leaving with a zone chosen asks the view to confirm, once.
    assert_eq!(w.on(Input::Choose), Outcome::Confirm(105));
    assert_eq!(w.page(), Page::TimeZone);
    assert_eq!(w.confirmed(true), Outcome::Page(Page::Comfort));
    assert!(w.reviewed());
    // Back reaches the page review-only: no list, no typing, no confirm.
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::TimeZone));
    assert_eq!(
        rows(&w),
        [
            "Time zone (1 of 3)",
            "Time zone: America/New_York. The clock is set.",
            "Change it later at Home: timezone Area/City.",
            "> Continue",
            "Up/Down to move, Enter to choose, Esc to go back.",
        ]
    );
    assert_eq!(w.on(Input::Char(b'x')), Outcome::Nothing);
    assert_eq!(w.on(Input::Erase), Outcome::Nothing);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Comfort));
    w.on(Input::Up); // to Continue
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Try));
    assert_eq!(rows(&w)[1], "Time zone: America/New_York");
    assert_eq!(w.on(Input::Choose), Outcome::Done(Finish::Try));
    assert_eq!(w.zone_name(), Some("America/New_York"));
}

/// Erase, a search with no match, and the query limit.
#[test]
fn typing_erasing_and_no_match() {
    let mut w = wizard(None);
    w.on(Input::Choose);
    type_str(&mut w, "newf");
    assert_eq!(rows(&w)[2], "> [ ] America/Newfoundland (NDT, UTC-2:30)");
    assert_eq!(w.on(Input::Erase), Outcome::Redraw);
    // "new" matches both New York and Newfoundland.
    assert_eq!(
        rows(&w)[2..4],
        [
            "> [ ] America/New_York (EDT, UTC-4)",
            "  [ ] America/Newfoundland (NDT, UTC-2:30)"
        ]
    );
    type_str(&mut w, "zz");
    assert_eq!(rows(&w)[1], "Search: newzz  (no zone matches)");
    assert_eq!(rows(&w)[2], "> Continue");
    for _ in 0..5 {
        w.on(Input::Erase);
    }
    assert_eq!(w.query(), "");
    assert_eq!(w.on(Input::Erase), Outcome::Nothing);
    assert_eq!(rows(&w)[1], "Type a city to search, or pick one.");
    for _ in 0..QUERY_MAX {
        assert_eq!(w.on(Input::Char(b'a')), Outcome::Redraw);
    }
    assert_eq!(w.on(Input::Char(b'a')), Outcome::Nothing);
    assert_eq!(w.on(Input::Char(0x07)), Outcome::Nothing);
}

/// No zone chosen: Continue goes on with no confirm (the clock stays
/// unconfirmed, §2.2), and the summary says so.
#[test]
fn continue_without_a_zone_confirms_nothing() {
    let mut w = wizard(None);
    w.on(Input::Choose);
    w.on(Input::Up); // to Continue
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Comfort));
    assert!(!w.reviewed());
    w.on(Input::Up);
    w.on(Input::Choose);
    assert_eq!(rows(&w)[1], "Time zone: not set");
    assert_eq!(w.choices().tz, None);
}

/// A refused confirm keeps the zone as a choice and the page editable; the
/// summary says the clock isn't set.
#[test]
fn a_refused_confirm_stays_editable() {
    let mut w = wizard(None);
    w.on(Input::Choose);
    w.on(Input::Down); // New York
    w.on(Input::Choose);
    w.on(Input::Up); // UTC
    w.on(Input::Up); // Continue (wraps)
    assert_eq!(w.on(Input::Choose), Outcome::Confirm(105));
    assert_eq!(w.confirmed(false), Outcome::Page(Page::Comfort));
    assert!(!w.reviewed());
    // Back: still editable, New York still chosen; leaving asks again (the
    // view reads the clock's status first and makes no second call once
    // it's confirmed).
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::TimeZone));
    assert_eq!(rows(&w)[1], "Type a city to search, or pick one.");
    assert_eq!(rows(&w)[3], "  [x] America/New_York (EDT, UTC-4)");
    w.on(Input::Up); // Continue
    assert_eq!(w.on(Input::Choose), Outcome::Confirm(105));
    assert_eq!(w.confirmed(false), Outcome::Page(Page::Comfort));
    w.on(Input::Up);
    w.on(Input::Choose);
    assert_eq!(w.page(), Page::Try);
    assert_eq!(rows(&w)[1], "Time zone: America/New_York (clock not set)");
}

/// Already confirmed at the start (an earlier session): review-only from
/// the first visit, and Continue never asks for a second confirm.
#[test]
fn confirmed_at_start_is_review_only() {
    let mut w = wizard(Some(Some(300)));
    assert!(w.reviewed());
    w.on(Input::Choose);
    assert_eq!(
        rows(&w)[1..4],
        [
            "Time zone: Europe/London. The clock is set.",
            "Change it later at Home: timezone Area/City.",
            "> Continue"
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Comfort));
    // Confirmed with no zone known (status 1, no record): still no confirm.
    let mut w = wizard(Some(None));
    w.on(Input::Choose);
    assert_eq!(rows(&w)[1], "Time zone: not set. The clock is set.");
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Comfort));
}

/// The kill-switch: no Time zone page, `N of M` counts without it.
#[test]
fn without_the_time_zone_page() {
    let mut w = Wizard::new(false).without_time_zone();
    assert_eq!(w.pages(), [Page::Welcome, Page::Comfort, Page::Try]);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Comfort));
    assert_eq!(rows(&w)[0], "Comfort (1 of 2)");
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Welcome));
}

/// Every S3 page fits the card, on every focus, searching or not.
#[test]
fn s3_pages_fit_the_card() {
    let mut w = wizard(None);
    w.on(Input::Choose);
    for q in ["", "a", "new", "new y", "zz"] {
        while w.on(Input::Erase) != Outcome::Nothing {}
        type_str(&mut w, q);
        for _ in 0..w.options().len() {
            assert!(w.rows().len() <= MAX_ROWS, "{:?}", w.rows());
            w.on(Input::Down);
        }
    }
}

/// Space still chooses outside the search, and other characters do nothing
/// there (S1's key map).
#[test]
fn characters_outside_the_search() {
    let mut w = wizard(None);
    assert_eq!(w.on(Input::Char(b'q')), Outcome::Nothing);
    assert_eq!(w.on(Input::Erase), Outcome::Nothing);
    assert_eq!(w.on(Input::Char(b' ')), Outcome::Page(Page::TimeZone));
    let mut s1 = Wizard::new(false);
    s1.on(Input::Choose);
    // S1's page (no directory): Space chooses, letters do nothing.
    assert_eq!(s1.on(Input::Char(b'n')), Outcome::Nothing);
    assert_eq!(s1.on(Input::Char(b' ')), Outcome::Redraw);
}

/// The PM's rule (#92): after a failed confirm the wizard takes what the
/// store holds, and the next page says the zone couldn't be saved, once.
#[test]
fn a_failed_confirm_takes_the_stores_zone_and_says_so() {
    let mut w = wizard(None);
    w.on(Input::Choose);
    type_str(&mut w, "new y");
    w.on(Input::Choose);
    w.on(Input::Up);
    assert_eq!(w.on(Input::Choose), Outcome::Confirm(105));
    // The store kept New York and the clock confirmed with it.
    assert_eq!(w.confirmed(false), Outcome::Page(Page::Comfort));
    w.reconciled(Some(105), true);
    w.notice(String::from(
        "Couldn't save the time zone. In use: America/New_York.",
    ));
    assert_eq!(
        rows(&w)[..3],
        [
            "Comfort (2 of 3)",
            "Couldn't save the time zone. In use: America/New_York.",
            "Make the screen calmer, if you like.",
        ]
    );
    assert!(w.rows().len() <= MAX_ROWS);
    assert!(w.reviewed());
    // Shown once: gone on the next page.
    w.on(Input::Back);
    assert_eq!(
        rows(&w)[1],
        "Time zone: America/New_York. The clock is set."
    );
    // The store held nothing: not set, editable.
    let mut w = wizard(None);
    w.on(Input::Choose);
    w.on(Input::Down);
    w.on(Input::Choose);
    w.on(Input::Up);
    w.on(Input::Up);
    assert_eq!(w.on(Input::Choose), Outcome::Confirm(105));
    w.confirmed(false);
    w.reconciled(None, false);
    assert_eq!(w.zone_name(), None);
    assert!(!w.reviewed());
}

/// Chad, #92 relay 379: a failed confirm whose store couldn't be re-read. The
/// page stays, editable, still holding the chosen zone; the notice names no
/// stored zone; leaving again confirms again.
#[test]
fn an_unresolved_confirm_stays_on_the_page() {
    let mut w = wizard(None);
    w.on(Input::Choose);
    type_str(&mut w, "new y");
    w.on(Input::Choose);
    w.on(Input::Up);
    assert_eq!(w.on(Input::Choose), Outcome::Confirm(105));
    let before = rows(&w);
    assert_eq!(w.unresolved(), Outcome::Page(Page::TimeZone));
    w.notice(String::from(
        "Couldn't save the time zone. Restart to check it.",
    ));
    let after = rows(&w);
    assert_eq!(after[0], before[0]);
    assert_eq!(
        after[1],
        "Couldn't save the time zone. Restart to check it."
    );
    assert_eq!(after.len(), before.len() + 1);
    // Shown afresh: the focus is on the first row (the chosen zone).
    assert_eq!(
        after[after.len() - 4..],
        [
            "Search: new y",
            "> [x] America/New_York (EDT, UTC-4)",
            "  Continue",
            "Type to search. Up/Down, Enter to choose.",
        ]
    );
    assert!(w.rows().len() <= MAX_ROWS);
    assert!(!w.reviewed());
    assert_eq!(w.zone_name(), Some("America/New_York"));
    assert_eq!(after.len(), 6);
    // Leaving forward asks for the confirm again.
    w.on(Input::Down);
    assert_eq!(w.on(Input::Choose), Outcome::Confirm(105));
    // Back from it leaves for Welcome, and the notice is gone.
    let mut w = wizard(None);
    w.on(Input::Choose);
    type_str(&mut w, "new y");
    w.on(Input::Choose);
    w.on(Input::Up);
    w.on(Input::Choose);
    w.unresolved();
    w.notice(String::from("x"));
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Welcome));
    assert!(!rows(&w).contains(&"x"));
}
