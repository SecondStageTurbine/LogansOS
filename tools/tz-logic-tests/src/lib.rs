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
}
