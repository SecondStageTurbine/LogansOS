// SPDX-License-Identifier: MPL-2.0
//! Pure I/O APIC register encoding (i8042 lane). Host-tested through
//! `tools/xtask/src/i8042_tests.rs`.
//!
//! The I/O APIC is reached through two MMIO registers: `IOREGSEL` (base+0x00)
//! selects a register index, `IOWIN` (base+0x10) reads or writes it. Index 1
//! is the version register (bits 23:16 = the highest redirection entry). The
//! redirection entry for pin `n` is the 64-bit pair at indices `0x10 + 2n`
//! (low dword) and `0x11 + 2n` (high dword). Intel 82093AA §3.2.4:
//! - bits 7:0 vector; 10:8 delivery mode (000 fixed); 11 destination mode
//!   (0 physical); 12 delivery status (RO); 13 polarity (1 = active low);
//!   14 remote IRR (RO); 15 trigger mode (1 = level); 16 mask;
//! - bits 63:56 destination (the target local APIC ID, physical mode).

pub const IOREGSEL: usize = 0x00;
pub const IOWIN: usize = 0x10;
pub const REG_VERSION: u32 = 0x01;

pub const fn redir_low_index(pin: u32) -> u32 {
    0x10 + 2 * pin
}

pub const fn redir_high_index(pin: u32) -> u32 {
    0x11 + 2 * pin
}

/// Redirection entries the version register reports.
pub const fn entries_from_version(version: u32) -> u32 {
    ((version >> 16) & 0xff) + 1
}

/// A fixed, physical-destination redirection entry as `(low, high)` dwords.
pub const fn redirection(
    vector: u8,
    dest_apic_id: u8,
    active_low: bool,
    level: bool,
    masked: bool,
) -> (u32, u32) {
    let mut low = vector as u32;
    if active_low {
        low |= 1 << 13;
    }
    if level {
        low |= 1 << 15;
    }
    if masked {
        low |= 1 << 16;
    }
    (low, (dest_apic_id as u32) << 24)
}
