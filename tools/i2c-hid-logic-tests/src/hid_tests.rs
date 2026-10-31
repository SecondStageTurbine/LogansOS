// SPDX-License-Identifier: MPL-2.0
//! HID over I2C: the descriptor, the commands, the input framing, the report
//! descriptor walk and the mouse decode, against hand-laid bytes.

use crate::hid_logic::*;

/// A 30-byte HID descriptor laid out field by field (spec §5.1.1), every
/// field a different value so a swapped offset shows.
pub const HID_DESC: [u8; 30] = [
    0x1E, 0x00, // wHIDDescLength = 30
    0x00, 0x01, // bcdVersion = 0x0100
    0x32, 0x00, // wReportDescLength = 50
    0x02, 0x00, // wReportDescRegister = 0x0002
    0x03, 0x00, // wInputRegister = 0x0003
    0x08, 0x00, // wMaxInputLength = 8
    0x04, 0x00, // wOutputRegister = 0x0004
    0x00, 0x00, // wMaxOutputLength = 0
    0x05, 0x00, // wCommandRegister = 0x0005
    0x06, 0x00, // wDataRegister = 0x0006
    0x05, 0x0B, // wVendorID = 0x0B05
    0x34, 0x12, // wProductID = 0x1234
    0x01, 0x00, // wVersionID = 0x0001
    0x00, 0x00, 0x00, 0x00, // reserved
];

#[test]
fn hid_descriptor_fields_decode_at_their_offsets() {
    let d = HidDescriptor::decode(&HID_DESC);
    assert_eq!(d.length, 30);
    assert_eq!(d.version, 0x0100);
    assert_eq!(d.report_desc_length, 50);
    assert_eq!(d.report_desc_register, 0x0002);
    assert_eq!(d.input_register, 0x0003);
    assert_eq!(d.max_input_length, 8);
    assert_eq!(d.output_register, 0x0004);
    assert_eq!(d.max_output_length, 0);
    assert_eq!(d.command_register, 0x0005);
    assert_eq!(d.data_register, 0x0006);
    assert_eq!(d.vendor_id, 0x0B05);
    assert_eq!(d.product_id, 0x1234);
    assert_eq!(d.version_id, 0x0001);
    assert_eq!(d.validate(), Ok(()));
}

#[test]
fn hid_descriptor_validation_names_what_is_wrong() {
    // A wrong descriptor register reads someone else's bytes: the length
    // and version catch it.
    let mut b = HID_DESC;
    b[0] = 0x1F;
    assert_eq!(
        HidDescriptor::decode(&b).validate(),
        Err(DescriptorError::Length(31))
    );
    let mut b = HID_DESC;
    b[2] = 0x01;
    assert_eq!(
        HidDescriptor::decode(&b).validate(),
        Err(DescriptorError::Version(0x0101))
    );
    // All-ones (a floating bus).
    assert_eq!(
        HidDescriptor::decode(&[0xFF; 30]).validate(),
        Err(DescriptorError::Length(0xFFFF))
    );
    let mut b = HID_DESC;
    b[4] = 0;
    assert_eq!(
        HidDescriptor::decode(&b).validate(),
        Err(DescriptorError::ReportDescLength(0))
    );
    let mut b = HID_DESC;
    b[10] = 2; // too short to hold a length prefix and a report
    assert_eq!(
        HidDescriptor::decode(&b).validate(),
        Err(DescriptorError::MaxInput(2))
    );
}

#[test]
fn commands_are_register_then_low_byte_then_opcode() {
    // Spec §7.2.1 RESET: opcode 1 in the high byte, low byte 0.
    assert_eq!(reset_command(0x0005), [0x05, 0x00, 0x00, 0x01]);
    // §7.2.8 SET_POWER: opcode 8, power state ON = 0, SLEEP = 1.
    assert_eq!(set_power_command(0x0005, true), [0x05, 0x00, 0x00, 0x08]);
    assert_eq!(set_power_command(0x0005, false), [0x05, 0x00, 0x01, 0x08]);
    // A register above 0xFF stays little-endian.
    assert_eq!(reset_command(0x0122), [0x22, 0x01, 0x00, 0x01]);
    // The Zenbook's descriptor register 0x0001 as the read prefix.
    assert_eq!(register(0x0001), [0x01, 0x00]);
}

