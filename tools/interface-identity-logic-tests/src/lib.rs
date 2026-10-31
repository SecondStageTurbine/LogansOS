// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the interface-identity selection + kill-switch logic (Phase 7.5
//! CB-3, the 7.5-D identity layer).
//!
//! Mounts the *real* kernel pure-logic source via `#[path]` and asserts it on the
//! host — the load-bearing, equality-critical part of the build: the multi-interface
//! source pick ([`chosen_interface`]), the second cap-gating sub-switch
//! ([`egress_requires_cap`]), the no-driver-edit class inference ([`classify`]), and
//! above all the Rule-1 **kill-switch equality** — disabled ≡ the pre-identity
//! "interface 0 only" behavior (7.5-C2), bit-for-bit — swept over **every**
//! representable `(up, has_source)` configuration of four interfaces, plus the
//! contrapositive proving the enabled path genuinely differs where it should.

#![allow(dead_code)]

extern crate alloc;

#[path = "../../../kernel/src/net/identity_logic.rs"]
mod identity_logic;

#[cfg(test)]
mod tests {
    use super::identity_logic::*;

    /// The pre-identity behavior the disabled path must reproduce bit-for-bit:
    /// the net stack always used interface 0 (`get_device(0)` / `preferred_address(0)`).
    const BASELINE_INDEX: u32 = 0;

    /// An independent reference for the *enabled* decision (not the unit under test):
    /// the lowest-index interface that is UP and has a usable source, else interface 0.
    fn expected_on(cands: &[Candidate]) -> u32 {
        cands
            .iter()
            .filter(|c| c.up && c.has_source)
            .map(|c| c.index)
            .min()
            .unwrap_or(0)
    }

    /// Every `(up, has_source)` configuration of four interfaces (indices 0..4):
    /// 2 bits per interface ⇒ `4^4 = 256` candidate sets, swept exhaustively.
    fn all_candidate_sets() -> impl Iterator<Item = Vec<Candidate>> {
        const N: usize = 4;
        (0u32..(1 << (2 * N))).map(|mask| {
            (0..N)
                .map(|i| {
                    let up = mask & (1 << (2 * i)) != 0;
                    let has_source = mask & (1 << (2 * i + 1)) != 0;
                    Candidate::new(i as u32, up, has_source)
                })
                .collect::<Vec<Candidate>>()
        })
    }

    // === The Rule-1 kill-switch equality (the required proof artifact) ==========

    #[test]
    fn killswitch_off_is_interface_zero_for_all_inputs() {
        for cands in all_candidate_sets() {
            assert_eq!(
                chosen_interface(&cands, false),
                BASELINE_INDEX,
                "disabled MUST collapse to interface 0 (7.5-C2) for {cands:?}"
            );
        }
        // The empty candidate set (no devices registered) also collapses to 0, both
        // disabled and enabled — a single-device/no-device system is unchanged.
        assert_eq!(chosen_interface(&[], false), 0);
        assert_eq!(chosen_interface(&[], true), 0);
    }

    #[test]
    fn enabled_matches_independent_reference_for_all_inputs() {
        for cands in all_candidate_sets() {
            assert_eq!(
                chosen_interface(&cands, true),
                expected_on(&cands),
                "enabled selection mismatch for {cands:?}"
            );
        }
    }

    /// Contrapositive: without this, a no-op feature would pass the OFF-sweep vacuously.
    #[test]
    fn killswitch_on_off_differ_when_expected() {
        // eth0 down, eth1 up+source: OFF picks 0 (the hardcode), ON picks 1.
        let cands = [
            Candidate::new(0, false, true),
            Candidate::new(1, true, true),
        ];
        assert_eq!(chosen_interface(&cands, false), 0);
        assert_eq!(chosen_interface(&cands, true), 1);
        assert_ne!(
            chosen_interface(&cands, true),
            chosen_interface(&cands, false)
        );
    }

    // === Selection correctness (enabled) ========================================

    #[test]
    fn up_but_no_source_is_skipped() {
        let cands = [
            Candidate::new(0, true, false), // up, but no usable source
            Candidate::new(1, true, true),
        ];
        assert_eq!(chosen_interface(&cands, true), 1);
    }

    #[test]
    fn source_but_down_is_skipped() {
        let cands = [
            Candidate::new(0, false, true), // has a source, but link is down
            Candidate::new(1, true, true),
        ];
        assert_eq!(chosen_interface(&cands, true), 1);
    }

