// SPDX-License-Identifier: MPL-2.0
//! Byte oracle for OT-87 R4.4's installer ABI (`SYS_INSTALL_LIST`'s record,
//! `SYS_INSTALL_RUN`'s status; ADR-028's rule: hand-laid bytes, not a round
//! trip). The kernel encoder (`kernel/src/install_ui_logic.rs`) and libneo's
//! decoder (`userland/libneo/src/install_abi.rs`) are each checked against
//! the same literal byte arrays, written out by hand from the layout table;
//! then the codes both sides name, the syscall numbers in both sources, and
//! the console's numbering and text.

#![allow(dead_code)]

#[path = "../../../kernel/src/install_ui_logic.rs"]
mod kernel;

#[path = "../../../userland/libneo/src/install_abi.rs"]
mod libneo;

#[cfg(test)]
mod tests {
    use super::{kernel, libneo};

    /// Slot 2, a blank 2 GiB disk at S=512, generation 3, offered "Install";
    /// it identifies itself, serial `LOGANSOS-TARGET`, no model.
    const OFFERED: [u8; 128] = [
        0x01, // version 1
        0x00, // class blank
        0x01, // action Install
        0x00, // refusal none
        0x01, // flags: identity
        0x00, // model length 0
        0x0f, // serial length 15
        0x00, // zero
        0x02, 0x00, 0x00, 0x00, // slot 2
        0x00, 0x02, 0x00, 0x00, // sector 512
        0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // generation 3
        0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, // capacity 2 GiB
        // model[48]: empty
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        // serial[32]: "LOGANSOS-TARGET"
        b'L', b'O', b'G', b'A', b'N', b'S', b'O', b'S', b'-', b'T', b'A', b'R', b'G', b'E', b'T',
        0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        // zero[16]
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    /// Slot 0, a 4 GiB disk at S=4096 holding something else, generation 1,
    /// refused as the boot disk; it doesn't identify itself, model `QEMU`.
    const REFUSED: [u8; 128] = [
        0x01, // version 1
        0x03, // class foreign
        0x00, // action none
        0x01, // refusal: the boot disk
        0x00, // flags: no identity
        0x04, // model length 4
        0x00, // serial length 0
        0x00, // zero
        0x00, 0x00, 0x00, 0x00, // slot 0
        0x00, 0x10, 0x00, 0x00, // sector 4096
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // generation 1
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // capacity 4 GiB
        // model[48]: "QEMU"
        b'Q', b'E', b'M', b'U', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        // serial[32]: empty
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        // zero[16]
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    fn offered_row() -> kernel::Row<'static> {
        kernel::Row {
            slot: 2,
            generation: 3,
            capacity: 2 << 30,
            sector: 512,
            class: kernel::class::BLANK,
            action: kernel::action::INSTALL,
            refusal: kernel::refusal::NONE,
            identity: true,
            model: b"",
            serial: b"LOGANSOS-TARGET",
        }
    }

    fn refused_row() -> kernel::Row<'static> {
        kernel::Row {
            slot: 0,
            generation: 1,
            capacity: 4 << 30,
            sector: 4096,
            class: kernel::class::FOREIGN,
            action: kernel::action::NONE,
            refusal: kernel::refusal::BOOT_DISK,
            identity: false,
            model: b"QEMU",
            serial: b"",
        }
    }

    #[test]
    fn kernel_encodes_the_hand_laid_records() {
        assert_eq!(kernel::encode(&offered_row()), OFFERED);
        assert_eq!(kernel::encode(&refused_row()), REFUSED);
    }

    #[test]
    fn libneo_decodes_the_hand_laid_records() {
        let r = libneo::Row::decode(&OFFERED).expect("v1 record");
        assert_eq!(
            (r.slot, r.generation, r.capacity, r.sector),
            (2, 3, 2 << 30, 512)
        );
        assert_eq!((r.class, r.action, r.refusal), (0, 1, 0));
        assert!(r.identity);
        assert_eq!(r.model(), b"");
        assert_eq!(r.serial(), b"LOGANSOS-TARGET");
        assert!(r.offered());

        let r = libneo::Row::decode(&REFUSED).expect("v1 record");
        assert_eq!(
            (r.slot, r.generation, r.capacity, r.sector),
            (0, 1, 4 << 30, 4096)
        );
        assert_eq!((r.class, r.action, r.refusal), (3, 0, 1));
        assert!(!r.identity);
        assert_eq!(r.model(), b"QEMU");
        assert_eq!(r.serial(), b"");
        assert!(!r.offered());
    }

