// SPDX-License-Identifier: MPL-2.0
//! The Zenbook S14 touchpad's platform quirk entry (rung 1) and its match.
//! Pure: no I/O. Host-tested by `tools/i2c-hid-logic-tests`.
//!
//! Until the AML interpreter can run the touchpad's `_CRS` and `_DSM`
//! (ACPI Fix 3), rung 1 takes the facts it needs from a table of static
//! values read out of the Zenbook's DSDT by hand. Each value names the DSDT
//! byte offset it came from (`fixtures/acpi/zenbook-s14-ux5406aa-303/dsdt.dat`,
//! BIOS UX5406AA.303). When Fix 3 lands, the AML path replaces this entry and
//! the entry stays as the cross-check: its values must equal what AML returns.
//!
//! **The match is three conditions, all required.** A machine that fails any
//! of them never has the controller touched.
//! 1. The DSDT header names the firmware: OEM ID `_ASUS_`, OEM table ID
//!    `Notebook`. That pair is shared by every ASUS notebook, so it only
//!    narrows the search.
//! 2. The live DSDT contains the touchpad's `_CRS` resource template, byte for
//!    byte: `I2cSerialBusV2` at 0x15, 400 kHz, on `\_SB.PC00.I2C5`, then
//!    `Interrupt` (level, active-low, wake) on GSI 0x69. It must sit inside the
//!    `ETPD` device. That device's `_DSM` must return the HID descriptor
//!    register as a constant for function 1, under the HID-over-I2C UUID. This
//!    condition is what makes the match specific: a firmware that does not
//!    describe exactly this device at exactly this address does not match.
//! 3. PCI 00:19.1 is `8086:E351`, the Panther Lake Serial-IO I2C controller
//!    that I2C5's `_ADR` (0x00190001) names.
//!
//! The values are matched by pattern, not by offset, so a BIOS update that
//! moves the AML still matches as long as the device description is the same.
//! The offsets are citations, checked against the fixture by the host tests.

/// A static value and where it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Citation {
    pub what: &'static str,
    pub value: u32,
    /// Byte offset of the value in the UX5406AA.303 DSDT.
    pub dsdt_offset: u32,
    /// The AML (or ASL) it was read from.
    pub source: &'static str,
}

/// One platform's touchpad description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quirk {
    pub name: &'static str,
    pub dsdt_oem_id: [u8; 6],
    pub dsdt_oem_table_id: [u8; 8],
    /// The ACPI device's NameSeg.
    pub device_name: [u8; 4],
    /// The `_CRS` resource template, as it appears in the DSDT.
    pub crs_template: &'static [u8],
    pub pci_vendor: u16,
    pub pci_device: u16,
    pub pci_bus: u8,
    pub pci_dev: u8,
    pub pci_function: u8,
    pub slave_address: u8,
    pub bus_speed_hz: u32,
    pub gsi: u32,
    pub level: bool,
    pub active_low: bool,
    pub wake: bool,
    pub hid_desc_register: u16,
    pub citations: &'static [Citation],
}

/// The HID-over-I2C `_DSM` UUID `3cdff6f7-4267-4555-ad05-b30a3d8938de`, in
/// the byte order `ToUUID` stores it (the first three fields little-endian).
pub const HID_I2C_DSM_UUID: [u8; 16] = [
    0xF7, 0xF6, 0xDF, 0x3C, 0x67, 0x42, 0x55, 0x45, 0xAD, 0x05, 0xB3, 0x0A, 0x3D, 0x89, 0x38, 0xDE,
];

