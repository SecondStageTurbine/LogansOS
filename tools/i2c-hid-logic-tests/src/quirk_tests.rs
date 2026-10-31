// SPDX-License-Identifier: MPL-2.0
//! The quirk entry and its match: hand-laid AML, then the rung-0 corpus.

use std::path::{Path, PathBuf};

use crate::quirk_logic::*;

#[test]
fn the_crs_template_says_what_the_quirk_says() {
    let t = &ZENBOOK_TOUCHPAD_CRS;
    let q = &ZENBOOK_S14_QUIRK;
    // I2cSerialBusV2: type 0x8E, length 0x001E (30 bytes after the header).
    assert_eq!(t[0], 0x8E);
    assert_eq!(u16::from_le_bytes([t[1], t[2]]), 0x1E);
    assert_eq!(3 + 0x1E, 33);
    assert_eq!(t[5], 0x01, "serial bus type I2C");
    assert_eq!(t[7..9], [0, 0], "7-bit addressing");
    let speed = u32::from_le_bytes([t[12], t[13], t[14], t[15]]);
    assert_eq!(speed, q.bus_speed_hz);
    assert_eq!(speed, 400_000);
    assert_eq!(u16::from_le_bytes([t[16], t[17]]), q.slave_address as u16);
    assert_eq!(&t[18..33], b"\\_SB.PC00.I2C5\0");
    // Extended Interrupt at byte 33: flags 0x15 = consumer | active-low |
    // wake, bit 1 (edge) clear = level.
    assert_eq!(t[33], 0x89);
    let flags = t[36];
    assert_eq!(flags & 1, 1);
    assert_eq!(flags >> 1 & 1 == 0, q.level);
    assert_eq!(flags >> 2 & 1 == 1, q.active_low);
    assert_eq!(flags >> 4 & 1 == 1, q.wake);
    assert_eq!(t[37], 1, "one interrupt");
    assert_eq!(u32::from_le_bytes([t[38], t[39], t[40], t[41]]), q.gsi);
    assert_eq!(q.gsi, 105);
    assert_eq!(t[42..], [0x79, 0x00]);
    // I2C5's _ADR 0x00190001 = device 0x19, function 1.
    assert_eq!((q.pci_dev, q.pci_function), (0x19, 1));
    assert_eq!((q.pci_vendor, q.pci_device), (0x8086, 0xE351));
    assert_eq!(q.hid_desc_register, 0x0001);
}

#[test]
fn pkg_length_and_integers() {
    // One byte: bits 5:0.
    assert_eq!(pkg_length(&[0x3F], 0), Some((0x3F, 1)));
    // ETPD's 4B 12: two bytes, (0x4B & 0x0F) | 0x12 << 4 = 0x12B.
    assert_eq!(pkg_length(&[0x4B, 0x12], 0), Some((0x12B, 2)));
    // Three bytes: 0x8A 0x34 0x12 = 0xA | 0x34 << 4 | 0x12 << 12 = 0x1234A.
    assert_eq!(pkg_length(&[0x8A, 0x34, 0x12], 0), Some((0x1234A, 3)));
    assert_eq!(pkg_length(&[0x4B], 0), None);
    assert_eq!(aml_integer(&[0x00], 0), Some(0));
    assert_eq!(aml_integer(&[0x01], 0), Some(1));
    assert_eq!(aml_integer(&[0x0A, 0x20], 0), Some(0x20));
    assert_eq!(aml_integer(&[0x0B, 0x22, 0x01], 0), Some(0x122));
    assert_eq!(aml_integer(&[0x0C, 1, 0, 0x19, 0], 0), Some(0x0019_0001));
    // Ones, and a name (a DerefOf, a method call) are not constants.
    assert_eq!(aml_integer(&[0xFF], 0), None);
    assert_eq!(aml_integer(&[0x83], 0), None);
}