    #[test]
    fn libneo_refuses_records_it_does_not_understand() {
        let mut bad = OFFERED;
        bad[0] = 2;
        assert_eq!(libneo::Row::decode(&bad), None, "another version");
        let mut bad = OFFERED;
        bad[7] = 1;
        assert_eq!(libneo::Row::decode(&bad), None, "byte 7 nonzero");
        let mut bad = OFFERED;
        bad[127] = 1;
        assert_eq!(libneo::Row::decode(&bad), None, "the tail nonzero");
        let mut bad = OFFERED;
        bad[5] = 49;
        assert_eq!(libneo::Row::decode(&bad), None, "model length past 48");
        let mut bad = OFFERED;
        bad[6] = 33;
        assert_eq!(libneo::Row::decode(&bad), None, "serial length past 32");
        assert_eq!(libneo::Row::decode(&OFFERED[..127]), None, "short");
        // #148 F2: codes and flag bits this version doesn't name.
        let mut bad = OFFERED;
        bad[1] = 5;
        assert_eq!(libneo::Row::decode(&bad), None, "class past UNREADABLE");
        let mut bad = OFFERED;
        bad[2] = 3;
        assert_eq!(libneo::Row::decode(&bad), None, "action past FINISH");
        let mut bad = REFUSED;
        bad[3] = 14;
        assert_eq!(libneo::Row::decode(&bad), None, "refusal past OTHER");
        for bit in 1..8 {
            let mut bad = OFFERED;
            bad[4] |= 1 << bit;
            assert_eq!(libneo::Row::decode(&bad), None, "flag bit {bit}");
        }
        let mut bad = OFFERED;
        bad[3] = 1;
        assert_eq!(libneo::Row::decode(&bad), None, "offered and refused");
        let mut bad = REFUSED;
        bad[3] = 0;
        assert_eq!(libneo::Row::decode(&bad), None, "neither");
        // The review's record, by hand: version 1, action 0xff, every other
        // byte zero (class blank, refusal none, no flags, empty fields).
        let mut chad = [0u8; 128];
        chad[0] = 0x01;
        chad[2] = 0xff;
        assert_eq!(libneo::Row::decode(&chad), None, "action 0xff");
        // And byte 4 = 0x80 on an otherwise valid offered record.
        let mut flag = OFFERED;
        flag[4] = 0x80;
        assert_eq!(libneo::Row::decode(&flag), None, "flags 0x80");
        // The edge codes still decode.
        let mut ok = REFUSED;
        ok[1] = libneo::class::UNREADABLE;
        ok[3] = libneo::refusal::OTHER;
        assert!(libneo::Row::decode(&ok).is_some());
        let mut ok = OFFERED;
        ok[1] = libneo::class::INCOMPLETE;
        ok[2] = libneo::action::FINISH;
        assert!(libneo::Row::decode(&ok).expect("finish").offered());
    }

    #[test]
    fn a_long_model_and_serial_are_cut_to_their_fields() {
        let model = [b'm'; 60];
        let serial = [b's'; 40];
        let row = kernel::Row {
            model: &model,
            serial: &serial,
            ..offered_row()
        };
        let rec = kernel::encode(&row);
        assert_eq!((rec[5], rec[6]), (48, 32));
        assert!(rec[32..80].iter().all(|&b| b == b'm'));
        assert!(rec[80..112].iter().all(|&b| b == b's'));
        assert!(rec[112..].iter().all(|&b| b == 0));
        let r = libneo::Row::decode(&rec).expect("v1 record");
        assert_eq!(r.model(), &model[..48]);
        assert_eq!(r.serial(), &serial[..32]);
    }

    #[test]
    fn both_sides_name_the_same_codes() {
        assert_eq!(kernel::ROW_BYTES, libneo::ROW_BYTES);
        assert_eq!(kernel::ROW_VERSION, libneo::ROW_VERSION);
        assert_eq!(kernel::MODEL_BYTES, libneo::MODEL_BYTES);
        assert_eq!(kernel::SERIAL_BYTES, libneo::SERIAL_BYTES);
        assert_eq!(kernel::FLAG_IDENTITY, libneo::FLAG_IDENTITY);
        assert_eq!(kernel::STATUS_INSTALLED, libneo::STATUS_INSTALLED);
        assert_eq!(kernel::STATUS_STOPPED, libneo::STATUS_STOPPED);
        for (k, l) in [
            (kernel::class::BLANK, libneo::class::BLANK),
            (kernel::class::INCOMPLETE, libneo::class::INCOMPLETE),
            (kernel::class::INSTALLED, libneo::class::INSTALLED),
            (kernel::class::FOREIGN, libneo::class::FOREIGN),
            (kernel::class::UNREADABLE, libneo::class::UNREADABLE),
            (kernel::action::NONE, libneo::action::NONE),
            (kernel::action::INSTALL, libneo::action::INSTALL),
            (kernel::action::FINISH, libneo::action::FINISH),
            (kernel::refusal::NONE, libneo::refusal::NONE),
            (kernel::refusal::BOOT_DISK, libneo::refusal::BOOT_DISK),
            (kernel::refusal::PROTECTED, libneo::refusal::PROTECTED),
            (kernel::refusal::TOO_SMALL, libneo::refusal::TOO_SMALL),
            (kernel::refusal::SECTOR_SIZE, libneo::refusal::SECTOR_SIZE),
            (kernel::refusal::NO_FLUSH, libneo::refusal::NO_FLUSH),
            (kernel::refusal::INSTALLED, libneo::refusal::INSTALLED),
            (kernel::refusal::FOREIGN, libneo::refusal::FOREIGN),
            (kernel::refusal::DEVICE_DEAD, libneo::refusal::DEVICE_DEAD),
            (kernel::refusal::IN_USE, libneo::refusal::IN_USE),
            (kernel::refusal::CHANGED, libneo::refusal::CHANGED),
            (kernel::refusal::MEDIUM, libneo::refusal::MEDIUM),
            (kernel::refusal::NO_IDENTITY, libneo::refusal::NO_IDENTITY),
            (kernel::refusal::OTHER, libneo::refusal::OTHER),
        ] {
            assert_eq!(k, l);
        }
    }