#[test]
fn input_framing() {
    // The zero-length reset ack (spec §7.2.1: the device returns a length
    // of 0 on the input register once reset completes).
    assert_eq!(parse_input(&[0x00, 0x00, 0xAA, 0xBB]), Input::Empty);
    // ELAN's bogus interrupt: length 0xFFFF.
    assert_eq!(parse_input(&[0xFF, 0xFF, 0x00]), Input::Bogus);
    // A length of 6 (it counts its own two bytes): ID 1 and three bytes.
    assert_eq!(
        parse_input(&[0x06, 0x00, 0x01, 0x01, 0x05, 0xFB, 0xEE, 0xEE]),
        Input::Report(&[0x01, 0x01, 0x05, 0xFB])
    );
    // Shorter than its own prefix, or longer than the read.
    assert_eq!(parse_input(&[0x01, 0x00, 0x00]), Input::BadLength(1));
    assert_eq!(parse_input(&[0x09, 0x00, 0x00, 0x00]), Input::BadLength(9));
    assert_eq!(parse_input(&[0x00]), Input::BadLength(0));
}

/// HID 1.11 Appendix E.10's boot mouse report descriptor, item by item.
pub const BOOT_MOUSE_DESC: [u8; 50] = [
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x02, // Usage (Mouse)
    0xA1, 0x01, // Collection (Application)
    0x09, 0x01, //   Usage (Pointer)
    0xA1, 0x00, //   Collection (Physical)
    0x05, 0x09, //     Usage Page (Button)
    0x19, 0x01, //     Usage Minimum (1)
    0x29, 0x03, //     Usage Maximum (3)
    0x15, 0x00, //     Logical Minimum (0)
    0x25, 0x01, //     Logical Maximum (1)
    0x95, 0x03, //     Report Count (3)
    0x75, 0x01, //     Report Size (1)
    0x81, 0x02, //     Input (Data, Variable, Absolute): bits 0..3
    0x95, 0x01, //     Report Count (1)
    0x75, 0x05, //     Report Size (5)
    0x81, 0x01, //     Input (Constant): bits 3..8, padding
    0x05, 0x01, //     Usage Page (Generic Desktop)
    0x09, 0x30, //     Usage (X)
    0x09, 0x31, //     Usage (Y)
    0x15, 0x81, //     Logical Minimum (-127)
    0x25, 0x7F, //     Logical Maximum (127)
    0x75, 0x08, //     Report Size (8)
    0x95, 0x02, //     Report Count (2)
    0x81, 0x06, //     Input (Data, Variable, Relative): X bits 8..16, Y 16..24
    0xC0, //   End Collection
    0xC0, // End Collection
];

#[test]
fn boot_mouse_descriptor_layout() {
    let map = parse_report_descriptor(&BOOT_MOUSE_DESC).unwrap();
    assert!(!map.uses_ids);
    let m = map.mouse.unwrap();
    assert_eq!(m.report_id, 0);
    assert_eq!(m.button_count, 3);
    assert_eq!(
        m.buttons,
        Field {
            bit: 0,
            size: 1,
            signed: false
        }
    );
    assert_eq!(
        m.x,
        Field {
            bit: 8,
            size: 8,
            signed: true
        }
    );
    assert_eq!(
        m.y,
        Field {
            bit: 16,
            size: 8,
            signed: true
        }
    );
    assert_eq!(m.wheel, None);
    assert_eq!(m.report_bits, 24);
    // 26 items: count the item lines above.
    assert_eq!(map.items, 26);
    // Left + right held, X +5, Y -5 (0xFB).
    assert_eq!(
        decode(&map, &[0b0000_0011, 0x05, 0xFB]),
        Decoded::Mouse(MouseReport {
            buttons: 0b011,
            dx: 5,
            dy: -5,
            wheel: 0
        })
    );
    // The padding bits (3..8) never become buttons.
    assert_eq!(
        decode(&map, &[0xF8, 0x00, 0x00]),
        Decoded::Mouse(MouseReport::default())
    );
    assert_eq!(decode(&map, &[0x01, 0x05]), Decoded::Short);
}

