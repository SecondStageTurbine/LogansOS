// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the Zenbook USB input lane's pure logic:
//! `kernel/src/drivers/usb/xhci_silicon_logic.rs` (the xHCI extended
//! capability walk, the choice between several xHCIs, the BIOS handoff and
//! SMI-disable writes, the PCI power-state write, the hub slot context, and
//! the port service's decisions).
//!
//! The module is mounted through `#[path]`, so these tests run the exact code
//! the kernel runs. Every expected value is worked out by hand in the comment
//! beside it from the xHCI 1.2, USB 2.0 and PCI PM 1.2 register layouts,
//! never computed from the code under test.

#![allow(dead_code)]

#[path = "../../../kernel/src/drivers/usb/xhci_silicon_logic.rs"]
mod xhci_silicon_logic;

#[cfg(test)]
mod tests {
    use super::xhci_silicon_logic::*;
    use std::collections::BTreeMap;

    /// A capability register file: byte offset -> dword. Unlisted offsets
    /// read as zero. Every read is recorded, so a test can assert that no
    /// read went past the window.
    struct Regs {
        map: BTreeMap<u64, u32>,
        reads: Vec<u64>,
    }

    impl Regs {
        fn new(entries: &[(u64, u32)]) -> Self {
            Self {
                map: entries.iter().copied().collect(),
                reads: Vec::new(),
            }
        }
        fn walk(&mut self, hcc1: u32, window: u64) -> ExtCaps {
            let map = &self.map;
            let reads = &mut self.reads;
            walk_ext_caps(
                &mut |offset| {
                    reads.push(offset);
                    map.get(&offset).copied().unwrap_or(0)
                },
                hcc1,
                window,
            )
        }
    }

    /// An Intel south-xHCI-shaped chain: xECP = 0x8000 bytes = 0x2000 dwords,
    /// so HCCPARAMS1[31:16] = 0x2000.
    ///   0x8000  USBLEGSUP    id 1, next 4 dwords   -> 0x0000_0401
    ///   0x8010  USB 2.0      id 2, next 4, rev 2.0 -> 0x0200_0402
    ///   0x8018  ports 1..=12 offset 1, count 12    -> 0x0000_0C01
    ///   0x8020  USB 3.1      id 2, next 0, rev 3.1 -> 0x0310_0002
    ///   0x8028  ports 13..=16 offset 13, count 4   -> 0x0000_040D
    fn south_chain() -> Regs {
        Regs::new(&[
            (0x8000, 0x0000_0401),
            (0x8010, 0x0200_0402),
            (0x8018, 0x0000_0C01),
            (0x8020, 0x0310_0002),
            (0x8028, 0x0000_040D),
        ])
    }

    #[test]
    fn walk_finds_legacy_and_both_protocols() {
        let mut regs = south_chain();
        let caps = regs.walk(0x2000_0000, 0x1_0000);
        assert_eq!(caps.legacy, Some(0x8000));
        assert_eq!(caps.stop, None);
        assert_eq!(
            caps.protocols(),
            &[
                ProtocolRange {
                    major: 2,
                    minor: 0x00,
                    first_port: 1,
                    count: 12
                },
                ProtocolRange {
                    major: 3,
                    minor: 0x10,
                    first_port: 13,
                    count: 4
                },
            ]
        );
        assert_eq!(caps.usb2_ports(), 12);
        assert_eq!(caps.port_major(1), Some(2));
        assert_eq!(caps.port_major(12), Some(2));
        assert_eq!(caps.port_major(13), Some(3));
        assert_eq!(caps.port_major(16), Some(3));
        assert_eq!(caps.port_major(17), None);
        assert_eq!(caps.port_major(0), None);
    }

    /// The Type-C subsystem's xHCI: SuperSpeed ports only, no legacy
    /// capability. xECP at 0x80 bytes = 0x20 dwords.
    #[test]
    fn walk_of_a_superspeed_only_controller_has_no_usb2_ports() {
        let mut regs = Regs::new(&[(0x80, 0x0320_0002), (0x88, 0x0000_0401)]);
        let caps = regs.walk(0x0020_0000, 0x1_0000);
        assert_eq!(caps.legacy, None);
        assert_eq!(caps.usb2_ports(), 0);
        assert_eq!(caps.port_major(1), Some(3));
        assert_eq!(caps.port_major(4), Some(3));
    }

