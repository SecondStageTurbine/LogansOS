// SPDX-License-Identifier: MPL-2.0
//! i8042 lane host KAT. The kernel's own pure modules are `#[path]`-included
//! (MADT parsing, I/O APIC entry encoding, the i8042 config transform and the
//! set-1 decoder) and checked against hand-laid bytes and hand-written
//! expectations, not values derived from the code under test (the byte-oracle
//! rule).

#[path = "../../../kernel/src/acpi/madt_logic.rs"]
#[allow(dead_code)]
mod madt_logic;

#[path = "../../../kernel/src/arch/x86_64/ioapic_logic.rs"]
#[allow(dead_code)]
mod ioapic_logic;

#[path = "../../../kernel/src/drivers/ps2/i8042_logic.rs"]
#[allow(dead_code)]
mod i8042_logic;

use i8042_logic::{ByteSource, Decoder, KeyEvent};

/// A hand-laid MADT: header (36 bytes; checksum not checked by the parser),
/// local APIC address + flags, then:
/// - a type-0 local APIC entry (8 bytes, ignored by this parser);
/// - a type-1 I/O APIC: id 2, address 0xFEC0_0000, GSI base 0;
/// - a type-2 override: ISA IRQ0 → GSI 2, flags 0 (conforming);
/// - a type-2 override: ISA IRQ9 → GSI 9, flags 0x000D (level, active high);
/// - a type-2 override: ISA IRQ1 → GSI 1, flags 0x000F (level, active low);
/// - a type-4 local APIC NMI entry (6 bytes, ignored).
fn madt_fixture(with_irq1_override: bool) -> Vec<u8> {
    let mut t = Vec::new();
    t.extend_from_slice(b"APIC");
    t.extend_from_slice(&[0, 0, 0, 0]); // length, patched below
    t.extend_from_slice(&[4, 0]); // revision, checksum
    t.extend_from_slice(b"NEOTST"); // OEM ID
    t.extend_from_slice(b"I8042KAT"); // OEM table ID
    t.extend_from_slice(&[1, 0, 0, 0]); // OEM revision
    t.extend_from_slice(b"NEO "); // creator ID
    t.extend_from_slice(&[1, 0, 0, 0]); // creator revision
    assert_eq!(t.len(), 36);
    t.extend_from_slice(&[0x00, 0x00, 0xE0, 0xFE]); // LAPIC address 0xFEE0_0000
    t.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // flags: PCAT_COMPAT
    // Type 0, len 8: processor 0, APIC ID 0, enabled.
    t.extend_from_slice(&[0x00, 0x08, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
    // Type 1, len 12: id 2, reserved, 0xFEC00000, GSI base 0.
    t.extend_from_slice(&[0x01, 0x0C, 0x02, 0x00]);
    t.extend_from_slice(&[0x00, 0x00, 0xC0, 0xFE]);
    t.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    // Type 2, len 10: ISA, IRQ0 → GSI 2, flags 0.
    t.extend_from_slice(&[0x02, 0x0A, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00]);
    // Type 2, len 10: ISA, IRQ9 → GSI 9, flags 0x000D.
    t.extend_from_slice(&[0x02, 0x0A, 0x00, 0x09, 0x09, 0x00, 0x00, 0x00, 0x0D, 0x00]);
    if with_irq1_override {
        // Type 2, len 10: ISA, IRQ1 → GSI 1, flags 0x000F.
        t.extend_from_slice(&[0x02, 0x0A, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x0F, 0x00]);
    }
    // Type 4, len 6: all processors, flags 5, LINT1.
    t.extend_from_slice(&[0x04, 0x06, 0xFF, 0x05, 0x00, 0x01]);
    let len = t.len() as u32;
    t[4..8].copy_from_slice(&len.to_le_bytes());
    t
}

#[test]
fn madt_parses_ioapic_and_overrides() {
    let madt = madt_logic::parse(&madt_fixture(true)).expect("parse");
    assert_eq!(madt.ioapic_count, 1);
    assert_eq!(
        madt.ioapics[0],
        madt_logic::IoApic {
            id: 2,
            address: 0xFEC0_0000,
            gsi_base: 0
        }
    );
    assert_eq!(madt.override_count, 3);
    assert_eq!(
        madt.overrides[0],
        madt_logic::SourceOverride {
            source: 0,
            gsi: 2,
            flags: 0
        }
    );
    assert_eq!(madt.overrides[2].source, 1);
    assert_eq!(madt.overrides[2].flags, 0x000F);
}

#[test]
fn madt_rejects_wrong_signature_and_survives_truncation() {
    let mut bad = madt_fixture(false);
    bad[0..4].copy_from_slice(b"FACP");
    assert!(madt_logic::parse(&bad).is_none());
    // A zero-length entry ends the walk; the I/O APIC before it is kept.
    let mut t = madt_fixture(false);
    t.truncate(44 + 8 + 12);
    t.extend_from_slice(&[0x02, 0x00]);
    let len = t.len() as u32;
    t[4..8].copy_from_slice(&len.to_le_bytes());
    let madt = madt_logic::parse(&t).expect("parse");
    assert_eq!(madt.ioapic_count, 1);
    assert_eq!(madt.override_count, 0);
}

#[test]
fn isa_irq_routing_honours_overrides() {
    use madt_logic::{IsaRoute, route_isa_irq};
    let without = madt_logic::parse(&madt_fixture(false)).unwrap();
    // No IRQ1 override: identity, edge, active high (the ISA default).
    assert_eq!(
        route_isa_irq(&without, 1),
        IsaRoute {
            gsi: 1,
            active_low: false,
            level: false
        }
    );
    // IRQ0's conforming override moves only the GSI.
    assert_eq!(
        route_isa_irq(&without, 0),
        IsaRoute {
            gsi: 2,
            active_low: false,
            level: false
        }
    );
    // 0x000D: trigger 11 (level), polarity 01 (high).
    assert_eq!(
        route_isa_irq(&without, 9),
        IsaRoute {
            gsi: 9,
            active_low: false,
            level: true
        }
    );
    let with = madt_logic::parse(&madt_fixture(true)).unwrap();
    assert_eq!(
        route_isa_irq(&with, 1),
        IsaRoute {
            gsi: 1,
            active_low: true,
            level: true
        }
    );
    // 24 pins on the one I/O APIC: GSI 1 is its pin 1; GSI 24 is nowhere.
    assert_eq!(madt_logic::ioapic_for_gsi(&with, &[24], 1), Some((0, 1)));
    assert_eq!(madt_logic::ioapic_for_gsi(&with, &[24], 24), None);
}

#[test]
fn ioapic_redirection_entry_encoding() {
    // 82093AA: version register 0x0017_0020 = 24 entries.
    assert_eq!(ioapic_logic::entries_from_version(0x0017_0020), 24);
    assert_eq!(ioapic_logic::redir_low_index(1), 0x12);
    assert_eq!(ioapic_logic::redir_high_index(1), 0x13);
    // Vector 0xF5, fixed, physical, edge, high, unmasked, to APIC ID 0.
    assert_eq!(
        ioapic_logic::redirection(0xF5, 0, false, false, false),
        (0x0000_00F5, 0)
    );
    // Active low (bit 13), level (bit 15), masked (bit 16), to APIC ID 3.
    assert_eq!(
        ioapic_logic::redirection(0xF5, 3, true, true, true),
        (0x0001_A0F5, 0x0300_0000)
    );
}

#[test]
fn i8042_config_byte_transform() {
    // Firmware default 0x47 (IRQ1, IRQ12, system flag, translate): quiesced
    // keeps the system flag and translation, drops both IRQs; running adds
    // IRQ1 only.
    assert_eq!(i8042_logic::config_quiesced(0x47), 0x44);
    assert_eq!(i8042_logic::config_running(0x44), 0x45);
    // Port 1 clock disabled and translation off (0x30): port 1 clock on,
    // translation on, port 2 clock left as found.
    assert_eq!(i8042_logic::config_quiesced(0x30), 0x60);
}

#[test]
fn i8042_status_classification() {
    assert_eq!(i8042_logic::classify(0x01), ByteSource::Keyboard);
    assert_eq!(i8042_logic::classify(0x21), ByteSource::Aux);
    assert_eq!(i8042_logic::classify(0x41), ByteSource::Error);
    assert_eq!(i8042_logic::classify(0x81), ByteSource::Error);
}

fn decode(bytes: &[u8]) -> Vec<(u8, bool)> {
    let mut d = Decoder::new();
    bytes
        .iter()
        .filter_map(|&b| d.feed(b))
        .map(|KeyEvent { usage, pressed }| (usage, pressed))
        .collect()
}

#[test]
fn set1_decodes_to_hid_usages() {
    // Hand-written: set-1 make/break (IBM PC/AT) → HID page 0x07 usage.
    type Case = (&'static [u8], &'static [(u8, bool)]);
    let cases: &[Case] = &[
        (&[0x1E, 0x9E], &[(0x04, true), (0x04, false)]), // A
        (&[0x02, 0x82], &[(0x1E, true), (0x1E, false)]), // 1
        (&[0x0B, 0x8B], &[(0x27, true), (0x27, false)]), // 0
        (&[0x1C, 0x9C], &[(0x28, true), (0x28, false)]), // Enter
        (&[0x01, 0x81], &[(0x29, true), (0x29, false)]), // Esc
        (&[0x39, 0xB9], &[(0x2C, true), (0x2C, false)]), // Space
        (&[0x2A, 0xAA], &[(0xE1, true), (0xE1, false)]), // left Shift
        (&[0x36, 0xB6], &[(0xE5, true), (0xE5, false)]), // right Shift
        (&[0x3B, 0xBB], &[(0x3A, true), (0x3A, false)]), // F1
        (&[0x58, 0xD8], &[(0x45, true), (0x45, false)]), // F12
        (&[0x56, 0xD6], &[(0x64, true), (0x64, false)]), // non-US backslash
        (&[0x52, 0xD2], &[(0x62, true), (0x62, false)]), // keypad 0
        (&[0xE0, 0x48, 0xE0, 0xC8], &[(0x52, true), (0x52, false)]), // Up
        (&[0xE0, 0x1D, 0xE0, 0x9D], &[(0xE4, true), (0xE4, false)]), // right Ctrl
        (&[0xE0, 0x38, 0xE0, 0xB8], &[(0xE6, true), (0xE6, false)]), // right Alt
        (&[0xE0, 0x5B, 0xE0, 0xDB], &[(0xE3, true), (0xE3, false)]), // left GUI
        (&[0xE0, 0x1C, 0xE0, 0x9C], &[(0x58, true), (0x58, false)]), // keypad Enter
        (&[0xE0, 0x53, 0xE0, 0xD3], &[(0x4C, true), (0x4C, false)]), // Delete
        // Pause: one six-byte sequence, a press and a release.
        (
            &[0xE1, 0x1D, 0x45, 0xE1, 0x9D, 0xC5],
            &[(0x48, true), (0x48, false)],
        ),
        // Print Screen with its fake shift: E0 2A E0 37 / E0 B7 E0 AA.
        (
            &[0xE0, 0x2A, 0xE0, 0x37, 0xE0, 0xB7, 0xE0, 0xAA],
            &[(0x46, true), (0x46, false)],
        ),
    ];
    for (bytes, want) in cases {
        assert_eq!(decode(bytes), want.to_vec(), "bytes {bytes:02x?}");
    }
}

#[test]
fn set1_typematic_repeat_and_noise_are_not_keys() {
    // A held key's repeated makes report once, like the USB boot report.
    assert_eq!(
        decode(&[0x1E, 0x1E, 0x1E, 0x9E]),
        vec![(0x04, true), (0x04, false)]
    );
    // A break for a key never pressed, the ACK/resend/echo/error bytes, and
    // an unassigned code are dropped; decoding then resumes normally.
    assert_eq!(
        decode(&[0x9E, 0xFA, 0xFE, 0xEE, 0x00, 0xFF, 0x7F, 0x10, 0x90]),
        vec![(0x14, true), (0x14, false)]
    );
    // An E0 prefix applies to the next byte only.
    assert_eq!(
        decode(&[0xE0, 0x48, 0x48, 0xE0, 0xC8, 0xC8]),
        vec![(0x52, true), (0x60, true), (0x52, false), (0x60, false)]
    );
}

/// Key auto-repeat: the typematic makes the decoder drops are counted (the
/// on-screen `typematic=`, which shows whether the EC repeats at all), and
/// only those: not the first make, not a break, not a break for a key never
/// pressed.
#[cfg(feature = "key-autorepeat")]
#[test]
fn set1_typematic_makes_are_counted() {
    let mut d = Decoder::new();
    // Backspace (0x0E) made, repeated three times, broken; a stray break.
    let mut events = Vec::new();
    for b in [0x0E, 0x0E, 0x0E, 0x0E, 0x8E, 0x9E] {
        events.extend(d.feed(b));
    }
    assert_eq!(
        events,
        vec![
            KeyEvent {
                usage: 0x2A,
                pressed: true
            },
            KeyEvent {
                usage: 0x2A,
                pressed: false
            },
        ]
    );
    assert_eq!(d.take_typematic(), 3);
    assert_eq!(d.take_typematic(), 0);
    // An E0 key's repeat counts too: Up (E0 48) held.
    for b in [0xE0, 0x48, 0xE0, 0x48, 0xE0, 0xC8] {
        let _ = d.feed(b);
    }
    assert_eq!(d.take_typematic(), 1);
}

/// The first-route cut's index arithmetic (free-running u32 ring indices).
#[test]
fn boot_cut_skips_only_what_came_before_the_route() {
    use i8042_logic::tail_after_cut;
    // Ten boot bytes queued (tail 0, head 10); the route came up at 10.
    assert_eq!(tail_after_cut(0, 10, 10), 10);
    // Two more typed after the route (head 12): they stay.
    assert_eq!(tail_after_cut(0, 12, 10), 10);
    // Nothing queued when the route came up: nothing skipped.
    assert_eq!(tail_after_cut(5, 5, 5), 5);
    assert_eq!(tail_after_cut(5, 9, 5), 5);
    // Across the u32 wrap.
    assert_eq!(tail_after_cut(0xFFFF_FFF0, 0x10, 0x08), 0x08);
    // A cut outside tail..=head (already consumed past it) skips nothing.
    assert_eq!(tail_after_cut(20, 30, 10), 20);
}

/// The Zenbook A5 failure, replayed on the real decoder. During the boot
/// holds the user pressed Enter (`1C 9C`), then held `A`: its make and
/// typematic makes filled the ring and its break was lost. The desktop's
/// first pump used to deliver all of it, Enter included, and the decoder
/// kept `A` held, so the user's next `A` was dropped as typematic. With the
/// cut, the boot bytes are skipped, the decoder starts afresh, and the
/// user's first key reaches the focus.
#[test]
fn boot_keys_never_reach_the_first_route() {
    use i8042_logic::tail_after_cut;
    // Boot: Enter make/break, then A make and typematic makes, no break.
    let mut ring: Vec<u8> = vec![0x1C, 0x9C, 0x1E, 0x1E, 0x1E, 0x1E];
    let cut = ring.len() as u32;
    // At the desktop: the user types A (make, break).
    ring.extend_from_slice(&[0x1E, 0x9E]);
    let head = ring.len() as u32;

    // Before the fix: one decoder over the whole ring.
    let mut old = Decoder::new();
    let old_events: Vec<(u8, bool)> = ring
        .iter()
        .filter_map(|&b| old.feed(b).map(|e| (e.usage, e.pressed)))
        .collect();
    // Enter (0x28) reached the focus, and the user's A press did not: the
    // decoder still held A from the boot, so only its break came through.
    assert_eq!(
        old_events,
        vec![(0x28, true), (0x28, false), (0x04, true), (0x04, false)]
    );
    assert_eq!(old_events.iter().filter(|e| **e == (0x04, true)).count(), 1);

    // With the cut: skip to `cut`, start a fresh decoder.
    let tail = tail_after_cut(0, head, cut) as usize;
    assert_eq!(tail, 6);
    let mut fresh = Decoder::new();
    let events: Vec<(u8, bool)> = ring[tail..]
        .iter()
        .filter_map(|&b| fresh.feed(b).map(|e| (e.usage, e.pressed)))
        .collect();
    assert_eq!(
        events,
        vec![(0x04, true), (0x04, false)],
        "the user's A, at once"
    );
    assert!(!fresh.any_held());
}