/// `\_SB.PC00.I2C5.ETPD._CRS`'s template (DSDT 0x676B3..0x676DF, 44 bytes), laid out
/// from ACPI 6.5 §6.4.3.8.2 (GenericSerialBus Connection, I2C type) and
/// §6.4.3.6 (Extended Interrupt), then the end tag (§6.4.2.9).
pub const ZENBOOK_TOUCHPAD_CRS: [u8; 44] = [
    // I2cSerialBusV2: descriptor type 0x8E, length 0x001E.
    0x8E, 0x1E, 0x00, //
    0x01, // revision ID
    0x00, // resource source index
    0x01, // serial bus type: I2C
    0x02, // general flags: bit 1 consumer, bit 0 = 0 controller-initiated
    0x00, 0x00, // type-specific flags: 7-bit addressing
    0x01, // type-specific revision
    0x06, 0x00, // type data length: 6
    0x80, 0x1A, 0x06, 0x00, // connection speed 0x00061A80 = 400000 Hz
    0x15, 0x00, // slave address 0x0015
    // resource source "\_SB.PC00.I2C5" + NUL
    b'\\', b'_', b'S', b'B', b'.', b'P', b'C', b'0', b'0', b'.', b'I', b'2', b'C', b'5', 0x00,
    // Extended Interrupt: descriptor type 0x89, length 0x0006.
    0x89, 0x06, 0x00, //
    0x15, // flags: bit 0 consumer, bit 1 = 0 level, bit 2 active-low, bit 4 wake
    0x01, // interrupt table length
    0x69, 0x00, 0x00, 0x00, // GSI 0x69
    // End tag, checksum 0.
    0x79, 0x00,
];

static ZENBOOK_CITATIONS: [Citation; 9] = [
    Citation {
        what: "dsdt oem id / table id",
        value: 0,
        dsdt_offset: 0x0A,
        source: "header: OEM ID \"_ASUS_\" @0x0A, OEM table ID \"Notebook\" @0x10",
    },
    Citation {
        what: "I2C5 _ADR (PCI device 0x19, function 1)",
        value: 0x0019_0001,
        dsdt_offset: 0xFFF5,
        source: "Method (_ADR) { Return (0x00190001) }: 14 0C 5F 41 44 52 00 A4 0C [01 00 19 00]",
    },
    Citation {
        what: "ETPD device",
        value: 0,
        dsdt_offset: 0x675B7,
        source: "Device (ETPD): 5B 82 4B 12 45 54 50 44 (PkgLength 0x12B)",
    },
    Citation {
        what: "_CRS I2cSerialBusV2 slave address",
        value: 0x15,
        dsdt_offset: 0x676C3,
        source: "I2cSerialBusV2 (0x0015, ...) in the template @0x676B3",
    },
    Citation {
        what: "_CRS connection speed (Hz)",
        value: 400_000,
        dsdt_offset: 0x676BF,
        source: "I2cSerialBusV2 (..., 0x00061A80, ...)",
    },
    Citation {
        what: "_CRS controller",
        value: 0,
        dsdt_offset: 0x676C5,
        source: "ResourceSource \"\\_SB.PC00.I2C5\"",
    },
    Citation {
        what: "_CRS interrupt GSI (level, active-low, wake: flags 0x15)",
        value: 0x69,
        dsdt_offset: 0x676D9,
        source: "Interrupt (ResourceConsumer, Level, ActiveLow, ExclusiveAndWake) { 0x69 }: 89 06 00 15 01 [69 00 00 00]",
    },
    Citation {
        what: "_DSM HID-over-I2C UUID",
        value: 0,
        dsdt_offset: 0x67652,
        source: "ToUUID (\"3cdff6f7-4267-4555-ad05-b30a3d8938de\")",
    },
    Citation {
        what: "_DSM function 1: HID descriptor register",
        value: 0x0001,
        dsdt_offset: 0x6767E,
        source: "If ((Arg2 == One)) { Return (One) }: 93 6A 01 A4 [01]",
    },
];