    #[test]
    fn walk_never_reads_outside_the_window() {
        // xECP at 0xFFF8 bytes: the 12 bytes a protocol needs end at
        // 0x1_0004 > 0x1_0000, so nothing is read.
        let mut regs = Regs::new(&[]);
        let caps = regs.walk(0x3FFE_0000, 0x1_0000);
        assert_eq!(caps.stop, Some(WalkStop::LeftWindow(0xFFF8)));
        assert!(regs.reads.is_empty());

        // A chain that walks off the end of a 0x9000 window: each header is
        // id 5 with next = 0xFF dwords (0x3FC bytes). 0x8000, 0x83FC, 0x87F8,
        // 0x8BF4 and 0x8FF0 (0x8FF0 + 12 = 0x8FFC <= 0x9000) are read; the
        // next pointer, 0x93EC, is past the window and is not.
        let mut chain = Vec::new();
        let mut offset = 0x8000u64;
        while offset + 12 <= 0x9000 {
            chain.push((offset, 0x0000_FF05));
            offset += 0xFF << 2;
        }
        let mut regs = Regs::new(&chain);
        let caps = regs.walk(0x2000_0000, 0x9000);
        assert_eq!(caps.stop, Some(WalkStop::LeftWindow(0x93EC)));
        assert_eq!(regs.reads, vec![0x8000, 0x83FC, 0x87F8, 0x8BF4, 0x8FF0]);
    }

    #[test]
    fn walk_stops_after_max_caps() {
        // 100 capabilities of id 0xC0, each pointing 1 dword on.
        let chain: Vec<(u64, u32)> = (0..100).map(|i| (0x100 + 4 * i, 0x0000_01C0)).collect();
        let mut regs = Regs::new(&chain);
        let caps = regs.walk(0x0040_0000, 0x1_0000);
        assert_eq!(caps.stop, Some(WalkStop::TooMany));
        assert_eq!(regs.reads.len(), MAX_EXT_CAPS);
    }

    #[test]
    fn walk_keeps_the_first_legacy_and_caps_protocols() {
        // Two legacy capabilities: the first is kept. Ten protocols: eight
        // are recorded.
        let mut entries = vec![(0x100, 0x0000_0101), (0x104, 0x0000_0101)];
        for i in 0..10u64 {
            let at = 0x108 + 16 * i;
            let next = if i == 9 { 0 } else { 4 };
            entries.push((at, 0x0200_0002 | (next << 8)));
            entries.push((at + 8, 0x0000_0101 + (i as u32)));
        }
        let mut regs = Regs::new(&entries);
        let caps = regs.walk(0x0040_0000, 0x1_0000);
        assert_eq!(caps.legacy, Some(0x100));
        assert_eq!(caps.protocol_count, MAX_PROTOCOLS);
        assert_eq!(caps.protocols_dropped, 2);
        assert_eq!(caps.stop, None);
        // Ranges were dropped, so they are not known.
        assert!(!protocols_known(&caps));
    }

    /// Chad's re-check of a4f20464 (#93), item 1, as he gave the registers.
    /// Controller 1: HCCPARAMS1 = 0x0040_0000 (xECP = 0x40 dwords = 0x100),
    /// window 0x1000, nine Supported Protocol capabilities at 0x100 + 16*i:
    ///   i = 0..=7  header 0x0300_0402 (id 2, next 4 dwords, USB 3.0)
    ///   i = 8      header 0x0200_0002 (id 2, next 0,        USB 2.0)
    ///   DWORD2     0x0000_0100 | (i+1)  (port i+1, count 1)
    /// Eight USB 3 ranges are recorded and the USB 2 range on port 9 is not,
    /// so the ranges must read as unknown. Controller 0 is genuinely
    /// SuperSpeed-only. The choice must not be the known SuperSpeed-only
    /// controller with "no-usb2-port-anywhere"; it is controller 1, by the
    /// AMBIGUOUS path.
    #[test]
    fn a_dropped_usb2_range_makes_the_controller_ambiguous_not_superspeed_only() {
        let mut entries = Vec::new();
        for i in 0..9u64 {
            let at = 0x100 + 16 * i;
            let header = if i < 8 { 0x0300_0402 } else { 0x0200_0002 };
            entries.push((at, header));
            entries.push((at + 8, 0x0000_0100 | (i as u32 + 1)));
        }
        let mut regs = Regs::new(&entries);
        let caps = regs.walk(0x0040_0000, 0x1000);
        assert_eq!(caps.protocol_count, 8);
        assert_eq!(caps.protocols_dropped, 1);
        assert_eq!(caps.stop, None);
        assert_eq!(caps.usb2_ports(), 0);
        assert!(!protocols_known(&caps));
        let controller1 = Candidate {
            readable: true,
            protocols_known: protocols_known(&caps),
            usb2_ports: caps.usb2_ports(),
            usb2_connected: 0,
            connected: 0,
        };
        assert_eq!(
            choose(&[TCSS_EMPTY, controller1]),
            Some((1, ChoiceReason::ProtocolsUnknown))
        );
    }