/// A precision-touchpad-shaped composite: a mouse collection with report ID
/// 1 (two buttons, 12-bit X/Y, a wheel), a Digitizer / Touch Pad collection
/// with report ID 3, and a feature report (the Input Mode, ID 4).
pub const PTP_COMPOSITE_DESC: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x02, // Usage (Mouse)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x01, //   Report ID (1)
    0x09, 0x01, //   Usage (Pointer)
    0xA1, 0x00, //   Collection (Physical)
    0x05, 0x09, //     Usage Page (Button)
    0x19, 0x01, //     Usage Minimum (1)
    0x29, 0x02, //     Usage Maximum (2)
    0x15, 0x00, //     Logical Minimum (0)
    0x25, 0x01, //     Logical Maximum (1)
    0x75, 0x01, //     Report Size (1)
    0x95, 0x02, //     Report Count (2)
    0x81, 0x02, //     Input (Data, Var, Abs): bits 0..2
    0x95, 0x06, //     Report Count (6)
    0x81, 0x03, //     Input (Constant): bits 2..8
    0x05, 0x01, //     Usage Page (Generic Desktop)
    0x09, 0x30, //     Usage (X)
    0x09, 0x31, //     Usage (Y)
    0x16, 0x01, 0xF8, // Logical Minimum (-2047), 2-byte item
    0x26, 0xFF, 0x07, // Logical Maximum (2047)
    0x75, 0x0C, //     Report Size (12)
    0x95, 0x02, //     Report Count (2)
    0x81, 0x06, //     Input (Data, Var, Rel): X bits 8..20, Y 20..32
    0x09, 0x38, //     Usage (Wheel)
    0x15, 0x81, //     Logical Minimum (-127)
    0x25, 0x7F, //     Logical Maximum (127)
    0x75, 0x08, //     Report Size (8)
    0x95, 0x01, //     Report Count (1)
    0x81, 0x06, //     Input (Data, Var, Rel): wheel bits 32..40
    0xC0, //   End Collection
    0xC0, // End Collection
    0x05, 0x0D, // Usage Page (Digitizer)
    0x09, 0x05, // Usage (Touch Pad)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x03, //   Report ID (3)
    0x09, 0x42, //   Usage (Tip Switch)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x02, //   Input (Data, Var, Abs)
    0x95, 0x07, //   Report Count (7)
    0x81, 0x03, //   Input (Constant)
    0x85, 0x04, //   Report ID (4)
    0x09, 0x52, //   Usage (Input Mode)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x01, //   Report Count (1)
    0xB1, 0x02, //   Feature (Data, Var, Abs)
    0xC0, // End Collection
];

#[test]
fn composite_descriptor_finds_the_mouse_and_names_the_digitizer() {
    let map = parse_report_descriptor(PTP_COMPOSITE_DESC).unwrap();
    assert!(map.uses_ids);
    let m = map.mouse.unwrap();
    assert_eq!(m.report_id, 1);
    assert_eq!(m.button_count, 2);
    assert_eq!(m.buttons.bit, 0);
    assert_eq!(
        m.x,
        Field {
            bit: 8,
            size: 12,
            signed: true
        }
    );
    assert_eq!(
        m.y,
        Field {
            bit: 20,
            size: 12,
            signed: true
        }
    );
    assert_eq!(
        m.wheel,
        Some(Field {
            bit: 32,
            size: 8,
            signed: true
        })
    );
    assert_eq!(m.report_bits, 40);
    // Input IDs only (the feature report 4 is not an input).
    assert_eq!(map.id_count, 2);
    assert_eq!(map.kind_of(1), Some(ReportKind::Mouse));
    assert_eq!(map.kind_of(3), Some(ReportKind::Digitizer));
    assert_eq!(map.kind_of(4), None);

    // ID 1, right button, X = +3 (0x003), Y = -2 (0xFFE), wheel -1:
    // payload bits 8..20 = 0x003, 20..32 = 0xFFE:
    // byte1 = 0x03, byte2 = 0xE0 (low nibble X bits 8..11 = 0, high nibble
    // Y bits 0..3 = 0xE), byte3 = 0xFF (Y bits 4..11), byte4 = 0xFF.
    assert_eq!(
        decode(&map, &[0x01, 0b10, 0x03, 0xE0, 0xFF, 0xFF]),
        Decoded::Mouse(MouseReport {
            buttons: 0b10,
            dx: 3,
            dy: -2,
            wheel: -1
        })
    );
    // A large X clamps to the pointer's i8 range: X = +2047 (0x7FF):
    // byte1 = 0xFF, byte2 low nibble = 0x7.
    assert_eq!(
        decode(&map, &[0x01, 0x00, 0xFF, 0x07, 0x00, 0x00]),
        Decoded::Mouse(MouseReport {
            buttons: 0,
            dx: 127,
            dy: 0,
            wheel: 0
        })
    );
    // A PTP report while in mouse mode is counted, not misparsed (trap 10).
    assert_eq!(
        decode(&map, &[0x03, 0x01]),
        Decoded::Unexpected(3, ReportKind::Digitizer)
    );
    assert_eq!(decode(&map, &[0x09, 0x00]), Decoded::UnknownId(9));
    assert_eq!(decode(&map, &[0x01, 0x00, 0x00]), Decoded::Short);
}

