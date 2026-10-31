// SPDX-License-Identifier: MPL-2.0
//! Pure local-APIC encodings shared by the kernel (`apic.rs`, `drivers::pci`)
//! and the host KATs in `tools/apic-logic-tests` (mounted there by `#[path]`,
//! so the tests run this exact code). No hardware access here.
//!
//! Sources, read for principle (Intel SDM Vol. 3A, ch. 11 "x2APIC"):
//! - In x2APIC mode each 16-byte-aligned xAPIC MMIO register at offset `o`
//!   becomes the 32-bit MSR `0x800 + (o >> 4)`.
//! - The x2APIC ICR is ONE 64-bit MSR (`0x830`): destination in bits 63:32,
//!   the xAPIC ICR-low layout in bits 31:0. The xAPIC ICR is two 32-bit MMIO
//!   registers, destination in ICR-high bits 31:24 (8 bits).
//! - An MSI address is `0xFEEx_xxxx`; without interrupt remapping the
//!   destination APIC ID is bits 19:12 (8 bits), physical destination mode,
//!   no redirection hint. An APIC ID above 255 cannot be reached that way.

/// First x2APIC MSR (the xAPIC register at offset 0).
pub const X2APIC_MSR_BASE: u32 = 0x800;

/// The x2APIC MSR for the xAPIC MMIO register at `offset`.
pub const fn x2apic_msr(offset: u64) -> u32 {
    X2APIC_MSR_BASE + (offset >> 4) as u32
}

/// Largest APIC ID the xAPIC ICR's 8-bit destination can name.
pub const XAPIC_MAX_DEST: u32 = 0xff;

/// xAPIC ICR as (ICR-high, ICR-low): an 8-bit physical destination, or
/// `None` for an ID above 255. Masking it would deliver to another CPU.
pub const fn xapic_icr(dest: u32, low: u32) -> Option<(u32, u32)> {
    if dest > XAPIC_MAX_DEST {
        None
    } else {
        Some((dest << 24, low))
    }
}

/// Whether an IPI can name APIC ID `id` in `mode`.
pub const fn ipi_addressable(mode: ApicMode, id: u32) -> bool {
    match mode {
        ApicMode::X2Apic => true,
        ApicMode::XApic => id <= XAPIC_MAX_DEST,
    }
}

/// x2APIC ICR: one 64-bit value, the full 32-bit destination in bits 63:32.
pub const fn x2apic_icr(dest: u32, low: u32) -> u64 {
    ((dest as u64) << 32) | low as u64
}

/// The MSI/MSI-X address base (`0xFEE0_0000`).
pub const MSI_ADDRESS_BASE: u32 = 0xfee0_0000;

/// Largest APIC ID an MSI address can name without interrupt remapping.
pub const MSI_MAX_DEST: u32 = 0xff;

/// The MSI address targeting APIC ID `dest` (physical mode, no redirection
/// hint), or `None` when `dest` does not fit the 8-bit field. A device must
/// then not be programmed at all: truncating would deliver to another CPU.
pub const fn msi_address(dest: u32) -> Option<u32> {
    if dest > MSI_MAX_DEST {
        None
    } else {
        Some(MSI_ADDRESS_BASE | (dest << 12))
    }
}

/// Which register interface the local APICs are in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ApicMode {
    XApic,
    X2Apic,
}

impl ApicMode {
    pub const fn name(self) -> &'static str {
        match self {
            ApicMode::XApic => "xAPIC",
            ApicMode::X2Apic => "x2APIC",
        }
    }
}

/// The mode to drive, from what the bootloader reports (`reported_x2apic`,
/// Limine's MP response flag bit 0) and what the CPU's `IA32_APIC_BASE` says
/// (`msr_x2apic`, bit 10). The hardware state wins: an xAPIC MMIO access in
/// x2APIC mode is ignored and an x2APIC MSR access in xAPIC mode faults, so a
/// wrong guess is never harmless. Returns the mode and whether the two
/// disagreed (logged by the caller).
pub const fn resolve_mode(reported_x2apic: bool, msr_x2apic: bool) -> (ApicMode, bool) {
    let mode = if msr_x2apic {
        ApicMode::X2Apic
    } else {
        ApicMode::XApic
    };
    (mode, reported_x2apic != msr_x2apic)
}

/// What a CPU's local-APIC init does, from the mode the kernel drives (the
/// BSP's) and that CPU's own `IA32_APIC_BASE` enable bits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ApInit {
    /// Initialize in the BSP's mode.
    Proceed,
    /// The CPU is in a mode the kernel cannot move it out of legally: halt
    /// it with a named line, before it signals ready or joins shootdowns.
    HaltModeMismatch,
}

/// The per-CPU mode check (CP-68, Jarvis P2). SDM transitions: x2APIC to
/// xAPIC directly is illegal (#GP; only through the disabled state), and the
/// kernel never rewrites `IA32_APIC_BASE` in x2APIC mode. So:
/// - x2APIC kernel: the CPU must already be enabled in x2APIC mode;
/// - xAPIC kernel: the CPU must NOT be in x2APIC mode (a globally disabled
///   or xAPIC-mode APIC is enabled in xAPIC mode, a legal transition).
pub const fn ap_mode_check(mode: ApicMode, lapic_enable: bool, x2apic_enable: bool) -> ApInit {
    match mode {
        ApicMode::X2Apic if lapic_enable && x2apic_enable => ApInit::Proceed,
        ApicMode::X2Apic => ApInit::HaltModeMismatch,
        ApicMode::XApic if x2apic_enable => ApInit::HaltModeMismatch,
        ApicMode::XApic => ApInit::Proceed,
    }
}
