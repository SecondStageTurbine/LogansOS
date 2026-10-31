// SPDX-License-Identifier: MPL-2.0
//! Device probes: named namespace paths whose methods a later lane depends
//! on, evaluated beyond `_STA`/`_INI`. A set lists them in `probes.json`:
//!
//! ```json
//! {"why": "...", "probes": [{"path": "\\_SB_.PC00.I2C5.ETPD", "label": "touchpad",
//!   "methods": ["_HID", "_STA", "_CRS", "_DSM(hid-i2c,1,1)"]}]}
//! ```
//!
//! A `_CRS` result is also fed to the crate's own resource decoder. Results
//! are what the interpreter computes under the scripted zero model: a method
//! that branches on firmware NVS state takes the "all zero" branch.

use serde_json::Value;

/// HID-over-I2C `_DSM` UUID 3CDFF6F7-4267-4555-AD05-B30A3D8938DE in its AML
/// `ToUUID` byte order. Taken from `iasl`'s own encoding of the ToUUID
/// string, and present verbatim in the Zenbook S14 DSDT (test below).
pub const HID_I2C_UUID: [u8; 16] = [
    0xf7, 0xf6, 0xdf, 0x3c, 0x67, 0x42, 0x55, 0x45, 0xad, 0x05, 0xb3, 0x0a, 0x3d, 0x89, 0x38, 0xde,
];

#[derive(Clone, Debug)]
pub struct Probe {
    pub path: String,
    pub label: String,
    pub methods: Vec<String>,
}

/// One method call: the method name and, for `_DSM`, (revision, function).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call<'a> {
    Plain(&'a str),
    DsmHidI2c { rev: u64, func: u64 },
}

pub fn parse_call(m: &str) -> Option<Call<'_>> {
    if let Some(rest) = m.strip_prefix("_DSM(hid-i2c,") {
        let (rev, func) = rest.strip_suffix(')')?.split_once(',')?;
        return Some(Call::DsmHidI2c {
            rev: rev.trim().parse().ok()?,
            func: func.trim().parse().ok()?,
        });
    }
    (m.len() == 4 && m.starts_with('_')).then_some(Call::Plain(m))
}

pub fn parse_probes(v: &Value) -> Vec<Probe> {
    v["probes"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    Some(Probe {
                        path: p["path"].as_str()?.to_string(),
                        label: p["label"].as_str().unwrap_or("").to_string(),
                        methods: p["methods"]
                            .as_array()?
                            .iter()
                            .filter_map(|m| m.as_str().map(str::to_string))
                            .collect(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Decode a compressed EISA ID (`EisaId("PNP0C09")` = 0x090CD041) as `_HID`
/// integers carry it: three 5-bit letters, then four hex digits.
pub fn eisa_id(v: u64) -> String {
    let b = (v as u32).to_le_bytes();
    let c = u16::from_be_bytes([b[0], b[1]]);
    let letters: String = [10u32, 5, 0]
        .iter()
        .map(|sh| char::from(((c >> sh) & 31) as u8 + 0x40))
        .collect();
    format!("{letters}{:02X}{:02X}", b[2], b[3])
}

/// Shorten a rendered value for the report.
pub fn clip(s: String, n: usize) -> String {
    if s.chars().count() <= n {
        s
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dsm_call_parses() {
        assert_eq!(
            parse_call("_DSM(hid-i2c,1,1)"),
            Some(Call::DsmHidI2c { rev: 1, func: 1 })
        );
        assert_eq!(parse_call("_CRS"), Some(Call::Plain("_CRS")));
        assert_eq!(parse_call("CRS"), None);
    }

    /// PNP0C09 is the ACPI spec's EC ID; ATK3001 is the value the Zenbook S14
    /// DSDT declares for PS2K as `EisaId ("ATK3001")` (iasl disassembly).
    #[test]
    fn eisa_ids_decode() {
        assert_eq!(eisa_id(0x090C_D041), "PNP0C09");
        assert_eq!(eisa_id(19_958_534), "ATK3001");
    }

    /// The UUID bytes are an exact-byte oracle: they must be the bytes the
    /// Zenbook S14's own HID-I2C `_DSM` compares Arg0 against.
    #[test]
    fn hid_i2c_uuid_is_the_one_the_zenbook_firmware_uses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/acpi/zenbook-s14-ux5406aa-303/dsdt.dat");
        if !path.exists() {
            // Real-machine sets are kept in the private repo only.
            eprintln!("SKIP: real-machine fixture {} absent", path.display());
            return;
        }
        let dsdt = std::fs::read(&path).unwrap();
        assert!(dsdt.windows(16).any(|w| w == HID_I2C_UUID));
    }
}