    #[test]
    fn lowest_index_wins_among_qualifying() {
        let cands = [
            Candidate::new(0, true, true),
            Candidate::new(1, true, true),
            Candidate::new(2, true, true),
        ];
        assert_eq!(chosen_interface(&cands, true), 0);
    }

    #[test]
    fn no_qualifying_interface_falls_back_to_zero() {
        let cands = [
            Candidate::new(0, false, false),
            Candidate::new(1, true, false),
            Candidate::new(2, false, true),
        ];
        assert_eq!(chosen_interface(&cands, true), 0);
    }

    // === The second sub-switch (ISC-5b) =========================================

    #[test]
    fn caps_subswitch_off_requires_no_cap() {
        assert!(!egress_requires_cap(false));
        assert!(egress_requires_cap(true));
    }

    // === No-driver-edit class inference =========================================

    #[test]
    fn classify_ethernet_vs_wireless() {
        // The in-tree ethernet identities classify as ethernet.
        assert_eq!(classify("virtio-net", "virtio-net0"), IfClass::Ethernet);
        assert_eq!(classify("cdc-ecm", "usb-eth0"), IfClass::Ethernet);
        // WiFi parts/markers classify as wireless (driver OR name).
        assert_eq!(classify("mt7601u", "mt7601u-sta"), IfClass::Wireless);
        assert_eq!(classify("acme", "wlan-x"), IfClass::Wireless);
        assert_eq!(classify("ieee80211", "y"), IfClass::Wireless);
        assert_eq!(classify("z", "wireless0"), IfClass::Wireless);
    }

    #[test]
    fn class_prefixes_are_stable() {
        assert_eq!(IfClass::Ethernet.prefix(), "eth");
        assert_eq!(IfClass::Wireless.prefix(), "wlan");
    }

    /// #218 bounce (PM): the snapshot's retry when the table grows between the
    /// sizing and the fill. Capacity 1, then length 2: the old retry
    /// (`reserve_exact(want - capacity)` on an empty `Vec`) reserved nothing
    /// from there and never returned. This must take exactly one more pass.
    #[test]
    fn a_snapshot_retries_once_when_the_table_grows_past_its_capacity() {
        use core::cell::Cell;
        let table = Cell::new(1usize);
        let passes = Cell::new(0u32);
        let out = snapshot_sized(
            || table.get(),
            |out: &mut Vec<u32>, _last| {
                passes.set(passes.get() + 1);
                // A registration lands between the sizing and the first fill.
                if passes.get() == 1 {
                    table.set(2);
                }
                assert!(passes.get() <= 2, "the retry made no progress");
                if table.get() > out.capacity() {
                    return false;
                }
                out.extend(0..table.get() as u32);
                true
            },
        );
        assert_eq!(passes.get(), 2);
        assert_eq!(out, [0, 1]);
        assert!(out.capacity() >= 2);
    }

    /// Chad #241 1b: a table that grows between every sizing and fill. The
    /// call still returns, after `SNAPSHOT_PASSES` passes, with the oldest
    /// entries that fit (the table only appends, so they are a prefix).
    #[test]
    fn a_snapshot_of_a_table_that_keeps_growing_is_bounded() {
        use core::cell::Cell;
        let table = Cell::new(1usize);
        let passes = Cell::new(0usize);
        let out = snapshot_sized(
            || table.get(),
            |out: &mut Vec<u32>, last| {
                passes.set(passes.get() + 1);
                // The probe's own cutoff: the unbounded retry never returns.
                assert!(passes.get() <= 1_000, "the retry has no bound");
                // A registration lands between every sizing and fill.
                table.set(table.get() + 1);
                if table.get() > out.capacity() && !last {
                    return false;
                }
                let fits = out.capacity();
                out.extend((0..table.get() as u32).take(fits));
                true
            },
        );
        assert_eq!(passes.get(), SNAPSHOT_PASSES);
        // The last pass was sized for SNAPSHOT_PASSES entries, and the table
        // then had one more: the snapshot is a prefix of at least that many.
        assert!(out.len() >= SNAPSHOT_PASSES, "{out:?}");
        assert_eq!(out[..], (0..out.len() as u32).collect::<Vec<_>>()[..]);
    }
}