    const TCSS_EMPTY: Candidate = Candidate {
        readable: true,
        protocols_known: true,
        usb2_ports: 0,
        usb2_connected: 0,
        connected: 0,
    };
    const SOUTH_EMPTY: Candidate = Candidate {
        readable: true,
        protocols_known: true,
        usb2_ports: 12,
        usb2_connected: 0,
        connected: 0,
    };

    #[test]
    fn one_controller_is_always_the_choice() {
        assert_eq!(choose(&[]), None);
        assert_eq!(choose(&[SOUTH_EMPTY]), Some((0, ChoiceReason::Only)));
        // Even unreadable: the pre-lane behaviour was to try it.
        assert_eq!(
            choose(&[Candidate::default()]),
            Some((0, ChoiceReason::Only))
        );
    }

    /// The Zenbook case: TCSS xHCI first in scan order, the south xHCI with
    /// the keyboard on a USB 2 port second.
    #[test]
    fn the_controller_with_a_usb2_device_wins_over_scan_order() {
        let south_with_keyboard = Candidate {
            usb2_connected: 1,
            connected: 1,
            ..SOUTH_EMPTY
        };
        assert_eq!(
            choose(&[TCSS_EMPTY, south_with_keyboard]),
            Some((1, ChoiceReason::Usb2Connected))
        );
    }

    #[test]
    fn with_nothing_plugged_in_the_most_usb2_ports_win() {
        assert_eq!(
            choose(&[TCSS_EMPTY, SOUTH_EMPTY]),
            Some((1, ChoiceReason::MostUsb2Ports))
        );
        // A SuperSpeed device on the TCSS controller does not pull the
        // choice there: no keyboard or mouse can appear on it.
        let tcss_with_disk = Candidate {
            connected: 1,
            ..TCSS_EMPTY
        };
        assert_eq!(
            choose(&[tcss_with_disk, SOUTH_EMPTY]),
            Some((1, ChoiceReason::MostUsb2Ports))
        );
        // Equal USB 2 port counts: the earlier controller.
        assert_eq!(
            choose(&[SOUTH_EMPTY, SOUTH_EMPTY]),
            Some((0, ChoiceReason::MostUsb2Ports))
        );
    }

    #[test]
    fn devices_on_two_controllers_pick_the_first() {
        let with = Candidate {
            usb2_connected: 2,
            connected: 2,
            ..SOUTH_EMPTY
        };
        assert_eq!(
            choose(&[with, with]),
            Some((0, ChoiceReason::Usb2Connected))
        );
    }

    #[test]
    fn unreadable_controllers_are_skipped() {
        let dead = Candidate {
            readable: false,
            protocols_known: true,
            usb2_ports: 12,
            usb2_connected: 3,
            connected: 3,
        };
        assert_eq!(
            choose(&[dead, SOUTH_EMPTY]),
            Some((1, ChoiceReason::MostUsb2Ports))
        );
        assert_eq!(
            choose(&[dead, TCSS_EMPTY]),
            Some((1, ChoiceReason::FirstReadable))
        );
        assert_eq!(
            choose(&[Candidate::default(), Candidate::default()]),
            Some((0, ChoiceReason::NoneReadable))
        );
    }

    #[test]
    fn handoff_writes() {
        // BIOS owned (bit 16) + id 1: request sets bit 24 and keeps bit 16.
        assert_eq!(legsup_request(0x0001_0001), 0x0101_0001);
        // Forcing clears bit 16.
        assert_eq!(legsup_force(0x0101_0001), 0x0100_0001);
        assert_eq!(legsup_force(0x0001_0401), 0x0100_0401);
    }

