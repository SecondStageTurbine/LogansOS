// SPDX-License-Identifier: MPL-2.0
//! Host KATs for S4's Network page (setup-menu THINK §3.3, §5 S4). Every row
//! is written out by hand.

use super::*;

fn rows(w: &Wizard) -> Vec<String> {
    w.rows()
}

fn on_network(n: Network) -> Wizard {
    let mut w = Wizard::new(false).with_network(n);
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::Network));
    w
}

/// The gate's page: CDC-ECM with slirp's lease.
#[test]
fn connected_shows_the_address_driver_and_lease() {
    let w = on_network(Network::Connected {
        driver: String::from("cdc-ecm"),
        address: Some([10, 0, 2, 15]),
        lease_secs: Some(86_000),
    });
    assert_eq!(
        rows(&w),
        [
            "Network (1 of 4)",
            "Wired: connected, 10.0.2.15",
            "Device: cdc-ecm. Lease: 23 h 53 min left.",
            "Wi-Fi: coming in a later version.",
            "> Continue",
            HINT,
        ]
    );
    assert!(w.rows().len() <= MAX_ROWS);
}

/// The control: no NIC. Continue is still there, and leads on.
#[test]
fn no_link_still_continues() {
    let mut w = on_network(Network::NoLink { driver: None });
    assert_eq!(
        rows(&w),
        [
            "Network (1 of 4)",
            "Wired: no link.",
            "Wi-Fi: coming in a later version.",
            "> Continue without a network",
            HINT,
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
    // A device without link names its driver.
    let w = on_network(Network::NoLink {
        driver: Some(String::from("rtl8125")),
    });
    assert_eq!(rows(&w)[1..3], ["Wired: no link.", "Device: rtl8125."]);
}

/// The kill-switch (`net-info` off: `ENOSYS`): the page says so, and goes on.
#[test]
fn unavailable_says_so_and_continues() {
    let mut w = on_network(Network::Unavailable);
    assert_eq!(
        rows(&w),
        [
            "Network (1 of 4)",
            "Network status is unavailable.",
            "Wi-Fi: coming in a later version.",
            "> Continue without a network",
            HINT,
        ]
    );
    assert_eq!(w.on(Input::Choose), Outcome::Page(Page::TimeZone));
}

#[test]
fn connected_without_an_address_or_lease() {
    let w = on_network(Network::Connected {
        driver: String::from("rtl8125"),
        address: None,
        lease_secs: None,
    });
    assert_eq!(
        rows(&w)[1..5],
        [
            "Wired: connected, no address yet.",
            "Device: rtl8125.",
            "Wi-Fi: coming in a later version.",
            "> Continue without a network",
        ]
    );
}

/// The page sits between Keyboard and Time zone, and Back returns to it.
#[test]
fn its_place_in_the_walk() {
    let w = Wizard::new(true).with_network(Network::Unavailable);
    assert_eq!(
        w.pages(),
        [
            Page::Welcome,
            Page::Language,
            Page::Keyboard,
            Page::Network,
            Page::TimeZone,
            Page::Comfort,
            Page::Try,
        ]
    );
    let mut w = on_network(Network::Unavailable);
    w.on(Input::Choose);
    assert_eq!(w.page(), Page::TimeZone);
    assert_eq!(rows(&w)[0], "Time zone (2 of 4)");
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Network));
    assert_eq!(w.on(Input::Back), Outcome::Page(Page::Welcome));
    // Without it, the walk is S1's.
    assert!(!Wizard::new(false).pages().contains(&Page::Network));
}

/// Keys that choose nothing on the page do nothing; it has one option.
#[test]
fn the_page_has_only_continue() {
    let mut w = on_network(Network::Unavailable);
    assert_eq!(w.options().len(), 1);
    assert_eq!(w.on(Input::Down), Outcome::Redraw);
    assert_eq!(w.focus(), 0);
    assert_eq!(w.on(Input::Char(b'x')), Outcome::Nothing);
    // The summary doesn't change: the network isn't a choice.
    assert_eq!(w.summary(), Wizard::new(false).summary());
}

#[test]
fn lease_words() {
    assert_eq!(lease_left(0), "under a minute left");
    assert_eq!(lease_left(59), "under a minute left");
    assert_eq!(lease_left(60), "1 min left");
    assert_eq!(lease_left(3599), "59 min left");
    assert_eq!(lease_left(3600), "1 h 0 min left");
    assert_eq!(lease_left(86_000), "23 h 53 min left");
}
