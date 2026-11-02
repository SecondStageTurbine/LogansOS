// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `userland/libneo/src/tz.rs` (mounted by `#[path]`, so these
//! run the exact code userland runs) and its generated table.
//!
//! Every expected instant is worked out by hand in the comment beside it
//! (day counts from year lengths; `docs/THINK-2026-11-02-time-zones.md` §4),
//! then cross-checked once with Python's `zoneinfo` (tzdata 2026e), never
//! computed by the code under test.
//!
//! Day numbers: 2026-01-01 is day 20454 since 1970-01-01 (56 years, 14 of
//! them leap: 1972..=2024), and a Thursday.

#![allow(dead_code)]

#[path = "../../../userland/libneo/src/tz.rs"]
mod tz;

#[cfg(test)]
mod tests {
    use super::tz::{
        self, LocalResult, ZONES, ZoneId, days_from_civil, local_to_utc, offset_at, utc_to_local,
        zone_by_name,
    };

    fn id(name: &str) -> ZoneId {
        zone_by_name(name).unwrap_or_else(|| panic!("{name} not in the table"))
    }

    /// (offset seconds, abbreviation, is_dst) at `utc` in `zone`.
    fn at(zone: &str, utc: i64) -> (i32, &'static str, bool) {
        let (_, o) = utc_to_local(utc, id(zone));
        (o.seconds, o.abbr, o.is_dst)
    }

