// SPDX-License-Identifier: MPL-2.0
//! Pure MADT parsing for legacy-IRQ routing (i8042 lane): the I/O APICs and
//! the ISA interrupt source overrides. No I/O; host-tested through
//! `tools/xtask/src/i8042_tests.rs`.
//!
//! Layout (ACPI 6.5 §5.2.12): the 36-byte SDT header, the local APIC address
//! (u32 @36) and flags (u32 @40), then variable-length entries from offset 44,
//! each `type u8, length u8, body`.
//! - Type 1, I/O APIC (length 12): `id u8 @2`, reserved `@3`, `address u32 @4`,
//!   `gsi_base u32 @8`.
//! - Type 2, Interrupt Source Override (length 10): `bus u8 @2` (0 = ISA),
//!   `source u8 @3` (the ISA IRQ), `gsi u32 @4`, `flags u16 @8` (MPS INTI
//!   flags: polarity bits 1:0, trigger mode bits 3:2).

/// At most this many I/O APICs are recorded (client parts have one).
pub const MAX_IOAPICS: usize = 4;
/// At most this many overrides are recorded (firmware lists a handful).
pub const MAX_OVERRIDES: usize = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IoApic {
    pub id: u8,
    pub address: u32,
    pub gsi_base: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceOverride {
    pub source: u8,
    pub gsi: u32,
    pub flags: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Madt {
    pub ioapics: [IoApic; MAX_IOAPICS],
    pub ioapic_count: usize,
    pub overrides: [SourceOverride; MAX_OVERRIDES],
    pub override_count: usize,
}

/// Electrical shape of an interrupt line as the I/O APIC needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsaRoute {
    pub gsi: u32,
    pub active_low: bool,
    pub level: bool,
}

fn rd_u16(b: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(off..off + 2)?.try_into().ok()?))
}

fn rd_u32(b: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(off..off + 4)?.try_into().ok()?))
}

/// Parse a whole MADT (signature and checksum already checked by the caller).
/// Truncated or zero-length entries end the walk; what was read is kept.
pub fn parse(table: &[u8]) -> Option<Madt> {
    if table.get(0..4)? != b"APIC" {
        return None;
    }
    let length = (rd_u32(table, 4)? as usize).min(table.len());
    let mut madt = Madt::default();
    let mut off = 44;
    while off + 2 <= length {
        let kind = table[off];
        let len = table[off + 1] as usize;
        if len < 2 || off + len > length {
            break;
        }
        let entry = &table[off..off + len];
        match kind {
            1 if len >= 12 && madt.ioapic_count < MAX_IOAPICS => {
                madt.ioapics[madt.ioapic_count] = IoApic {
                    id: entry[2],
                    address: rd_u32(entry, 4)?,
                    gsi_base: rd_u32(entry, 8)?,
                };
                madt.ioapic_count += 1;
            }
            // Bus 0 is ISA; nothing else is defined.
            2 if len >= 10 && entry[2] == 0 && madt.override_count < MAX_OVERRIDES => {
                madt.overrides[madt.override_count] = SourceOverride {
                    source: entry[3],
                    gsi: rd_u32(entry, 4)?,
                    flags: rd_u16(entry, 8)?,
                };
                madt.override_count += 1;
            }
            _ => {}
        }
        off += len;
    }
    Some(madt)
}

/// Where ISA `irq` arrives: identity-mapped, edge-triggered and active-high
/// unless an override says otherwise. "Conforms to the bus" (00) means the ISA
/// default for each field; the reserved encoding (10) is read as the default.
pub fn route_isa_irq(madt: &Madt, irq: u8) -> IsaRoute {
    let mut route = IsaRoute {
        gsi: irq as u32,
        active_low: false,
        level: false,
    };
    if let Some(o) = madt.overrides[..madt.override_count]
        .iter()
        .find(|o| o.source == irq)
    {
        route.gsi = o.gsi;
        route.active_low = o.flags & 0b11 == 0b11;
        route.level = (o.flags >> 2) & 0b11 == 0b11;
    }
    route
}

/// The I/O APIC that serves `gsi`, given each one's number of redirection
/// entries (`entries[i]` for `madt.ioapics[i]`), and the pin on it.
pub fn ioapic_for_gsi(madt: &Madt, entries: &[u32], gsi: u32) -> Option<(usize, u32)> {
    (0..madt.ioapic_count).find_map(|i| {
        let base = madt.ioapics[i].gsi_base;
        let n = *entries.get(i)?;
        (gsi >= base && gsi < base + n).then_some((i, gsi - base))
    })
}