/// `zenbook_s14_quirk`: the UX5406AA touchpad (`ASCE1206`/`ASCF1206`/
/// `ELAN1010`, picked by NVS; the descriptor register is the same constant
/// for all three, since `_DSM` function 1 returns `One` unconditionally).
pub static ZENBOOK_S14_QUIRK: Quirk = Quirk {
    name: "zenbook-s14-ux5406aa",
    dsdt_oem_id: *b"_ASUS_",
    dsdt_oem_table_id: *b"Notebook",
    device_name: *b"ETPD",
    crs_template: &ZENBOOK_TOUCHPAD_CRS,
    pci_vendor: 0x8086,
    pci_device: 0xE351,
    pci_bus: 0,
    pci_dev: 0x19,
    pci_function: 1,
    slave_address: 0x15,
    bus_speed_hz: 400_000,
    gsi: 0x69,
    level: true,
    active_low: true,
    wake: true,
    hid_desc_register: 0x0001,
    citations: &ZENBOOK_CITATIONS,
};

/// The table every lookup walks (one entry in rung 1).
pub static QUIRKS: [&Quirk; 1] = [&ZENBOOK_S14_QUIRK];

/// What the DSDT says about a quirk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DsdtFinding {
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_matches: bool,
    /// Offset of the `_CRS` template, if present.
    pub template_at: Option<usize>,
    /// Offset of the enclosing `Device (ETPD)` opcode, if the template is
    /// inside one.
    pub device_at: Option<usize>,
    /// The constant `_DSM` function 1 returns inside that device.
    pub hid_desc_register: Option<u16>,
}

/// Why a machine did or did not match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// No DSDT could be read.
    NoDsdt,
    /// The header is another firmware's.
    OemMismatch {
        oem_id: [u8; 6],
        oem_table_id: [u8; 8],
    },
    /// The firmware does not describe this touchpad.
    NoTemplate,
    /// The template is there but not inside the named device.
    TemplateOutsideDevice,
    /// `_DSM` function 1 does not return a readable constant, or returns a
    /// different one.
    DescriptorRegister(Option<u16>),
    /// Nothing at the PCI function.
    PciAbsent,
    /// Another device at the PCI function.
    PciMismatch {
        vendor: u16,
        device: u16,
    },
    Match,
}

