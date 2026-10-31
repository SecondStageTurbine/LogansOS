// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/arch/x86_64/apic_logic.rs` (CP-68, x2APIC).
//!
//! The real module is mounted through `#[path]`, so these tests exercise the
//! exact code the kernel runs. Every expected value is written out by hand
//! from the Intel SDM Vol. 3A x2APIC register map (table "Local APIC Register
//! Address Map Supported by x2APIC"), not computed from the code under test.

#![allow(dead_code)]

#[path = "../../../kernel/src/arch/x86_64/apic_logic.rs"]
mod apic_logic;

#[cfg(test)]
mod tests {
    use super::apic_logic::{
        ApInit, ApicMode, MSI_ADDRESS_BASE, ap_mode_check, ipi_addressable, msi_address,
        resolve_mode, x2apic_icr, x2apic_msr, xapic_icr,
    };

    /// The registers the kernel touches, xAPIC MMIO offset → x2APIC MSR,
    /// as listed in the SDM's map.
    #[test]
    fn msr_map_matches_the_sdm_table() {
        assert_eq!(x2apic_msr(0x0b0), 0x80b, "EOI");
        assert_eq!(x2apic_msr(0x0f0), 0x80f, "SVR");
        assert_eq!(x2apic_msr(0x300), 0x830, "ICR (one 64-bit MSR)");
        assert_eq!(x2apic_msr(0x320), 0x832, "LVT timer");
        assert_eq!(x2apic_msr(0x380), 0x838, "timer initial count");
        assert_eq!(x2apic_msr(0x3e0), 0x83e, "timer divide configuration");
        assert_eq!(x2apic_msr(0x020), 0x802, "APIC ID");
    }

    /// The xAPIC ICR encoding is the pre-CP-68 one, byte for byte: the
    /// destination in ICR-high bits 31:24, ICR-low unchanged. This is the
    /// kill-switch equality for the IPI path.
    #[test]
    fn xapic_icr_is_the_pre_x2apic_encoding() {
        let low = (1 << 14) | 0xfd; // level assert | RESCHEDULE_IPI_VECTOR
        for dest in [0u32, 1, 3, 0x7f, 0xff] {
            let pre = ((dest & 0xff) << 24, low);
            assert_eq!(xapic_icr(dest, low), Some(pre));
        }
        assert_eq!(xapic_icr(3, 0x40fd), Some((0x0300_0000, 0x0000_40fd)));
    }

    /// Jarvis note 3: an ID above 255 is refused in xAPIC mode, never masked
    /// (0x103 & 0xff would be CPU 3).
    #[test]
    fn xapic_refuses_ids_above_255() {
        assert_eq!(xapic_icr(0x100, 0x40fd), None);
        assert_eq!(xapic_icr(0x103, 0x40fd), None);
        assert_eq!(xapic_icr(u32::MAX, 0x40fd), None);
        assert!(ipi_addressable(ApicMode::XApic, 0xff));
        assert!(!ipi_addressable(ApicMode::XApic, 0x100));
        assert!(ipi_addressable(ApicMode::X2Apic, 0x100));
        assert!(ipi_addressable(ApicMode::X2Apic, 0xffff_fffe));
    }

    /// Jarvis P2: every (kernel mode, CPU's own x2APIC bit) combination, with
    /// the CPU's APIC enabled, plus a globally disabled APIC.
    #[test]
    fn ap_mode_check_all_combinations() {
        // (kernel mode, LAPIC_ENABLE, X2APIC_ENABLE) -> outcome
        assert_eq!(ap_mode_check(ApicMode::X2Apic, true, true), ApInit::Proceed);
        assert_eq!(
            ap_mode_check(ApicMode::X2Apic, true, false),
            ApInit::HaltModeMismatch,
            "xAPIC CPU under an x2APIC kernel"
        );
        assert_eq!(
            ap_mode_check(ApicMode::XApic, true, true),
            ApInit::HaltModeMismatch,
            "x2APIC CPU under an xAPIC kernel: x2APIC -> xAPIC directly is #GP"
        );
        assert_eq!(ap_mode_check(ApicMode::XApic, true, false), ApInit::Proceed);
        // Globally disabled: enabling into xAPIC is legal; x2APIC needs the
        // kernel to leave IA32_APIC_BASE alone, so it halts.
        assert_eq!(
            ap_mode_check(ApicMode::XApic, false, false),
            ApInit::Proceed
        );
        assert_eq!(
            ap_mode_check(ApicMode::X2Apic, false, false),
            ApInit::HaltModeMismatch
        );
    }

    /// x2APIC ICR: one 64-bit value, the full 32-bit destination in 63:32.
    #[test]
    fn x2apic_icr_carries_the_full_destination() {
        assert_eq!(x2apic_icr(3, 0x40fd), 0x0000_0003_0000_40fd);
        assert_eq!(x2apic_icr(0, 0x40fc), 0x0000_0000_0000_40fc);
        // An ID above 255, which xAPIC would truncate.
        assert_eq!(x2apic_icr(0x1_02, 0x40fd), 0x0000_0102_0000_40fd);
        assert_eq!(x2apic_icr(0xffff_fffe, 0x40fd), 0xffff_fffe_0000_40fd);
    }

    /// MSI: destination ID in address bits 19:12. APIC ID 0 gives the exact
    /// constant every driver hard-coded before CP-68 (kill-switch equality).
    #[test]
    fn msi_address_targets_the_given_apic_id() {
        assert_eq!(MSI_ADDRESS_BASE, 0xfee0_0000);
        assert_eq!(msi_address(0), Some(0xfee0_0000));
        assert_eq!(msi_address(1), Some(0xfee0_1000));
        assert_eq!(msi_address(5), Some(0xfee0_5000));
        assert_eq!(msi_address(0xff), Some(0xfeef_f000));
    }

    /// An APIC ID that does not fit 8 bits is refused, never truncated
    /// (0x100 would otherwise alias APIC ID 0).
    #[test]
    fn msi_refuses_ids_above_255() {
        assert_eq!(msi_address(0x100), None);
        assert_eq!(msi_address(0x1ff), None);
        assert_eq!(msi_address(u32::MAX), None);
    }

    /// The CPU's IA32_APIC_BASE decides; a disagreement is reported.
    #[test]
    fn mode_follows_the_msr() {
        assert_eq!(resolve_mode(false, false), (ApicMode::XApic, false));
        assert_eq!(resolve_mode(true, true), (ApicMode::X2Apic, false));
        assert_eq!(resolve_mode(true, false), (ApicMode::XApic, true));
        assert_eq!(resolve_mode(false, true), (ApicMode::X2Apic, true));
        assert_eq!(ApicMode::XApic.name(), "xAPIC");
        assert_eq!(ApicMode::X2Apic.name(), "x2APIC");
    }
}