    #[test]
    fn both_sources_name_the_same_syscall_numbers() {
        let kernel_src = include_str!("../../../kernel/src/arch/x86_64/syscall.rs");
        assert!(kernel_src.contains("pub const SYS_INSTALL_LIST: u64 = 0x1C0;"));
        assert!(kernel_src.contains("pub const SYS_INSTALL_RUN: u64 = 0x1C1;"));
        assert_eq!(libneo::SYS_INSTALL_LIST, 0x1C0);
        assert_eq!(libneo::SYS_INSTALL_RUN, 0x1C1);
    }

    fn row(action: u8, refusal: u8) -> libneo::Row {
        let rec = kernel::encode(&kernel::Row {
            action,
            refusal,
            ..offered_row()
        });
        libneo::Row::decode(&rec).unwrap()
    }

    #[test]
    fn only_offered_rows_are_numbered_or_chosen() {
        use libneo::{Choice, action, choose, numbers, refusal};
        let rows = [
            row(action::NONE, refusal::BOOT_DISK),
            row(action::INSTALL, refusal::NONE),
            row(action::NONE, refusal::TOO_SMALL),
            row(action::FINISH, refusal::NONE),
        ];
        // Never sent: a record with both doesn't decode (#148 F2), so it
        // can't be numbered or chosen.
        let both = kernel::encode(&kernel::Row {
            action: action::INSTALL,
            refusal: refusal::IN_USE,
            ..offered_row()
        });
        assert_eq!(libneo::Row::decode(&both), None);
        assert_eq!(numbers(&rows), [None, Some(1), None, Some(2)]);
        assert_eq!(choose(&rows, "1"), Choice::Row(1));
        assert_eq!(choose(&rows, " 2 "), Choice::Row(3));
        assert_eq!(choose(&rows, ""), Choice::Back);
        for typed in ["0", "3", "9", "-1", "x", "1x", "99999999999999999999"] {
            assert_eq!(choose(&rows, typed), Choice::Invalid, "{typed:?}");
        }
    }

    #[test]
    fn the_console_text() {
        assert_eq!(libneo::size_text(2 << 30), "2.0 GiB");
        assert_eq!(libneo::size_text((2 << 30) - 512), "1.9 GiB");
        assert_eq!(libneo::size_text(512 << 20), "512 MiB");
        let offered = libneo::Row::decode(&OFFERED).unwrap();
        assert_eq!(libneo::name_text(&offered), "LOGANSOS-TARGET");
        assert_eq!(
            libneo::detail_text(&offered),
            "2.0 GiB, 512-byte sectors, blank"
        );
        assert_eq!(libneo::verdict_text(&offered), "Install");
        let refused = libneo::Row::decode(&REFUSED).unwrap();
        assert_eq!(libneo::name_text(&refused), "QEMU (identity unavailable)");
        assert_eq!(
            libneo::verdict_text(&refused),
            "not available: this is the disk LogansOS is running from"
        );
        assert_eq!(
            libneo::status_text(0),
            "LogansOS is installed. Remove the USB and restart to boot from the disk."
        );
        assert!(libneo::status_text(0x104).contains("stopped at step W4"));
        assert!(libneo::status_text(3).contains("too small: needs 2 GiB"));
        assert!(libneo::status_text(-1).contains("error -1"));
        // Every refusal code has its own reason.
        let mut seen = std::collections::BTreeSet::new();
        for code in 1..=13u8 {
            assert!(seen.insert(libneo::refusal_text(code)), "code {code}");
        }
    }
}