impl Verdict {
    pub fn is_match(self) -> bool {
        self == Verdict::Match
    }
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// Decode an AML PkgLength at `at`: `(length, encoding bytes)`. The length
/// counts from the first PkgLength byte (ACPI 6.5 §20.2.4).
pub fn pkg_length(aml: &[u8], at: usize) -> Option<(usize, usize)> {
    let lead = *aml.get(at)?;
    let follow = (lead >> 6) as usize;
    if follow == 0 {
        return Some(((lead & 0x3F) as usize, 1));
    }
    let mut length = (lead & 0x0F) as usize;
    for i in 0..follow {
        length |= (*aml.get(at + 1 + i)? as usize) << (4 + 8 * i);
    }
    Some((length, 1 + follow))
}

/// Decode the AML data object at `at` as an integer: Zero, One, or a
/// Byte/Word/DWord prefix (§20.2.3). `Ones` and anything else is `None`.
pub fn aml_integer(aml: &[u8], at: usize) -> Option<u32> {
    match *aml.get(at)? {
        0x00 => Some(0),
        0x01 => Some(1),
        0x0A => Some(*aml.get(at + 1)? as u32),
        0x0B => Some(u16::from_le_bytes([*aml.get(at + 1)?, *aml.get(at + 2)?]) as u32),
        0x0C => Some(u32::from_le_bytes([
            *aml.get(at + 1)?,
            *aml.get(at + 2)?,
            *aml.get(at + 3)?,
            *aml.get(at + 4)?,
        ])),
        _ => None,
    }
}

/// `If (Arg2 == One)` then `Return (`: LEqual(Arg2, One) followed by the
/// Return opcode.
const DSM_FUNCTION_1_RETURN: [u8; 4] = [0x93, 0x6A, 0x01, 0xA4];

/// Search a whole DSDT (header included) for `quirk`'s description.
pub fn inspect_dsdt(dsdt: &[u8], quirk: &Quirk) -> DsdtFinding {
    let mut oem_id = [0u8; 6];
    let mut oem_table_id = [0u8; 8];
    if dsdt.len() >= 36 {
        oem_id.copy_from_slice(&dsdt[10..16]);
        oem_table_id.copy_from_slice(&dsdt[16..24]);
    }
    let oem_matches =
        dsdt.len() >= 36 && oem_id == quirk.dsdt_oem_id && oem_table_id == quirk.dsdt_oem_table_id;
    let mut finding = DsdtFinding {
        oem_id,
        oem_table_id,
        oem_matches,
        template_at: None,
        device_at: None,
        hid_desc_register: None,
    };
    if !oem_matches {
        return finding;
    }
    let Some(template_at) = find(dsdt, quirk.crs_template, 36) else {
        return finding;
    };
    finding.template_at = Some(template_at);
    // The nearest `Device (<name>)` before the template whose package covers
    // it: DeviceOp is 5B 82, then PkgLength, then the NameSeg.
    let mut cursor = 36;
    let mut device = None;
    while let Some(at) = find(dsdt, &[0x5B, 0x82], cursor) {
        if at >= template_at {
            break;
        }
        cursor = at + 1;
        let Some((length, enc)) = pkg_length(dsdt, at + 2) else {
            continue;
        };
        let name_at = at + 2 + enc;
        if dsdt.get(name_at..name_at + 4) != Some(&quirk.device_name[..]) {
            continue;
        }
        let end = at + 2 + length;
        if template_at + quirk.crs_template.len() <= end && end <= dsdt.len() {
            device = Some((at, end));
        }
    }
    let Some((device_at, device_end)) = device else {
        return finding;
    };
    finding.device_at = Some(device_at);
    let body = &dsdt[..device_end];
    if let Some(uuid_at) = find(body, &HID_I2C_DSM_UUID, device_at)
        && let Some(ret) = find(body, &DSM_FUNCTION_1_RETURN, uuid_at)
        && let Some(value) = aml_integer(body, ret + DSM_FUNCTION_1_RETURN.len())
        && value <= 0xFFFF
    {
        finding.hid_desc_register = Some(value as u16);
    }
    finding
}

/// The verdict from the DSDT finding and what PCI reads at the quirk's
/// function (`None`: nothing there, vendor 0xFFFF).
pub fn verdict(finding: &DsdtFinding, quirk: &Quirk, pci: Option<(u16, u16)>) -> Verdict {
    if !finding.oem_matches {
        return Verdict::OemMismatch {
            oem_id: finding.oem_id,
            oem_table_id: finding.oem_table_id,
        };
    }
    if finding.template_at.is_none() {
        return Verdict::NoTemplate;
    }
    if finding.device_at.is_none() {
        return Verdict::TemplateOutsideDevice;
    }
    if finding.hid_desc_register != Some(quirk.hid_desc_register) {
        return Verdict::DescriptorRegister(finding.hid_desc_register);
    }
    match pci {
        None => Verdict::PciAbsent,
        Some((vendor, device)) if (vendor, device) != (quirk.pci_vendor, quirk.pci_device) => {
            Verdict::PciMismatch { vendor, device }
        }
        Some(_) => Verdict::Match,
    }
}

/// The first quirk entry whose DSDT conditions hold (PCI is read after).
pub fn select(dsdt: &[u8]) -> (Option<&'static Quirk>, DsdtFinding) {
    let mut last = inspect_dsdt(dsdt, QUIRKS[0]);
    for quirk in QUIRKS {
        let finding = inspect_dsdt(dsdt, quirk);
        if finding.hid_desc_register == Some(quirk.hid_desc_register) {
            return (Some(quirk), finding);
        }
        last = finding;
    }
    (None, last)
}

/// Printable form of a fixed-width OEM field (non-printables as `.`).
pub fn printable<const N: usize>(field: &[u8; N]) -> [u8; N] {
    let mut out = [b'.'; N];
    for (o, &b) in out.iter_mut().zip(field.iter()) {
        if (0x20..0x7F).contains(&b) {
            *o = b;
        }
    }
    out
}