#[test]
fn descriptors_the_walk_refuses() {
    // No mouse collection at all (just the digitizer half).
    let digitizer = PTP_COMPOSITE_DESC
        .windows(2)
        .position(|w| w == [0x05, 0x0D])
        .unwrap();
    assert_eq!(
        parse_report_descriptor(&PTP_COMPOSITE_DESC[digitizer..]),
        Err(ParseError::NoMouse)
    );
    // Truncated item data.
    assert_eq!(
        parse_report_descriptor(&[0x05, 0x01, 0x16, 0x01]),
        Err(ParseError::Truncated)
    );
    // A long item.
    assert_eq!(
        parse_report_descriptor(&[0xFE, 0x00, 0x00]),
        Err(ParseError::Unsupported(0xFE))
    );
    // Push.
    assert_eq!(
        parse_report_descriptor(&[0xA4]),
        Err(ParseError::Unsupported(0xA4))
    );
    // Unbalanced collections.
    assert_eq!(
        parse_report_descriptor(&[0xC0]),
        Err(ParseError::Collections)
    );
    assert_eq!(
        parse_report_descriptor(&BOOT_MOUSE_DESC[..49]),
        Err(ParseError::Collections)
    );
    // All-ones (a floating bus): 0xFF is a 4-byte long-item-free short item
    // with tag 0xFC, then truncation.
    assert!(parse_report_descriptor(&[0xFF; 7]).is_err());
}

#[test]
fn field_extraction_is_little_endian_bit_order() {
    // A 4-bit signed field at bit 6 spanning two bytes: value -3 = 0b1101.
    // bits 6,7 of byte 0 = 0b01 (low two bits), bits 0,1 of byte 1 = 0b11.
    let f = Field {
        bit: 6,
        size: 4,
        signed: true,
    };
    assert_eq!(extract(&[0b0100_0000, 0b0000_0011], f), Some(-3));
    let u = Field { signed: false, ..f };
    assert_eq!(extract(&[0b0100_0000, 0b0000_0011], u), Some(13));
    assert_eq!(extract(&[0xFF], f), None);
}

#[test]
fn line_governor_masks_a_storm_and_stops_on_errors() {
    let mut g = LineGovernor::default();
    // Fifteen empty assertions keep the line live; data resets the count.
    for _ in 0..STORM_LIMIT - 1 {
        assert_eq!(g.after_irq(Outcome::NoData), Action::Continue);
    }
    assert_eq!(g.after_irq(Outcome::Data), Action::Continue);
    assert_eq!(g.spurious, 0);
    // Sixteen in a row: stay masked and poll.
    for _ in 0..STORM_LIMIT - 1 {
        assert_eq!(g.after_irq(Outcome::NoData), Action::Continue);
    }
    assert_eq!(g.after_irq(Outcome::NoData), Action::MaskAndPoll);
    assert_eq!(STORM_LIMIT, 16);

    // Eight failed transfers in a row stop the reader; one success resets.
    let mut g = LineGovernor::default();
    for _ in 0..ERROR_LIMIT - 1 {
        assert_eq!(g.after_irq(Outcome::Error), Action::Continue);
    }
    assert_eq!(g.after_irq(Outcome::Data), Action::Continue);
    for _ in 0..ERROR_LIMIT - 1 {
        assert_eq!(g.after_irq(Outcome::Error), Action::Continue);
    }
    assert_eq!(g.after_irq(Outcome::Error), Action::Stop);
    assert_eq!(ERROR_LIMIT, 8);

    // Polled: no data is normal and never masks; errors still stop.
    let mut g = LineGovernor::default();
    for _ in 0..100 {
        assert_eq!(g.after_poll(Outcome::NoData), Action::Continue);
    }
    for _ in 0..ERROR_LIMIT - 1 {
        assert_eq!(g.after_poll(Outcome::Error), Action::Continue);
    }
    assert_eq!(g.after_poll(Outcome::Error), Action::Stop);
}

