// SPDX-License-Identifier: MPL-2.0
//! The BAR0 decision across the touchpad controller's D3hot -> D0 transition
//! (`pci_pm_logic::decide`). Each BAR dword is written out by hand from the
//! PCI 3.0 BAR layout: bit 0 = I/O, bits 2:1 = type (0b00 32-bit, 0b10
//! 64-bit), bit 3 = prefetchable, bits 31:4 = address.

use super::pci_pm_logic::{BarDecision, Bars, PMCSR_NO_SOFT_RESET, decide};

/// A 64-bit non-prefetchable BAR at 0x4_1700_0000: low dword 0x1700_0004
/// (address 0x1700_0000, type 0b10 -> 0x4), high dword 0x0000_0004.
const ASSIGNED_64: Bars = Bars {
    bar0: 0x1700_0004,
    bar1: 0x0000_0004,
};
/// What a soft reset leaves: the type bits stay (they are read-only), the
/// address reads 0.
const RESET_64: Bars = Bars {
    bar0: 0x0000_0004,
    bar1: 0x0000_0000,
};

#[test]
fn bar_layout_is_read_as_the_spec_lays_it_out() {
    assert!(ASSIGNED_64.is_64bit());
    assert!(!ASSIGNED_64.is_io());
    assert_eq!(ASSIGNED_64.address(), 0x4_1700_0000);
    // 32-bit at 0xFE20_0000: type 0b00, the next dword is not part of it.
    let b32 = Bars {
        bar0: 0xFE20_0000,
        bar1: 0xDEAD_BEEF,
    };
    assert!(!b32.is_64bit());
    assert_eq!(b32.address(), 0xFE20_0000);
    // PMCSR bit 3.
    assert_eq!(PMCSR_NO_SOFT_RESET, 0x0008);
}

#[test]
fn assigned_and_kept_is_ok() {
    assert_eq!(decide(ASSIGNED_64, ASSIGNED_64), BarDecision::Ok);
}

#[test]
fn assigned_then_cleared_by_the_soft_reset_is_restored() {
    // The image A3 case: assigned before, 0 after D3hot -> D0.
    assert_eq!(decide(ASSIGNED_64, RESET_64), BarDecision::Restore);
    // All-zero after (type bits read 0 too) is the same.
    let zero = Bars { bar0: 0, bar1: 0 };
    assert_eq!(decide(ASSIGNED_64, zero), BarDecision::Restore);
    // A 64-bit BAR whose address is only in the high dword.
    let high_only = Bars {
        bar0: 0x0000_0004,
        bar1: 0x0000_0001,
    };
    assert_eq!(high_only.address(), 0x1_0000_0000);
    assert_eq!(decide(high_only, RESET_64), BarDecision::Restore);
}

#[test]
fn zero_before_the_transition_is_unassigned_not_restored() {
    assert_eq!(decide(RESET_64, RESET_64), BarDecision::Unassigned);
    assert_eq!(decide(RESET_64, ASSIGNED_64), BarDecision::Unassigned);
    let zero = Bars { bar0: 0, bar1: 0 };
    assert_eq!(decide(zero, zero), BarDecision::Unassigned);
}

#[test]
fn an_io_bar_or_a_different_address_stops() {
    let io = Bars {
        bar0: 0x0000_E001,
        bar1: 0,
    };
    assert_eq!(decide(io, io), BarDecision::NotMemory);
    let moved = Bars {
        bar0: 0x1800_0004,
        bar1: 0x0000_0004,
    };
    assert_eq!(decide(ASSIGNED_64, moved), BarDecision::Changed);
    // An I/O value, or all-ones (the function not answering), after a
    // memory BAR: not a cleared BAR, so no restore.
    assert_eq!(decide(ASSIGNED_64, io), BarDecision::Changed);
    let all_ones = Bars {
        bar0: 0xFFFF_FFFF,
        bar1: 0xFFFF_FFFF,
    };
    assert_eq!(decide(ASSIGNED_64, all_ones), BarDecision::Changed);
}

// --- BAR assignment (`i2c-hid-bar-assign`) ---------------------------------

use super::pci_pm_logic::{Range, assigned_bar_bound, bar_size_from_mask, candidates};

const fn r(start: u64, end: u64) -> Range {
    Range { start, end }
}

#[test]
fn bar_size_is_read_from_the_all_ones_mask() {
    // A 4 KiB 64-bit BAR reads back 0xFFFF_F004 / 0xFFFF_FFFF.
    assert_eq!(
        bar_size_from_mask(0xFFFF_F004, 0xFFFF_FFFF, true),
        Some(0x1000)
    );
    // 16 KiB.
    assert_eq!(
        bar_size_from_mask(0xFFFF_C004, 0xFFFF_FFFF, true),
        Some(0x4000)
    );
    // A 64-bit BAR whose size needs the high dword: 8 GiB reads
    // 0x0000_000C (prefetchable, 64-bit) / 0xFFFF_FFFE.
    assert_eq!(
        bar_size_from_mask(0x0000_000C, 0xFFFF_FFFE, true),
        Some(0x2_0000_0000)
    );
    // A 32-bit 1 MiB BAR: 0xFFF0_0000.
    assert_eq!(bar_size_from_mask(0xFFF0_0000, 0, false), Some(0x10_0000));
    // Nothing decoded, or an I/O BAR.
    assert_eq!(bar_size_from_mask(0x0000_0004, 0, true), None);
    assert_eq!(bar_size_from_mask(0xFFFF_FF01, 0, false), None);
}