    #[test]
    fn day_counts() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2026, 1, 1), 20454);
        // + Jan 31 + Feb 28 + 7.
        assert_eq!(days_from_civil(2026, 3, 8), 20520);
        // + 304 (Jan..Oct).
        assert_eq!(days_from_civil(2026, 11, 1), 20758);
        // + 90 + 3.
        assert_eq!(days_from_civil(2026, 4, 4), 20547);
        // + 273 + 2.
        assert_eq!(days_from_civil(2026, 10, 3), 20729);
    }

    #[test]
    fn table_identity_and_pinned_ids() {
        assert_eq!(tz::TZDATA_VERSION, "2026e");
        assert_eq!(
            tz::TZDATA_COMMIT,
            "039ef27cc5f062a2055cb67435d6d71adbefd27d"
        );
        assert_eq!(ZONES[0].name, "Etc/UTC");
        assert_eq!(tz::UTC, 0);
        // Ids are append-only: pin the ones the KATs and settings rely on.
        assert_eq!(id("America/New_York"), 105);
        assert_eq!(ZONES[105].name, "America/New_York");
        // Aliases resolve to their zone; case is ignored.
        assert_eq!(id("UTC"), 0);
        assert_eq!(id("US/Eastern"), 105);
        assert_eq!(id("america/new_york"), 105);
        assert_eq!(id("Asia/Calcutta"), id("Asia/Kolkata"));
        assert_eq!(zone_by_name("Mars/Olympus_Mons"), None);
        // Names are unique.
        for (i, z) in ZONES.iter().enumerate() {
            assert!(ZONES[..i].iter().all(|o| o.name != z.name), "{}", z.name);
        }
    }

    /// New York 2026. Spring forward: Sunday 8 March (March 1 is a Sunday)
    /// at 02:00 EST = 07:00 UTC = 20520 * 86400 + 25200 = 1772953200.
    /// Fall back: Sunday 1 November at 02:00 EDT = 06:00 UTC =
    /// 20758 * 86400 + 21600 = 1793512800.
    #[test]
    fn new_york_2026() {
        assert_eq!(at("America/New_York", 1772953199), (-18000, "EST", false));
        assert_eq!(at("America/New_York", 1772953200), (-14400, "EDT", true));
        assert_eq!(at("America/New_York", 1793512799), (-14400, "EDT", true));
        assert_eq!(at("America/New_York", 1793512800), (-18000, "EST", false));
        // Local wall times either side (local seconds counted as if UTC):
        // 01:59:59 EST and 03:00:00 EDT on 8 March; 01:59:59 EDT and
        // 01:00:00 EST on 1 November.
        let ny = id("America/New_York");
        assert_eq!(utc_to_local(1772953199, ny).0, 20520 * 86400 + 7199);
        assert_eq!(utc_to_local(1772953200, ny).0, 20520 * 86400 + 10800);
        assert_eq!(utc_to_local(1793512799, ny).0, 20758 * 86400 + 7199);
        assert_eq!(utc_to_local(1793512800, ny).0, 20758 * 86400 + 3600);
        // J.c.'s reading: 2026-11-02 06:29 UTC (day 20759) is 01:29 EST.
        assert_eq!(
            utc_to_local(20759 * 86400 + 6 * 3600 + 29 * 60, ny).0,
            20759 * 86400 + 3600 + 29 * 60
        );
    }

    #[test]
    fn new_york_local_to_utc() {
        let ny = id("America/New_York");
        // 1 Nov 01:30 local: EDT gives 05:30 UTC (1793511000), EST 06:30 UTC
        // (1793514600).
        assert_eq!(
            local_to_utc(20758 * 86400 + 5400, ny),
            LocalResult::Ambiguous(1793511000, 1793514600)
        );
        // 8 Mar 02:30 local does not exist. Before the gap (EST, -5 h):
        // 07:30 UTC = 1772955000; after it (EDT, -4 h): 06:30 UTC = 1772951400.
        assert_eq!(
            local_to_utc(20520 * 86400 + 9000, ny),
            LocalResult::Gap {
                offset_before: 1772955000,
                offset_after: 1772951400
            }
        );
        // 8 Mar 01:30 local: EST only, 06:30 UTC.
        assert_eq!(
            local_to_utc(20520 * 86400 + 5400, ny),
            LocalResult::Single(1772951400)
        );
    }

    /// Sydney 2026 (`AEST-10AEDT,M10.1.0,M4.1.0/3`): the year starts in DST.
    /// April 1 is a Wednesday, so DST ends Sunday 5 April at 03:00 AEDT =
    /// 4 April 16:00 UTC = 20547 * 86400 + 57600 = 1775318400. October 1 is a
    /// Thursday, so DST starts Sunday 4 October at 02:00 AEST = 3 October
    /// 16:00 UTC = 20729 * 86400 + 57600 = 1791043200.
    #[test]
    fn sydney_2026() {
        assert_eq!(at("Australia/Sydney", 20454 * 86400), (39600, "AEDT", true));
        assert_eq!(at("Australia/Sydney", 1775318399), (39600, "AEDT", true));
        assert_eq!(at("Australia/Sydney", 1775318400), (36000, "AEST", false));
        assert_eq!(at("Australia/Sydney", 1791043199), (36000, "AEST", false));
        assert_eq!(at("Australia/Sydney", 1791043200), (39600, "AEDT", true));
        let syd = id("Australia/Sydney");
        // 5 Apr (day 20548) 02:30 local: AEDT gives 15:30 UTC on the 4th
        // (1775316600), AEST 16:30 UTC (1775320200).
        assert_eq!(
            local_to_utc(20548 * 86400 + 9000, syd),
            LocalResult::Ambiguous(1775316600, 1775320200)
        );
        // 4 Oct (day 20730) 02:30 local does not exist. Before the gap
        // (AEST, +10): 16:30 UTC = 1791045000; after (AEDT, +11): 15:30 UTC
        // = 1791041400.
        assert_eq!(
            local_to_utc(20730 * 86400 + 9000, syd),
            LocalResult::Gap {
                offset_before: 1791045000,
                offset_after: 1791041400
            }
        );
    }

    #[test]
    fn other_rule_shapes() {
        // Kolkata: +05:30, no DST.
        assert_eq!(at("Asia/Kolkata", 1793512800), (19800, "IST", false));
        // Dublin: IST (+1) is standard and GMT the "daylight" (negative DST).
        // Winter (1 January) is GMT, summer (1 July, day 20454 + 181) IST.
        assert_eq!(at("Europe/Dublin", 20454 * 86400), (0, "GMT", true));
        assert_eq!(
            at("Europe/Dublin", (20454 + 181) * 86400),
            (3600, "IST", false)
        );
        // A numeric abbreviation: Dubai is <+04>.
        assert_eq!(at("Asia/Dubai", 1793512800), (14400, "+04", false));
        // Nuuk (`<-02>2<-01>,M3.5.0/-1,M10.5.0/0`): DST starts the last
        // Sunday of March (29 March 2026, day 20541) at -1:00 local standard,
        // i.e. Saturday 28 March 23:00 at -02 = 29 March 01:00 UTC =
        // 20541 * 86400 + 3600.
        assert_eq!(
            at("America/Nuuk", 20541 * 86400 + 3599),
            (-7200, "-02", false)
        );
        assert_eq!(
            at("America/Nuuk", 20541 * 86400 + 3600),
            (-3600, "-01", true)
        );
    }

    /// The known limitation (THINK §2.4): a date before a zone's last
    /// explicit transition converts with its final rule. Casablanca's 2026e
    /// footer is `<+00>0`, so 2026-06-01 (actually +01) reads +00.
    #[test]
    fn final_rule_limitation_is_pinned() {
        assert_eq!(
            at("Africa/Casablanca", (20454 + 151) * 86400 + 43200),
            (0, "+00", false)
        );
    }

    #[test]
    fn utc_is_identity() {
        for t in [0, 1772953200, 1793512800, 4102444800] {
            assert_eq!(utc_to_local(t, tz::UTC).0, t);
            assert_eq!(local_to_utc(t, tz::UTC), LocalResult::Single(t));
        }
        // An unknown id falls back to UTC.
        assert_eq!(utc_to_local(1000, u16::MAX).0, 1000);
        let utc_rule = ZONES[0].rule;
        assert_eq!(offset_at(&utc_rule, 0).abbr, "UTC");
    }

    /// Dexter (#59): every function is defined for any `i64`. Outside
    /// ±2^40 s the offset is the bound's, and results saturate; in the
    /// test profile overflow checks are on, so a panic would fail this.
    #[test]
    fn the_i64_extremes_are_defined() {
        use super::tz::{INSTANT_MAX, INSTANT_MIN};
        let ny = id("America/New_York");
        let sydney = id("Australia/Sydney");
        for z in [tz::UTC, ny, sydney, u16::MAX] {
            let (hi, o_hi) = utc_to_local(i64::MAX, z);
            assert_eq!(o_hi, utc_to_local(INSTANT_MAX, z).1);
            assert_eq!(hi, i64::MAX.saturating_add(o_hi.seconds as i64));
            let (lo, o_lo) = utc_to_local(i64::MIN, z);
            assert_eq!(o_lo, utc_to_local(INSTANT_MIN, z).1);
            assert_eq!(lo, i64::MIN.saturating_add(o_lo.seconds as i64));
            for t in [i64::MAX, i64::MIN, i64::MAX - 1, i64::MIN + 1] {
                match local_to_utc(t, z) {
                    LocalResult::Single(u) | LocalResult::Ambiguous(u, _) => {
                        assert_eq!(u.signum(), t.signum())
                    }
                    LocalResult::Gap { offset_after, .. } => {
                        assert_eq!(offset_after.signum(), t.signum())
                    }
                }
            }
        }
        // New York west of UTC: the top saturates nowhere (MAX - 5 h or
        // 4 h), the bottom saturates to MIN.
        assert!(matches!(
            utc_to_local(i64::MAX, ny).1.seconds,
            -18000 | -14400
        ));
        assert_eq!(utc_to_local(i64::MIN, ny).0, i64::MIN);
        // Sydney east of UTC: the top saturates to MAX.
        assert_eq!(utc_to_local(i64::MAX, sydney).0, i64::MAX);
        // Inside the range nothing changed: the 2026 KAT instants hold.
        assert_eq!(utc_to_local(1793512800, ny).1.abbr, "EST");
    }

    /// The session's reading of the RTC (THINK §2.1/§2.3).
    #[test]
    fn rtc_reading_as_utc() {
        use super::tz::rtc_reading_utc;
        let ny = id("America/New_York");
        // J.c.'s Zenbook: the RTC holds 2026-11-02 01:29 EST (day 20759,
        // + 5340 s); the instant is 06:29 UTC (+ 23340 s).
        let wall = 20759 * 86400 + 5340;
        assert_eq!(rtc_reading_utc(wall, ny, true), Some(20759 * 86400 + 23340));
        // An RTC that holds UTC: the reading is the instant, whatever the zone.
        assert_eq!(rtc_reading_utc(wall, ny, false), Some(wall));
        assert_eq!(rtc_reading_utc(wall, tz::UTC, false), Some(wall));
        // Local RTC, no zone: unknown ("zone not set").
        assert_eq!(rtc_reading_utc(wall, tz::UTC, true), None);
        // The repeated hour takes the earlier instant (EDT): 1 Nov 01:30.
        assert_eq!(
            rtc_reading_utc(20758 * 86400 + 5400, ny, true),
            Some(1793511000)
        );
        // The spring gap takes the offset after it: 8 Mar 02:30 -> 06:30 UTC.
        assert_eq!(
            rtc_reading_utc(20520 * 86400 + 9000, ny, true),
            Some(1772951400)
        );
    }

    /// Janus (#60): once realtime is confirmed UTC, only the display applies
    /// the zone. Every instant below is worked out by hand from day numbers
    /// (2026-11-02 is day 20759, 2026-11-01 day 20758).
    #[test]
    fn confirmed_utc_is_only_displayed_in_the_zone() {
        use super::tz::{Basis, Shown, confirm_offset, needs_restart, shown};
        let ny = id("America/New_York");
        let sydney = id("Australia/Sydney");
        // J.c.'s Zenbook: RTC 01:29 local, New York: confirm -18000 s, and
        // realtime becomes 06:29 UTC = 1793577600 + 23340.
        let wall = 20759 * 86400 + 5340;
        assert_eq!(confirm_offset(wall, ny, true), Some(-18000));
        let utc = 1_793_600_940;
        assert_eq!(
            shown(utc, Basis::Utc, ny),
            (20759 * 86400 + 5340, Shown::Zone("EST"))
        );
        // Then `timezone Australia/Sydney`: 06:29 UTC is 17:29 AEDT (UTC+11),
        // not 01:29 AEDT (the RTC's reading re-interpreted).
        assert_eq!(
            shown(utc, Basis::Utc, sydney),
            (20759 * 86400 + 62940, Shown::Zone("AEDT"))
        );
        // The fold: confirmed earlier, now 06:30 UTC on 1 Nov, after the
        // 06:00 UTC change: 01:30 EST, whatever is re-entered.
        assert_eq!(
            shown(20758 * 86400 + 23400, Basis::Utc, ny),
            (20758 * 86400 + 5400, Shown::Zone("EST"))
        );
        // No zone, confirmed: UTC itself.
        assert_eq!(shown(utc, Basis::Utc, tz::UTC), (utc, Shown::Zone("UTC")));
        // Unconfirmed and the kill-switch: the reading as it is.
        assert_eq!(shown(wall, Basis::Rtc, ny), (wall, Shown::NotSet));
        assert_eq!(shown(wall, Basis::Legacy, ny), (wall, Shown::Plain));
        assert_eq!(Basis::of(Ok(true)), Basis::Utc);
        assert_eq!(Basis::of(Ok(false)), Basis::Rtc);
        assert_eq!(Basis::of(Err(-38)), Basis::Legacy);
        // A later mode or zone that would read the RTC differently needs a
        // restart; the same settings don't.
        assert!(!needs_restart(utc, -18000, ny, true));
        assert!(needs_restart(utc, -18000, ny, false));
        assert!(needs_restart(utc, -18000, sydney, true));
        assert!(needs_restart(utc, -18000, tz::UTC, true));
    }

    #[test]
    fn home_verbs_parse() {
        use super::tz::{ClockVerb, parse_verb};
        assert_eq!(parse_verb(b"timezone"), Some(ClockVerb::Show));
        assert_eq!(parse_verb(b"  TimeZone  "), Some(ClockVerb::Show));
        assert_eq!(
            parse_verb(b"timezone America/New_York"),
            Some(ClockVerb::Zone(b"America/New_York"))
        );
        assert_eq!(
            parse_verb(b"timezone  new york "),
            Some(ClockVerb::Zone(b"new york"))
        );
        assert_eq!(parse_verb(b"rtc local"), Some(ClockVerb::Rtc(true)));
        assert_eq!(parse_verb(b"RTC UTC"), Some(ClockVerb::Rtc(false)));
        // Not verbs: they go to Arthur.
        assert_eq!(parse_verb(b"rtc"), None);
        assert_eq!(parse_verb(b"rtc please"), None);
        assert_eq!(parse_verb(b"what timezone am i in"), None);
        assert_eq!(parse_verb(b"timezones"), None);
        assert_eq!(parse_verb(b""), None);
    }

    #[test]
    fn typed_zone_names_resolve() {
        use super::tz::find_zone;
        let ny = id("America/New_York");
        assert_eq!(find_zone(b"America/New_York"), Ok(ny));
        assert_eq!(find_zone(b"america/new_york"), Ok(ny));
        assert_eq!(find_zone(b"America/New York"), Ok(ny));
        assert_eq!(find_zone(b"us/eastern"), Ok(ny));
        assert_eq!(find_zone(b"new york"), Ok(ny));
        assert_eq!(find_zone(b"New_York"), Ok(ny));
        assert_eq!(find_zone(b"sydney"), Ok(id("Australia/Sydney")));
        assert_eq!(find_zone(b"utc"), Ok(0));
        // Nothing exact: up to three zones containing it, in id order.
        let Err(near) = find_zone(b"york") else {
            panic!("york is not a zone name");
        };
        assert_eq!(near[0], Some(ny));
        assert_eq!(find_zone(b"atlantis"), Err([None, None, None]));
    }
}