/// A clickpad press arrives with no motion: the boot mouse with only the
/// left button down and X = Y = 0 still decodes the button (A5: the pointer
/// moved but clicks did not land, so the button path is pinned here).
#[test]
fn a_press_without_motion_decodes_the_button() {
    let map = parse_report_descriptor(&BOOT_MOUSE_DESC).unwrap();
    assert_eq!(
        decode(&map, &[0b0000_0001, 0x00, 0x00]),
        Decoded::Mouse(MouseReport {
            buttons: 0b001,
            dx: 0,
            dy: 0,
            wheel: 0
        })
    );
    // And its release.
    assert_eq!(
        decode(&map, &[0x00, 0x00, 0x00]),
        Decoded::Mouse(MouseReport::default())
    );
}

/// Buttons named by 4-byte extended usages (page in the upper 16 bits)
/// while the global Usage Page is still Generic Desktop: Usage Minimum
/// `1B 01 00 09 00` = Button 1, Usage Maximum `2B 02 00 09 00` = Button 2.
/// Laid out by hand: report ID 2, buttons at bits 0..2, 6 bits padding, X
/// at 8..16, Y at 16..24.
pub const EXTENDED_USAGE_BUTTONS_DESC: [u8; 52] = [
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x02, // Usage (Mouse)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x02, //   Report ID (2)
    0x09, 0x01, //   Usage (Pointer)
    0xA1, 0x00, //   Collection (Physical)
    0x1B, 0x01, 0x00, 0x09, 0x00, // Usage Minimum (Button 1, extended)
    0x2B, 0x02, 0x00, 0x09, 0x00, // Usage Maximum (Button 2, extended)
    0x15, 0x00, //     Logical Minimum (0)
    0x25, 0x01, //     Logical Maximum (1)
    0x95, 0x02, //     Report Count (2)
    0x75, 0x01, //     Report Size (1)
    0x81, 0x02, //     Input (Data, Variable, Absolute): bits 0..2
    0x95, 0x06, //     Report Count (6)
    0x81, 0x03, //     Input (Constant): bits 2..8
    0x09, 0x30, //     Usage (X)
    0x09, 0x31, //     Usage (Y)
    0x15, 0x81, //     Logical Minimum (-127)
    0x25, 0x7F, //     Logical Maximum (127)
    0x75, 0x08, //     Report Size (8)
    0x95, 0x02, //     Report Count (2)
    0x81, 0x06, //     Input (Data, Variable, Relative): X 8..16, Y 16..24
    0xC0, //   End Collection
    0xC0, // End Collection
];

#[test]
fn extended_usage_buttons_are_buttons() {
    let map = parse_report_descriptor(&EXTENDED_USAGE_BUTTONS_DESC).unwrap();
    assert!(map.uses_ids);
    let m = map.mouse.unwrap();
    assert_eq!(m.report_id, 2);
    assert_eq!(m.button_count, 2);
    assert_eq!(m.buttons.bit, 0);
    assert_eq!(m.x.bit, 8);
    assert_eq!(m.y.bit, 16);
    // ID 2, right button down, no motion.
    assert_eq!(
        decode(&map, &[0x02, 0b10, 0x00, 0x00]),
        Decoded::Mouse(MouseReport {
            buttons: 0b10,
            dx: 0,
            dy: 0,
            wheel: 0
        })
    );
}
