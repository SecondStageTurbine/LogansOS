// SPDX-License-Identifier: MPL-2.0
//! The FADT's CMOS-clock fields for the RTC epoch seed (OT-85, `rtc-epoch`).
//! A child of `acpi` so it walks the tables with the parent's own (private)
//! RSDP/root-table helpers; read only, once, at boot.

use super::rtc_fadt_logic::{self, RtcFadt};
use super::{checksum_ok, find_table, locate_root_table, phys_slice, rd_u32};

const SIG_FADT: &[u8; 4] = b"FACP";

/// What the FADT says about the CMOS clock, or `None` when the tables are not
/// reachable (no RSDP, a bad root table, no FADT, a bad checksum).
pub fn rtc_fadt(rsdp_phys: u64) -> Option<RtcFadt> {
    if rsdp_phys == 0 {
        return None;
    }
    let (root_phys, is_xsdt) = locate_root_table(rsdp_phys)?;
    let phys = find_table(root_phys, is_xsdt, SIG_FADT)?;
    // SAFETY: an ACPI table the root table points at (HHDM-readable, read only).
    let header = unsafe { phys_slice(phys, 36) };
    let length = rd_u32(header, 4)? as usize;
    if !(36..=1 << 16).contains(&length) {
        return None;
    }
    // SAFETY: as above, for the length the header declares.
    let table = unsafe { phys_slice(phys, length) };
    if !checksum_ok(table) {
        return None;
    }
    rtc_fadt_logic::parse(table)
}
