// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/arch/x86_64/fpu_logic.rs`: the XSAVE area size.
//!
//! The real module is mounted through `#[path]`, so these tests exercise the
//! exact code the kernel runs. The component offsets and sizes are written
//! out by hand from the Intel SDM Vol. 1, 13.4 ("XSAVE Area") / table
//! "Offsets and Sizes of State Components" (standard format): AVX (YMM_Hi128)
//! 256 bytes at 576; AVX-512 opmask 64 at 1088, ZMM_Hi256 512 at 1152,
//! Hi16_ZMM 1024 at 1664; PKRU 8 at 2688. Not computed from the code under
//! test.

#![allow(dead_code)]

#[path = "../../../kernel/src/arch/x86_64/fpu_logic.rs"]
mod fpu_logic;

#[cfg(test)]
mod tests {
    use super::fpu_logic::{Component, XSAVE_LEGACY_AND_HEADER, ebx_understated, xsave_area_size};

    const AVX: Component = Component {
        size: 256,
        offset: 576,
    };
    const OPMASK: Component = Component {
        size: 64,
        offset: 1088,
    };
    const ZMM_HI256: Component = Component {
        size: 512,
        offset: 1152,
    };
    const HI16_ZMM: Component = Component {
        size: 1024,
        offset: 1664,
    };
    const PKRU: Component = Component {
        size: 8,
        offset: 2688,
    };

    #[test]
    fn legacy_plus_header_is_576() {
        assert_eq!(XSAVE_LEGACY_AND_HEADER, 576);
    }

    /// The preview under WHPX: EBX said 576 with XCR0 = x87|SSE|AVX. The
    /// area must still end past AVX: 576 + 256 = 832.
    #[test]
    fn whpx_understated_ebx_is_overruled_by_avx() {
        assert_eq!(xsave_area_size(576, [AVX]), 832);
        assert!(ebx_understated(576, 832));
    }

    /// A correct EBX (TCG, real CPUs) gives the same 832.
    #[test]
    fn correct_ebx_agrees() {
        assert_eq!(xsave_area_size(832, [AVX]), 832);
        assert!(!ebx_understated(832, 832));
    }

    /// x87|SSE only (no component >= 2): the legacy region plus the header,
    /// even if EBX were reported as 0.
    #[test]
    fn no_extended_components() {
        assert_eq!(xsave_area_size(576, []), 576);
        assert_eq!(xsave_area_size(0, []), 576);
    }

    /// EBX larger than the components need is kept (it may cover padding
    /// the kernel doesn't model).
    #[test]
    fn larger_ebx_is_kept() {
        assert_eq!(xsave_area_size(1024, [AVX]), 1024);
    }

    /// AVX-512 and PKRU: the furthest end wins (PKRU, 2688 + 8 = 2696),
    /// rounded up to 64: 2752.
    #[test]
    fn avx512_and_pkru_round_up_to_64() {
        assert_eq!(
            xsave_area_size(576, [AVX, OPMASK, ZMM_HI256, HI16_ZMM]),
            2688
        );
        assert_eq!(
            xsave_area_size(576, [AVX, OPMASK, ZMM_HI256, HI16_ZMM, PKRU]),
            2752
        );
    }

    /// An odd EBX is rounded up to the 64-byte alignment.
    #[test]
    fn odd_ebx_rounds_up() {
        assert_eq!(xsave_area_size(833, [AVX]), 896);
    }

    /// Components are order-independent.
    #[test]
    fn order_does_not_matter() {
        assert_eq!(
            xsave_area_size(576, [HI16_ZMM, AVX]),
            xsave_area_size(576, [AVX, HI16_ZMM])
        );
    }
}
