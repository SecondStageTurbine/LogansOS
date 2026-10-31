// SPDX-License-Identifier: MPL-2.0
//! DI-1b resize: the WM-to-client configure mailbox (feature
//! `atrium-configure`).
//!
//! IPC here is a synchronous rendezvous: a send blocks until the peer
//! receives. A WM that sent a client an unsolicited `OP_CONFIGURE` would park
//! until the client next received on that channel — and deadlock outright
//! against a client parked in its own send of a commit. So the WM never sends
//! a configure. It *publishes* one in a four-word buffer the client maps
//! read-only; the client polls it while idle (one atomic load), lays itself
//! out, then acks with an ordinary `OP_CONFIGURE` request on its surface
//! channel, which the WM answers like any request. The WM honours an ack
//! only when it equals what it published (a client cannot size itself).
//!
//! Layout (four little-endian `u32` words, a 4x1 XRGB8888 buffer): `seq`,
//! `width`, `height`, and a zero word. A seqlock: the WM writes
//! `seq = 2·serial − 1` (odd: being written), then the size, then
//! `seq = 2·serial` (even: complete). A reader takes the size only when it
//! read the same even, non-zero `seq` before and after it.
//!
//! Serials live in `1..=SERIAL_MAX` (31 bits) and wrap from `SERIAL_MAX`
//! back to 1 ([`next_serial`]): `2·serial` then never overflows to the
//! reserved `seq` 0, and the counter never pins (#53 nit). A client applies a
//! configure whose serial differs from the last one it applied
//! ([`is_new`]), never "greater than", so the wrap is just another change.

use core::sync::atomic::{AtomicU32, Ordering};

pub const MAILBOX_WIDTH: u32 = 4;
pub const MAILBOX_HEIGHT: u32 = 1;
pub const MAILBOX_STRIDE: u32 = 16;
pub const MAILBOX_BYTES: u64 = 16;
pub const MAILBOX_WORDS: usize = 4;

/// A published configure: the serial (from 1) and the size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Published {
    pub serial: u32,
    pub width: u32,
    pub height: u32,
}

/// The largest serial: `2·SERIAL_MAX` is the largest even `u32`.
pub const SERIAL_MAX: u32 = 0x7fff_ffff;

/// The serial after `serial`: `1..=SERIAL_MAX`, wrapping to 1 (skipping the
/// reserved 0). The WM's only way to advance its counter.
pub const fn next_serial(serial: u32) -> u32 {
    if serial >= SERIAL_MAX { 1 } else { serial + 1 }
}

/// The client side: whether `published` is a configure it has not applied
/// yet (`last_applied` 0 = none). A change of serial, not an increase, so
/// the wrap from `SERIAL_MAX` to 1 is honoured.
pub fn is_new(published: Published, last_applied: u64) -> bool {
    u64::from(published.serial) != last_applied
}

/// The WM side: publish `configure`. Its serial is in `1..=SERIAL_MAX` and
/// differs from the last one published (see [`next_serial`]).
pub fn publish(words: &[AtomicU32; MAILBOX_WORDS], configure: Published) {
    debug_assert!(configure.serial != 0 && configure.serial <= SERIAL_MAX);
    let seq = configure.serial.wrapping_mul(2);
    words[0].store(seq.wrapping_sub(1), Ordering::Release);
    words[1].store(configure.width, Ordering::Release);
    words[2].store(configure.height, Ordering::Release);
    words[0].store(seq, Ordering::Release);
}

/// The client side: the latest complete configure, or `None` (none
/// published yet, or a write in progress — poll again).
pub fn read(words: &[AtomicU32; MAILBOX_WORDS]) -> Option<Published> {
    let before = words[0].load(Ordering::Acquire);
    if before == 0 || before % 2 == 1 {
        return None;
    }
    let width = words[1].load(Ordering::Acquire);
    let height = words[2].load(Ordering::Acquire);
    let after = words[0].load(Ordering::Acquire);
    (before == after).then_some(Published {
        serial: before / 2,
        width,
        height,
    })
}

/// The WM side: whether a client's `OP_CONFIGURE` ack is exactly the
/// configure published (serial and size). Anything else is refused: a
/// client cannot size itself, ack ahead, or ack a stale configure.
pub fn acks(published: Published, width: u32, height: u32, serial: u64) -> bool {
    serial == u64::from(published.serial) && width == published.width && height == published.height
}