#[test]
fn an_assigned_bar_is_bounded_by_its_alignment() {
    // The Zenbook's xHCIs: 0x140_1734_0000 is 256 KiB aligned, 0x140_1732_0000
    // 128 KiB aligned.
    assert_eq!(
        assigned_bar_bound(0x140_1734_0000),
        r(0x140_1734_0000, 0x140_1738_0000)
    );
    assert_eq!(
        assigned_bar_bound(0x140_1732_0000),
        r(0x140_1732_0000, 0x140_1734_0000)
    );
    assert_eq!(assigned_bar_bound(0), r(0, 0));
    // The top of the space saturates, never wraps.
    assert_eq!(assigned_bar_bound(1 << 63).end, u64::MAX);
}

#[test]
fn the_lowest_clear_aligned_slot_is_chosen() {
    const CEIL: u64 = 1 << 46;
    let mut out = [0u64; 4];
    // Nothing in use: the floor itself (already aligned).
    assert_eq!(candidates(&[], 0x1000, 0x140_0000_0000, CEIL, &mut out), 4);
    assert_eq!(
        out,
        [
            0x140_0000_0000,
            0x140_0000_1000,
            0x140_0000_2000,
            0x140_0000_3000
        ]
    );
    // An unaligned floor rounds up to the BAR's alignment.
    assert_eq!(
        candidates(&[], 0x4000, 0x140_0000_1234, CEIL, &mut out[..1]),
        1
    );
    assert_eq!(out[0], 0x140_0000_4000);
}

#[test]
fn occupied_ranges_are_skipped_including_partial_overlaps() {
    const CEIL: u64 = 1 << 46;
    let used = [
        // The xHCIs' bounds, out of order.
        r(0x140_1734_0000, 0x140_1738_0000),
        r(0x140_1732_0000, 0x140_1734_0000),
        // A range that only clips the first 4 KiB slot after them.
        r(0x140_1737_0000, 0x140_1738_0800),
    ];
    let mut out = [0u64; 2];
    let n = candidates(&used, 0x1000, 0x140_1732_0000, CEIL, &mut out);
    assert_eq!(n, 2);
    // 0x140_1738_0000 is clipped (to 0x..._0800), so the first clear 4 KiB
    // slot is 0x140_1738_1000.
    assert_eq!(out, [0x140_1738_1000, 0x140_1738_2000]);
    // Every candidate is clear of every range.
    for &c in &out[..n] {
        for u in &used {
            assert!(c + 0x1000 <= u.start || c >= u.end, "{c:#x} vs {u:?}");
        }
    }
}

#[test]
fn a_gap_between_ranges_is_used_only_if_the_bar_fits_aligned() {
    const CEIL: u64 = 1 << 46;
    // A 0x3000 gap at 0x1_0000_1000..0x1_0000_4000: a 0x4000 BAR does not fit
    // aligned, a 0x1000 BAR does.
    let used = [
        r(0x1_0000_0000, 0x1_0000_1000),
        r(0x1_0000_4000, 0x1_0001_0000),
    ];
    let mut out = [0u64; 1];
    assert_eq!(candidates(&used, 0x4000, 0x1_0000_0000, CEIL, &mut out), 1);
    assert_eq!(out[0], 0x1_0001_0000);
    assert_eq!(candidates(&used, 0x1000, 0x1_0000_0000, CEIL, &mut out), 1);
    assert_eq!(out[0], 0x1_0000_1000);
}

#[test]
fn the_ceiling_wrap_and_bad_sizes_give_no_candidate() {
    let mut out = [0u64; 2];
    // Every slot below the ceiling is taken.
    let full = [r(0x1000, 0x1_0000)];
    assert_eq!(candidates(&full, 0x1000, 0x1000, 0x1_0000, &mut out), 0);
    // A floor that rounds up past the top of the space.
    assert_eq!(
        candidates(&[], 0x1000, u64::MAX - 10, u64::MAX, &mut out),
        0
    );
    // A range reaching the top of the space: nothing past it.
    let top = [r(0xFFFF_FFFF_FFFF_0000, u64::MAX)];
    assert_eq!(
        candidates(&top, 0x1000, 0xFFFF_FFFF_FFFF_0000, u64::MAX, &mut out),
        0
    );
    // Size 0 or not a power of two.
    assert_eq!(candidates(&[], 0, 0x1000, 1 << 46, &mut out), 0);
    assert_eq!(candidates(&[], 0x3000, 0x1000, 1 << 46, &mut out), 0);
}
