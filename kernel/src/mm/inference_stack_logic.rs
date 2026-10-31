// SPDX-License-Identifier: MPL-2.0
//! B1c (F5): the pure arithmetic of the dedicated real-LM inference stack —
//! its layout, guard classification, canary and high-water scan. Kernel-free
//! and host-tested (`tools/module-reclaim-logic-tests` mounts it by `#[path]`).
//!
//! Layout, from low to high addresses, starting at a 2 MiB-aligned `region`:
//!
//! ```text
//! region            region+GUARD_LOW        +SIZE                  region+REGION_LEN
//! | low guard (64 KiB unmapped) | stack (mapped, 4 KiB pages) | high guard (4 KiB unmapped) |
//! ```
//!
//! The stack grows down from `stack_top()`; overflowing it touches the low
//! guard, and the CPU cannot push the `#PF` frame onto that same unmapped
//! page, so the fault escalates to `#DF`, which runs on its own IST stack.

/// Bytes of usable stack: the depth the real-brain forward has always run
/// with (the 256 KiB thread stack of the real-brain test builds).
pub const SIZE: u64 = 256 * 1024;
/// Unmapped address space below the stack. 64 KiB, not one page: the kernel
/// target emits no stack probes, so a single frame larger than the guard
/// could step over it. Address space only — nothing backs it.
pub const GUARD_LOW: u64 = 64 * 1024;
/// One unmapped page above the stack (an underflow past the top).
pub const GUARD_HIGH: u64 = 4096;
/// The whole reserved virtual span.
pub const REGION_LEN: u64 = GUARD_LOW + SIZE + GUARD_HIGH;
/// The canary occupies the lowest mapped bytes of the stack.
pub const CANARY_LEN: usize = 64;
pub const CANARY_BYTE: u8 = 0xC5;
/// Every byte above the canary is filled with this before each call, so the
/// deepest byte the call wrote is the lowest byte that is no longer `FILL_BYTE`.
pub const FILL_BYTE: u8 = 0x5A;

/// Region alignment, and the unmapped gap kept between the region and the
/// heap or kernel image beside it.
pub const ALIGN: u64 = 2 * 1024 * 1024;
pub const GAP: u64 = 2 * 1024 * 1024;

/// Candidate region bases, in preference order: above the heap; below the
/// heap (the heap sits under the kernel image); below the kernel image
/// (KASLR near the canonical ceiling: the heap sits directly above the
/// image and runs to the top, so neither heap-side slot fits). `None` is a
/// slot that wraps the address space. The caller still checks each slot
/// against the live page tables, the image, the heap and the HHDM.
pub fn region_candidates(heap_lo: u64, heap_hi: u64, image_lo: u64) -> [Option<u64>; 3] {
    let above = heap_hi
        .checked_add(GAP)
        .and_then(|a| a.checked_add(ALIGN - 1))
        .map(|a| a & !(ALIGN - 1))
        .filter(|a| a.checked_add(REGION_LEN).is_some());
    let below = |lo: u64| {
        lo.checked_sub(GAP)
            .and_then(|b| b.checked_sub(REGION_LEN))
            .map(|b| b & !(ALIGN - 1))
    };
    [above, below(heap_lo), below(image_lo)]
}

/// The lowest mapped stack byte.
pub const fn stack_bottom(region: u64) -> u64 {
    region + GUARD_LOW
}

/// One past the highest mapped stack byte: the initial `rsp` (16-aligned).
pub const fn stack_top(region: u64) -> u64 {
    region + GUARD_LOW + SIZE
}

/// Which guard page (if any) an address falls in.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GuardHit {
    /// The low guard: a stack overflow.
    Low,
    /// The high guard: an underflow (a return or pop past the top).
    High,
}

/// Classify `addr` against the guard pages of the region at `region`.
/// `region == 0` means no inference stack was mapped.
pub const fn guard_hit(region: u64, addr: u64) -> Option<GuardHit> {
    if region == 0 {
        return None;
    }
    if addr >= region && addr < region + GUARD_LOW {
        return Some(GuardHit::Low);
    }
    let high = region + GUARD_LOW + SIZE;
    if addr >= high && addr < high + GUARD_HIGH {
        return Some(GuardHit::High);
    }
    None
}

/// Is the canary (the lowest `CANARY_LEN` bytes of `stack`) intact?
pub fn canary_intact(stack: &[u8]) -> bool {
    stack.len() >= CANARY_LEN && stack[..CANARY_LEN].iter().all(|&b| b == CANARY_BYTE)
}

/// Bytes of stack used by the call: from the top down to the lowest byte that
/// no longer holds `FILL_BYTE` (the canary region counts as used only if it
/// was overwritten). `stack` is the whole mapped stack, lowest byte first.
pub fn depth_used(stack: &[u8]) -> usize {
    let mut lowest = stack.len();
    for (i, &b) in stack.iter().enumerate() {
        let expected = if i < CANARY_LEN {
            CANARY_BYTE
        } else {
            FILL_BYTE
        };
        if b != expected {
            lowest = i;
            break;
        }
    }
    stack.len() - lowest
}

/// Prepare `stack` (lowest byte first) for a call: canary, then fill.
pub fn prepare(stack: &mut [u8]) {
    let canary = CANARY_LEN.min(stack.len());
    stack[..canary].fill(CANARY_BYTE);
    stack[canary..].fill(FILL_BYTE);
}