    #[test]
    fn smi_disable_clears_enables_keeps_rsvdp_and_acks_events() {
        // All five enables (0x0000_E011) and all three events (0xE000_0000)
        // set: the write is events only.
        assert_eq!(legctl_disable_smis(0xE000_E011), 0xE000_0000);
        // RsvdP 3:1 set (0xE), RO bits 16 and 20 set (0x0011_0000), enables
        // 0..4 and 13..15 set: RsvdP kept, RO and enables written 0.
        assert_eq!(legctl_disable_smis(0x0011_E01F), 0xE000_000E);
        // Every RsvdP bit set: 0x000E_1FEE kept.
        assert_eq!(legctl_disable_smis(0x000E_1FEE), 0xE00E_1FEE);
        assert_eq!(LEGCTL_RSVDP, 0x000E_1FEE);
    }

    #[test]
    fn pmcsr_d0_write() {
        assert_eq!(pmcsr_state(0x0003), 3);
        assert_eq!(pmcsr_state(0x0008), 0);
        // D3hot with PME_En (bit 8) and PME_Status (bit 15) set: the write
        // keeps PME_En, writes PME_Status 0, state 0.
        assert_eq!(pmcsr_to_d0(0x8103), 0x0100);
        // NoSoftReset (bit 3) is read-only and harmless to write back.
        assert_eq!(pmcsr_to_d0(0x000B), 0x0008);
    }

    #[test]
    fn hub_slot_context_for_a_high_speed_hub() {
        // Output slot: Context Entries 1 (1<<27), speed HS (3<<20) ->
        // 0x0830_0000; root port 2 (2<<16) -> 0x0002_0000; no TT fields.
        let out = [0x0830_0000, 0x0002_0000, 0];
        // wHubCharacteristics 0x0029: bits 6:5 = (0x29 >> 5) & 3 = 1.
        // DWORD0 |= Hub (0x0400_0000) -> 0x0C30_0000; 4 ports in DWORD1
        // 31:24 -> 0x0402_0000; TTT 1 in DWORD2 17:16 -> 0x0001_0000.
        assert_eq!(
            hub_slot_context(out, 4, true, false, 0x0029),
            [0x0C30_0000, 0x0402_0000, 0x0001_0000]
        );
        // Multi-TT adds 0x0200_0000.
        assert_eq!(hub_slot_context(out, 4, true, true, 0x0029)[0], 0x0E30_0000);
        // Interrupter target / max exit latency fields in the output are
        // kept: DWORD1 low 16 bits 0x0005 and DWORD2 bits 31:22 0xFFC0_0000.
        assert_eq!(
            hub_slot_context([0x0830_0000, 0x0002_0005, 0xFFC0_0000], 7, true, false, 0),
            [0x0C30_0000, 0x0702_0005, 0xFFC0_0000]
        );
    }

    #[test]
    fn hub_slot_context_for_a_full_speed_hub_clears_mtt_and_ttt() {
        // Output with speed FS (1<<20), Context Entries 1, MTT set
        // (0x0A10_0000) and stale TTT 3 (0x0003_0000).
        assert_eq!(
            hub_slot_context(
                [0x0A10_0000, 0x0001_0000, 0x0003_0000],
                4,
                false,
                true,
                0x0060
            ),
            [0x0C10_0000, 0x0401_0000, 0]
        );
    }

    #[test]
    fn hub_update_command_by_version() {
        assert!(!hub_needs_configure_endpoint(0x0095));
        assert!(hub_needs_configure_endpoint(0x0096));
        assert!(hub_needs_configure_endpoint(0x0100));
        assert!(hub_needs_configure_endpoint(0x0120));
    }

    #[test]
    fn tsc_budget_is_sized_for_a_10ghz_tsc() {
        // 10 ms at 10 GHz = 1e8 cycles; at a real 2 GHz TSC that is 50 ms.
        assert_eq!(tsc_cycles_for_ms(10), 100_000_000);
        assert_eq!(tsc_cycles_for_ms(0), 0);
        assert_eq!(tsc_cycles_for_ms(u64::MAX), u64::MAX);
    }