/// A minimal DSDT around the touchpad: header, `Device (ETPD)` holding a
/// `_DSM` whose function 1 returns `ret`, then `_CRS`'s template.
fn synthetic_dsdt(oem: &[u8; 6], table: &[u8; 8], ret: &[u8], crs_in_device: bool) -> Vec<u8> {
    let mut body = Vec::new();
    // The UUID buffer, then If (Arg2 == One) { Return (ret) }.
    body.extend_from_slice(&[0x11, 0x13, 0x0A, 0x10]); // Buffer (16)
    body.extend_from_slice(&HID_I2C_DSM_UUID);
    body.extend_from_slice(&[0xA0, 0x06, 0x93, 0x6A, 0x01, 0xA4]);
    body.extend_from_slice(ret);
    let mut crs = vec![0x08, b'S', b'B', b'F', b'I', 0x11, 0x2F, 0x0A, 0x2C];
    crs.extend_from_slice(&ZENBOOK_TOUCHPAD_CRS);
    if crs_in_device {
        body.extend_from_slice(&crs);
    }
    // Device (ETPD): 5B 82, a two-byte PkgLength, the NameSeg, the body.
    let length = 2 + 4 + body.len();
    let mut dev = vec![
        0x5B,
        0x82,
        0x40 | (length & 0x0F) as u8,
        (length >> 4) as u8,
    ];
    dev.extend_from_slice(b"ETPD");
    dev.extend_from_slice(&body);
    let mut t = vec![0u8; 36];
    t[0..4].copy_from_slice(b"DSDT");
    t[10..16].copy_from_slice(oem);
    t[16..24].copy_from_slice(table);
    t.extend_from_slice(&dev);
    if !crs_in_device {
        t.extend_from_slice(&crs);
    }
    let len = t.len() as u32;
    t[4..8].copy_from_slice(&len.to_le_bytes());
    t
}

#[test]
fn synthetic_dsdts() {
    let q = &ZENBOOK_S14_QUIRK;
    let pci = Some((0x8086, 0xE351));
    let good = synthetic_dsdt(b"_ASUS_", b"Notebook", &[0x01], true);
    let f = inspect_dsdt(&good, q);
    assert_eq!(f.device_at, Some(36));
    assert_eq!(f.hid_desc_register, Some(1));
    assert_eq!(verdict(&f, q, pci), Verdict::Match);
    assert_eq!(verdict(&f, q, None), Verdict::PciAbsent);
    // The touchscreen's controller (E350) at the touchpad's function.
    assert_eq!(
        verdict(&f, q, Some((0x8086, 0xE350))),
        Verdict::PciMismatch {
            vendor: 0x8086,
            device: 0xE350
        }
    );
    assert!(select(&good).0.is_some());

    // Another ASUS firmware whose touchpad's _DSM says 0x20.
    let other_reg = synthetic_dsdt(b"_ASUS_", b"Notebook", &[0x0A, 0x20], true);
    let f = inspect_dsdt(&other_reg, q);
    assert_eq!(verdict(&f, q, pci), Verdict::DescriptorRegister(Some(0x20)));
    assert!(select(&other_reg).0.is_none());
    // Function 1 computed at run time (the touchscreen's DerefOf).
    let computed = synthetic_dsdt(b"_ASUS_", b"Notebook", &[0x83, 0x88], true);
    assert_eq!(
        verdict(&inspect_dsdt(&computed, q), q, pci),
        Verdict::DescriptorRegister(None)
    );
    // The template, but outside the device.
    let outside = synthetic_dsdt(b"_ASUS_", b"Notebook", &[0x01], false);
    assert_eq!(
        verdict(&inspect_dsdt(&outside, q), q, pci),
        Verdict::TemplateOutsideDevice
    );
    // Another vendor's firmware with the same bytes.
    let lenovo = synthetic_dsdt(b"LENOVO", b"TP-R2X  ", &[0x01], true);
    assert!(matches!(
        verdict(&inspect_dsdt(&lenovo, q), q, pci),
        Verdict::OemMismatch { .. }
    ));
    // The ASUS header with no touchpad at all.
    let mut bare = vec![0u8; 36];
    bare[10..16].copy_from_slice(b"_ASUS_");
    bare[16..24].copy_from_slice(b"Notebook");
    assert_eq!(
        verdict(&inspect_dsdt(&bare, q), q, pci),
        Verdict::NoTemplate
    );
    // Too short to have a header.
    assert!(matches!(
        verdict(&inspect_dsdt(&[0u8; 10], q), q, pci),
        Verdict::OemMismatch { .. }
    ));
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/acpi")
}

/// Every set in the corpus with a DSDT: (name, bytes).
fn corpus() -> Option<Vec<(String, Vec<u8>)>> {
    let dir = corpus_dir();
    if !dir.is_dir() {
        return None;
    }
    let mut sets: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| {
            let e = e.ok()?;
            let dsdt = std::fs::read(e.path().join("dsdt.dat")).ok()?;
            Some((e.file_name().to_string_lossy().into_owned(), dsdt))
        })
        .collect();
    sets.sort();
    Some(sets)
}