/// View a mapped mailbox buffer as its four words.
///
/// # Safety
///
/// `base` must point to a mapping of at least `MAILBOX_BYTES`, 4-byte
/// aligned, that stays mapped for the life of the process (the kernel pins a
/// mapped buffer; there is no unmap). The other side may write it at any
/// time; every access is atomic.
pub unsafe fn words(base: *const u8) -> &'static [AtomicU32; MAILBOX_WORDS] {
    // SAFETY: per the contract above.
    unsafe { &*(base as *const [AtomicU32; MAILBOX_WORDS]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mailbox() -> [AtomicU32; MAILBOX_WORDS] {
        [const { AtomicU32::new(0) }; MAILBOX_WORDS]
    }

    #[test]
    fn nothing_is_read_before_the_first_publish() {
        assert_eq!(read(&mailbox()), None);
    }

    #[test]
    fn a_publish_is_read_back_whole() {
        let m = mailbox();
        let c = Published {
            serial: 1,
            width: 760,
            height: 460,
        };
        publish(&m, c);
        assert_eq!(read(&m), Some(c));
        let d = Published {
            serial: 2,
            width: 320,
            height: 200,
        };
        publish(&m, d);
        assert_eq!(read(&m), Some(d));
        // The byte layout is the documented one (the WM and the client agree
        // on it independently of these functions).
        let raw: [u32; 4] = core::array::from_fn(|i| m[i].load(Ordering::Relaxed));
        assert_eq!(raw, [4, 320, 200, 0]);
    }

    /// Every state a reader can observe mid-publish is either the old
    /// configure or `None`, never a torn mix (the new width with the old
    /// height, or either with the old serial).
    #[test]
    fn a_reader_never_sees_a_torn_configure() {
        let old = Published {
            serial: 1,
            width: 640,
            height: 400,
        };
        let new = Published {
            serial: 2,
            width: 900,
            height: 520,
        };
        // The four stores of `publish(new)` over `old`, applied one at a time.
        let steps: [(usize, u32); 4] = [(0, 3), (1, 900), (2, 520), (0, 4)];
        for done in 0..=steps.len() {
            let m = mailbox();
            publish(&m, old);
            for &(i, v) in &steps[..done] {
                m[i].store(v, Ordering::Relaxed);
            }
            let got = read(&m);
            match done {
                0 => assert_eq!(got, Some(old)),
                4 => assert_eq!(got, Some(new)),
                _ => assert_eq!(got, None, "after {done} stores"),
            }
        }
    }

    /// #53 nit KAT: the serial wraps from `SERIAL_MAX` to 1, skipping 0, so
    /// `seq` never reaches the reserved 0 (the u32 serial `0x8000_0000` would
    /// have published `seq` 0, read as "nothing published", and a saturating
    /// counter then pinned every later configure pending).
    #[test]
    fn the_serial_wraps_past_the_boundary_skipping_zero() {
        assert_eq!(SERIAL_MAX, 0x7fff_ffff);
        assert_eq!(next_serial(1), 2);
        assert_eq!(next_serial(0x7fff_fffe), 0x7fff_ffff);
        assert_eq!(next_serial(0x7fff_ffff), 1);
        // The failure the wrap prevents: the next u32 serial doubles to 0.
        assert_eq!(0x8000_0000u32.wrapping_mul(2), 0);

        // A WM counter crossing the boundary: every publish is read back
        // whole, with an even non-zero seq, and is new to a client that
        // applied the one before it.
        let m = mailbox();
        let mut serial = 0x7fff_fffe;
        let mut applied = 0u64;
        let mut seen = [0u32; 4];
        for (i, slot) in seen.iter_mut().enumerate() {
            let c = Published {
                serial,
                width: 640 + i as u32,
                height: 400,
            };
            publish(&m, c);
            let raw = m[0].load(Ordering::Relaxed);
            assert!(raw != 0 && raw.is_multiple_of(2), "seq {raw:#x}");
            let got = read(&m).expect("a complete publish is readable");
            assert_eq!(got, c);
            assert!(
                is_new(got, applied),
                "serial {serial:#x} after {applied:#x}"
            );
            applied = u64::from(got.serial);
            // Re-reading the same configure is not new.
            assert!(!is_new(got, applied));
            *slot = serial;
            serial = next_serial(serial);
        }
        assert_eq!(seen, [0x7fff_fffe, 0x7fff_ffff, 1, 2]);
        // The boundary bytes: seq for SERIAL_MAX is the largest even u32.
        publish(
            &m,
            Published {
                serial: SERIAL_MAX,
                width: 1,
                height: 2,
            },
        );
        let raw: [u32; 4] = core::array::from_fn(|i| m[i].load(Ordering::Relaxed));
        assert_eq!(raw, [0xffff_fffe, 1, 2, 0]);
    }

    /// Only the ack that repeats the published serial and size is honoured.
    #[test]
    fn only_the_published_configure_is_acked() {
        let p = Published {
            serial: 3,
            width: 760,
            height: 460,
        };
        assert!(acks(p, 760, 460, 3));
        for (w, h, serial) in [
            (760, 460, 2),
            (760, 460, 4),
            (761, 460, 3),
            (760, 459, 3),
            (1280, 800, 3),
            (760, 460, 0),
            (760, 460, 3 + (1 << 32)),
        ] {
            assert!(!acks(p, w, h, serial), "{w}x{h} serial {serial}");
        }
    }
}