    #[test]
    fn port_service_decisions() {
        let fresh = PortTrack::default();
        let attached = PortTrack {
            attached: true,
            attempts: 0,
        };
        assert_eq!(port_action(fresh, false), PortAction::Nothing);
        assert_eq!(port_action(fresh, true), PortAction::Enumerate);
        assert_eq!(port_action(attached, true), PortAction::Nothing);
        assert_eq!(port_action(attached, false), PortAction::Detached);

        // Three failures, then the port is left alone while connected.
        let mut track = fresh;
        for expected_attempts in 1..=3u8 {
            assert_eq!(port_action(track, true), PortAction::Enumerate);
            track = port_after(track, true, PortAction::Enumerate, false);
            assert_eq!(track.attempts, expected_attempts);
        }
        assert_eq!(port_action(track, true), PortAction::GaveUp);
        assert_eq!(port_after(track, true, PortAction::GaveUp, false), track);
        // A disconnect resets it: a re-plug gets three new attempts.
        track = port_after(track, false, port_action(track, false), false);
        assert_eq!(track, fresh);
        assert_eq!(port_action(track, true), PortAction::Enumerate);

        // Success attaches; a detach forgets it.
        let ok = port_after(fresh, true, PortAction::Enumerate, true);
        assert_eq!(ok, attached);
        assert_eq!(port_after(ok, false, PortAction::Detached, false), fresh);
    }

    // --- Chad's review (#93), item 1: the protocol ranges are missing ---

    /// The south controller's Supported Protocol chain is absent or
    /// truncated, so it reports no USB 2 ports; the first controller is
    /// known SuperSpeed-only. With nothing plugged in, the unknown one is
    /// chosen, never the known SuperSpeed-only one, and the reason says the
    /// choice is ambiguous.
    #[test]
    fn unknown_protocols_beat_a_known_superspeed_only_controller() {
        let south_unknown = Candidate {
            readable: true,
            protocols_known: false,
            usb2_ports: 0,
            usb2_connected: 0,
            connected: 0,
        };
        assert_eq!(
            choose(&[TCSS_EMPTY, south_unknown]),
            Some((1, ChoiceReason::ProtocolsUnknown))
        );
        assert_eq!(
            ChoiceReason::ProtocolsUnknown.name(),
            "AMBIGUOUS-protocols-unknown"
        );
        // A known USB 2 controller still wins over an unknown one.
        assert_eq!(
            choose(&[south_unknown, SOUTH_EMPTY]),
            Some((1, ChoiceReason::MostUsb2Ports))
        );
        // All known, no USB 2 port anywhere: the first readable, named so.
        assert_eq!(
            choose(&[TCSS_EMPTY, TCSS_EMPTY]),
            Some((0, ChoiceReason::FirstReadable))
        );
        assert_eq!(ChoiceReason::FirstReadable.name(), "no-usb2-port-anywhere");
    }

    /// With the ranges missing, a connected port's speed still identifies a
    /// keyboard: the census counts it as a USB 2 device and the controller
    /// is chosen for it.
    #[test]
    fn port_speed_identifies_usb2_devices_without_protocol_ranges() {
        // Known ranges decide.
        assert!(port_holds_usb2_device(Some(2), 4));
        assert!(!port_holds_usb2_device(Some(3), 1));
        // No range: PORTSC speed IDs 1 full, 2 low, 3 high are USB 2; 4 and
        // 5 are SuperSpeed; 0 is "not yet known".
        assert!(port_holds_usb2_device(None, 1));
        assert!(port_holds_usb2_device(None, 2));
        assert!(port_holds_usb2_device(None, 3));
        assert!(!port_holds_usb2_device(None, 4));
        assert!(!port_holds_usb2_device(None, 5));
        assert!(!port_holds_usb2_device(None, 0));

        let south_unknown_with_keyboard = Candidate {
            readable: true,
            protocols_known: false,
            usb2_ports: 0,
            usb2_connected: 1,
            connected: 1,
        };
        assert_eq!(
            choose(&[TCSS_EMPTY, south_unknown_with_keyboard]),
            Some((1, ChoiceReason::Usb2Connected))
        );
    }

    #[test]
    fn protocols_known_needs_a_complete_walk_with_a_range() {
        // The south chain: complete, two ranges.
        let mut regs = south_chain();
        assert!(protocols_known(&regs.walk(0x2000_0000, 0x1_0000)));
        // Legacy capability only (0x8000: id 1, next 0): no range.
        let mut regs = Regs::new(&[(0x8000, 0x0000_0001)]);
        assert!(!protocols_known(&regs.walk(0x2000_0000, 0x1_0000)));
        // A USB 2 range, then a pointer past the window: truncated.
        let mut regs = Regs::new(&[(0x8000, 0x0200_FF02), (0x8008, 0x0000_0C01)]);
        let caps = regs.walk(0x2000_0000, 0x8100);
        assert_eq!(caps.usb2_ports(), 12);
        assert!(caps.stop.is_some());
        assert!(!protocols_known(&caps));
    }