#[test]
fn the_zenbook_corpus_dsdt_matches_and_every_citation_holds() {
    // The corpus (PR #96) is in the tree: a missing one is a failure. The
    // Zenbook set is a real-machine set, kept in the private repo only, so a
    // public checkout skips this test.
    let sets = corpus().expect("fixtures/acpi (the rung-0 ACPI corpus) is in the tree");
    let Some((_, dsdt)) = sets.iter().find(|(n, _)| n == "zenbook-s14-ux5406aa-303") else {
        eprintln!("SKIP: the Zenbook set (a real-machine set) is absent");
        return;
    };
    let q = &ZENBOOK_S14_QUIRK;
    let (selected, f) = select(dsdt);
    assert_eq!(selected.map(|s| s.name), Some(q.name));
    assert!(f.oem_matches);
    assert_eq!(f.template_at, Some(0x676B3));
    assert_eq!(f.device_at, Some(0x675B7));
    assert_eq!(f.hid_desc_register, Some(0x0001));
    assert_eq!(verdict(&f, q, Some((0x8086, 0xE351))), Verdict::Match);

    // Each citation's bytes, read straight out of the fixture.
    let at = |c: &str| {
        q.citations
            .iter()
            .find(|x| x.what.starts_with(c))
            .unwrap_or_else(|| panic!("citation {c}"))
            .dsdt_offset as usize
    };
    let u16_at = |o: usize| u16::from_le_bytes([dsdt[o], dsdt[o + 1]]) as u32;
    let u32_at = |o: usize| u32::from_le_bytes([dsdt[o], dsdt[o + 1], dsdt[o + 2], dsdt[o + 3]]);
    assert_eq!(&dsdt[at("dsdt oem")..at("dsdt oem") + 6], b"_ASUS_");
    assert_eq!(&dsdt[0x10..0x18], b"Notebook");
    // `_ADR` returns DWordConst 0x00190001: the 0x0C prefix before it, and
    // the Return (A4) before that.
    let adr = at("I2C5 _ADR");
    assert_eq!(dsdt[adr - 2..adr], [0xA4, 0x0C]);
    assert_eq!(u32_at(adr), 0x0019_0001);
    assert_eq!(&dsdt[adr - 7..adr - 3], b"_ADR");
    let dev = at("ETPD device");
    assert_eq!(
        dsdt[dev..dev + 8],
        [0x5B, 0x82, 0x4B, 0x12, b'E', b'T', b'P', b'D']
    );
    assert_eq!(u16_at(at("_CRS I2cSerialBusV2 slave")), 0x15);
    assert_eq!(u32_at(at("_CRS connection speed")), 400_000);
    assert_eq!(
        &dsdt[at("_CRS controller")..at("_CRS controller") + 14],
        b"\\_SB.PC00.I2C5"
    );
    let gsi = at("_CRS interrupt GSI");
    assert_eq!(dsdt[gsi - 5..gsi], [0x89, 0x06, 0x00, 0x15, 0x01]);
    assert_eq!(u32_at(gsi), 0x69);
    let uuid = at("_DSM HID-over-I2C UUID");
    assert_eq!(dsdt[uuid..uuid + 16], HID_I2C_DSM_UUID);
    let reg = at("_DSM function 1");
    assert_eq!(dsdt[reg - 4..reg], [0x93, 0x6A, 0x01, 0xA4]);
    assert_eq!(dsdt[reg], 0x01, "Return (One)");
    // Every citation with a value names bytes that hold it.
    for c in q.citations {
        assert!((c.dsdt_offset as usize) < dsdt.len(), "{}", c.what);
    }
}

#[test]
fn no_other_corpus_machine_matches() {
    // The corpus (PR #96) is in the tree: a missing one is a failure. The
    // real-machine sets are kept in the private repo only; without them the
    // three QEMU sets are checked.
    let sets = corpus().expect("fixtures/acpi (the rung-0 ACPI corpus) is in the tree");
    let others: Vec<_> = sets
        .iter()
        .filter(|(n, _)| !n.starts_with("zenbook-s14"))
        .collect();
    let qemu = others
        .iter()
        .filter(|(n, _)| n.starts_with("qemu-"))
        .count();
    assert!(qemu >= 3, "QEMU sets missing: {qemu}");
    if others.len() == qemu {
        eprintln!("note: real-machine corpus sets absent; checking the QEMU sets only");
    } else {
        assert!(others.len() >= 5, "corpus too small: {}", others.len());
    }
    for (name, dsdt) in others {
        let (selected, f) = select(dsdt);
        assert!(selected.is_none(), "{name} matched");
        // Even with the right PCI device present, the verdict is no.
        assert!(
            !verdict(&f, &ZENBOOK_S14_QUIRK, Some((0x8086, 0xE351))).is_match(),
            "{name}"
        );
        assert_eq!(f.template_at, None, "{name} has the touchpad template");
    }
}
