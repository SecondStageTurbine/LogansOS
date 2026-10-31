// SPDX-License-Identifier: MPL-2.0
//! Pure parsing of what the FADT says about the CMOS clock (OT-85, feature
//! `rtc-epoch`). Shared with the host KATs in `tools/rtc-logic-tests`.
//!
//! Source, read for principle (ACPI 6.5 §5.2.9, Table 5.9 "FADT Format", and
//! Table 5.10 "IA-PC Boot Architecture Flags"):
//! - `CENTURY` (1 byte at offset 108): the CMOS RAM index of the century byte;
//!   0 means the platform has none. It lies in the fixed-hardware range
//!   (offsets 46-108) that a hardware-reduced platform's OSPM must ignore.
//! - `IAPC_BOOT_ARCH` (2 bytes at 109, FADT revision 2 or later; reserved in
//!   the ACPI 1.0 table): bit 5 "CMOS RTC Not Present" (ACPI 5.0+). Set, the
//!   ports 0x70/0x71 do not reach a clock, and reading them means nothing.
//! - Flags (4 bytes at 112): bit 20 `HW_REDUCED_ACPI`.

/// FADT flags: hardware-reduced ACPI.
pub const HW_REDUCED_ACPI: u32 = 1 << 20;
/// IAPC_BOOT_ARCH: the CMOS RTC is not present.
pub const CMOS_RTC_NOT_PRESENT: u16 = 1 << 5;

/// The FADT's word on the CMOS clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RtcFadt {
    /// The century byte's CMOS index, when the FADT names one (and is not
    /// hardware-reduced).
    pub century_index: Option<u8>,
    /// IAPC_BOOT_ARCH bit 5 is set: there is no CMOS RTC.
    pub rtc_not_present: bool,
}

/// Read the FADT bytes `fadt` (signature included). `None` when it is not a
/// FADT or too short to hold `CENTURY`.
pub fn parse(fadt: &[u8]) -> Option<RtcFadt> {
    if fadt.get(..4)? != b"FACP" {
        return None;
    }
    let revision = *fadt.get(8)?;
    let century = *fadt.get(108)?;
    let flags = fadt
        .get(112..116)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let reduced = flags & HW_REDUCED_ACPI != 0;
    let boot_arch = match fadt.get(109..111) {
        Some(b) if revision >= 2 => u16::from_le_bytes([b[0], b[1]]),
        _ => 0,
    };
    Some(RtcFadt {
        century_index: (century != 0 && !reduced).then_some(century),
        rtc_not_present: boot_arch & CMOS_RTC_NOT_PRESENT != 0,
    })
}