    // --- Chad's review (#93), item 2: multi-TT hubs ---

    #[test]
    fn multi_tt_only_for_a_high_speed_protocol_2_hub() {
        // bDeviceProtocol: 0 full-speed hub, 1 HS single TT, 2 HS multi TT.
        assert!(wants_multi_tt(true, 2));
        assert!(!wants_multi_tt(true, 1));
        assert!(!wants_multi_tt(true, 0));
        assert!(!wants_multi_tt(false, 2));
        // With the hub switched, the slot context carries MTT (0x0200_0000).
        let out = [0x0830_0000, 0x0002_0000, 0];
        assert_eq!(
            hub_slot_context(out, 4, true, true, 0)[0] & SLOT_MTT,
            SLOT_MTT
        );
        assert_eq!(hub_slot_context(out, 4, true, false, 0)[0] & SLOT_MTT, 0);
    }

    // --- Chad's review (#93), item 3: hub children retry until enumerated ---

    #[test]
    fn route_subtrees() {
        // Root device (route 0) covers everything on its root port.
        assert!(route_is_under(0x0000_0000, 0));
        assert!(route_is_under(0x0000_0021, 0));
        // Hub port 1 (route 0x1): its child 0x21 (tier 2 port 2) is under it.
        assert!(route_is_under(0x0000_0001, 0x1));
        assert!(route_is_under(0x0000_0021, 0x1));
        assert!(!route_is_under(0x0000_0002, 0x1));
        assert!(!route_is_under(0x0000_0022, 0x1));
        // Two tiers.
        assert!(route_is_under(0x0000_0321, 0x21));
        assert!(!route_is_under(0x0000_0311, 0x21));
    }

    #[test]
    fn addressed_set_insert_remove_and_bound() {
        let mut set = AddressedSet::new();
        assert!(set.is_empty());
        assert!(set.insert(2, 0));
        assert!(set.insert(2, 0x1));
        assert!(set.insert(2, 0x21));
        assert!(set.insert(3, 0));
        assert!(set.insert(2, 0x1)); // already present
        assert_eq!(set.len(), 4);
        // Hub child 0x1 detaches: it and 0x21 below it go; root 2's own entry
        // and root 3 stay.
        set.remove_subtree(2, 0x1);
        assert!(set.contains(2, 0));
        assert!(!set.contains(2, 0x1));
        assert!(!set.contains(2, 0x21));
        assert!(set.contains(3, 0));
        // Root 2 detaches: everything on it goes.
        set.insert(2, 0x2);
        set.remove_subtree(2, 0);
        assert_eq!(set.len(), 1);
        // Full: the 65th insert is refused.
        let mut full = AddressedSet::new();
        for i in 0..ADDRESSED_MAX as u32 {
            assert!(full.insert(1, i + 1));
        }
        assert!(!full.insert(1, 0xFFFF));
    }

    /// The bounce's input: a child port resets and is enabled, then Enable
    /// Slot or Address Device fails. Attached comes from the addressed set,
    /// not PORT_ENABLE, so the port gets all three attempts; a success
    /// attaches it.
    #[test]
    fn an_enabled_child_whose_enumeration_fails_is_retried() {
        let set = AddressedSet::new();
        let mut attempts = 0u8;
        for expected in 1..=MAX_ATTEMPTS {
            // PORT_ENABLE is set every pass; it plays no part.
            let track = PortTrack {
                attached: set.contains(2, 0x1),
                attempts,
            };
            assert_eq!(port_action(track, true), PortAction::Enumerate);
            attempts = port_after(track, true, PortAction::Enumerate, false).attempts;
            assert_eq!(attempts, expected);
        }
        let track = PortTrack {
            attached: set.contains(2, 0x1),
            attempts,
        };
        assert_eq!(port_action(track, true), PortAction::GaveUp);

        // The success path: enumeration inserts the route; the next pass is
        // Nothing.
        let mut set = AddressedSet::new();
        set.insert(2, 0x1);
        let track = PortTrack {
            attached: set.contains(2, 0x1),
            attempts: 1,
        };
        assert_eq!(port_action(track, true), PortAction::Nothing);
        assert_eq!(port_action(track, false), PortAction::Detached);
    }
}
