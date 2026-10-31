// SPDX-License-Identifier: MPL-2.0
//! OT-22: the `SYS_WAIT_ANY` ABI, pure (host-tested from
//! `tools/atrium-logic-tests` against hand-laid bytes, independent of
//! libneo's `WaitItem`).
//!
//! An item is 8 little-endian bytes: `u32 handle`, then `u32 kind`.

/// Bytes per item.
pub const ITEM_LEN: usize = 8;
/// A channel the caller could receive on without parking.
pub const WAIT_KIND_CHANNEL: u32 = 1;
/// An input focus or pointer-route leaf with an event queued.
pub const WAIT_KIND_INPUT: u32 = 2;
/// A pending SAK chord (the handle is ignored).
pub const WAIT_KIND_SAK: u32 = 3;
/// Nanoseconds per timer tick (100 Hz).
pub const NS_PER_TICK: u64 = 10_000_000;

/// `(handle, kind)` from one item's bytes.
pub fn decode_item(b: &[u8; ITEM_LEN]) -> (u32, u32) {
    (
        u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
    )
}

/// An absolute monotonic deadline in ns as an absolute tick, rounded up so a
/// wait never ends before the instant asked for. `u64::MAX` stays "none".
pub fn deadline_ticks(deadline_ns: u64) -> u64 {
    if deadline_ns == u64::MAX {
        u64::MAX
    } else {
        deadline_ns.div_ceil(NS_PER_TICK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-laid bytes: handle 0x04030201, kind 2 (input).
    #[test]
    fn an_item_decodes_from_its_little_endian_bytes() {
        assert_eq!(
            decode_item(&[0x01, 0x02, 0x03, 0x04, 0x02, 0x00, 0x00, 0x00]),
            (0x0403_0201, WAIT_KIND_INPUT)
        );
        assert_eq!(
            decode_item(&[0xff, 0xff, 0xff, 0xff, 0x03, 0x00, 0x00, 0x00]),
            (u32::MAX, WAIT_KIND_SAK)
        );
    }

    #[test]
    fn deadlines_round_up_to_the_next_tick() {
        assert_eq!(deadline_ticks(0), 0);
        assert_eq!(deadline_ticks(1), 1);
        assert_eq!(deadline_ticks(10_000_000), 1);
        assert_eq!(deadline_ticks(10_000_001), 2);
        assert_eq!(deadline_ticks(u64::MAX), u64::MAX);
    }
}
