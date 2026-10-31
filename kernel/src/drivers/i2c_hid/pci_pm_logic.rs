// SPDX-License-Identifier: MPL-2.0
//! The touchpad controller's D3hot → D0 transition and BAR0 (pure).
//!
//! PCI PM 1.2 §5.4/§8.2.2: with PMCSR `No_Soft_Reset` (bit 3) clear, the
//! D3hot → D0 transition is an internal reset that returns the function to
//! D0-uninitialized, so the BARs and the command register read 0 afterwards.
//! The OS is expected to have saved them and to write them back (Linux does
//! this as a save before and a restore after the transition; read for the
//! principle only). Image A3 on the Zenbook stopped on `bar0=0x0` after
//! exactly this transition.
//!
//! The driver reads BAR0/BAR1 as raw dwords before touching PMCSR and again
//! after D0 + 10 ms, and [`decide`] says what to do with them.
//!
//! Host-tested through `tools/i2c-hid-logic-tests`.

/// PMCSR bit 3: `No_Soft_Reset`. Set: D3hot → D0 keeps the configuration.
pub const PMCSR_NO_SOFT_RESET: u16 = 1 << 3;

/// A memory BAR's address bits in its low dword.
const BAR_ADDR_MASK: u32 = 0xFFFF_FFF0;

/// The BAR0/BAR1 config dwords (offsets 0x10 and 0x14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bars {
    pub bar0: u32,
    pub bar1: u32,
}

impl Bars {
    /// BAR0 is an I/O BAR (bit 0).
    pub const fn is_io(self) -> bool {
        self.bar0 & 1 != 0
    }

    /// BAR0 is a 64-bit memory BAR (type bits 2:1 = 0b10), so BAR1 is its
    /// high dword.
    pub const fn is_64bit(self) -> bool {
        (self.bar0 >> 1) & 0x3 == 0x2
    }

    /// BAR0's address (with BAR1 as the high dword for a 64-bit BAR).
    pub const fn address(self) -> u64 {
        let low = (self.bar0 & BAR_ADDR_MASK) as u64;
        if self.is_64bit() {
            low | ((self.bar1 as u64) << 32)
        } else {
            low
        }
    }
}

/// What to do with BAR0 after the D0 transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarDecision {
    /// BAR0 is assigned and survived: carry on.
    Ok,
    /// BAR0 was assigned before and reads 0 after (the soft reset cleared
    /// it): write back the saved dwords (BAR0, then BAR1 for a 64-bit BAR).
    Restore,
    /// BAR0 was already 0 before the transition: firmware never assigned it.
    Unassigned,
    /// BAR0 is an I/O BAR: not this controller's layout.
    NotMemory,
    /// BAR0 reads a different non-zero address, an I/O BAR or all-ones
    /// after: nothing known explains it, so the driver stops rather than
    /// guess.
    Changed,
}

/// Decide from the dwords read before touching PMCSR and after D0.
pub const fn decide(before: Bars, after: Bars) -> BarDecision {
    if before.is_io() {
        return BarDecision::NotMemory;
    }
    if before.address() == 0 {
        return BarDecision::Unassigned;
    }
    // An I/O-looking value after (all-ones included: the function is not
    // answering config reads) is not a cleared BAR.
    if after.is_io() {
        return BarDecision::Changed;
    }
    if after.address() == before.address() {
        BarDecision::Ok
    } else if after.address() == 0 {
        BarDecision::Restore
    } else {
        BarDecision::Changed
    }
}

// --- Assigning an unassigned BAR0 (`i2c-hid-bar-assign`) ---------------------
//
// Image A4 on the Zenbook: BAR0 is a 64-bit memory BAR that firmware never
// assigned (`bar0=0x00000004`, `nsr=1`), as Intel laptop firmware commonly
// leaves the LPSS BARs for the OS. The driver places it itself: the lowest
// naturally aligned slot at or above the lowest firmware 64-bit BAR that
// clears every range already in use (every function's BARs, every bridge
// window, every memory-map entry), then probes the slot before claiming it.
// Read for principle only: Linux's `setup-res.c` / `setup-bus.c`.

/// A half-open physical range `[start, end)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: u64,
    pub end: u64,
}

impl Range {
    const fn overlaps(self, start: u64, end: u64) -> bool {
        self.start < end && start < self.end
    }
}

/// The size a memory BAR decodes, from the dwords read back after writing
/// all-ones (`low` at the BAR, `high` at the next dword for a 64-bit BAR).
/// `None` for an I/O BAR or one that decodes nothing.
pub const fn bar_size_from_mask(low: u32, high: u32, is_64bit: bool) -> Option<u64> {
    if low & 1 != 0 {
        return None;
    }
    let mask = if is_64bit {
        ((high as u64) << 32) | (low & BAR_ADDR_MASK) as u64
    } else {
        // A 32-bit BAR: the upper half of the 64-bit view is all ones.
        0xFFFF_FFFF_0000_0000 | (low & BAR_ADDR_MASK) as u64
    };
    if mask == 0 || mask == 0xFFFF_FFFF_0000_0000 {
        return None;
    }
    Some((!mask).wrapping_add(1))
}

/// The most an assigned memory BAR at `base` can decode: BARs are naturally
/// aligned to their size, so the size divides `base`. Used for every BAR the
/// driver does not size itself (it never writes another device's BARs), so
/// the excluded range can be larger than the device's, never smaller.
pub const fn assigned_bar_bound(base: u64) -> Range {
    let size = if base == 0 {
        0
    } else if base.trailing_zeros() >= 63 {
        1 << 63
    } else {
        1u64 << base.trailing_zeros()
    };
    Range {
        start: base,
        end: base.saturating_add(size),
    }
}

/// `x` rounded up to a multiple of `align` (a power of two); `None` on wrap.
const fn align_up(x: u64, align: u64) -> Option<u64> {
    match x.checked_add(align - 1) {
        Some(v) => Some(v & !(align - 1)),
        None => None,
    }
}

/// The candidate addresses for a BAR of `size` bytes (a power of two), in
/// increasing order: each naturally aligned, at or above `floor`, with
/// `start + size <= ceiling`, and clear of every range in `occupied`. Writes
/// at most `out.len()` and returns how many.
pub fn candidates(
    occupied: &[Range],
    size: u64,
    floor: u64,
    ceiling: u64,
    out: &mut [u64],
) -> usize {
    if size == 0 || !size.is_power_of_two() {
        return 0;
    }
    let mut n = 0;
    let Some(mut at) = align_up(floor, size) else {
        return 0;
    };
    while n < out.len() {
        let Some(end) = at.checked_add(size) else {
            break;
        };
        if end > ceiling {
            break;
        }
        // Past every range that overlaps this slot, if any.
        let mut past = None;
        for r in occupied {
            if r.overlaps(at, end) && past.is_none_or(|p| r.end > p) {
                past = Some(r.end);
            }
        }
        match past {
            None => {
                out[n] = at;
                n += 1;
                at = end;
            }
            Some(p) => match align_up(p, size) {
                Some(next) => at = next,
                None => break,
            },
        }
    }
    n
}
