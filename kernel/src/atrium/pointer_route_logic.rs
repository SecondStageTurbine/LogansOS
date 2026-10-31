// SPDX-License-Identifier: MPL-2.0
//! DI-1a B-ref: kernel-minted, consume-once pointer records (THINK §8.3).
//!
//! The WM receives every mouse report as a kind-4 record carrying a sequence
//! tag, chooses the target and local coordinates (trusted policy), and hands
//! the tag back to forward or drop it. The kernel keeps the authoritative
//! copy here, so the buttons and phase a client finally receives come from
//! the record, never from the WM, and a record can be consumed exactly once.
//!
//! Pure and syscall-free (host KATs). Fixed size, no allocation:
//! - a 64-slot ring indexed by `seq % 64`, tagged with the sequence itself:
//!   sequences run 1..=[`MAX_SEQ`] (0 is never minted), so a tag names one
//!   record for the whole life of the route and can never alias a newer one
//!   (Jarvis, #46 RETURN F1);
//! - minting over a still-live slot expires it (counted); if that record was
//!   a button transition, the next record carries [`RESYNC`];
//! - past [`MAX_SEQ`] the route is spent: `mint` returns `None` and the
//!   caller emits one [`ROUTE_EXPIRED`] record ([`RecordRing::expiry`]) so the
//!   WM reclaims a fresh route (new handle, new ring). Fail closed: nothing
//!   is minted on a spent route.

use super::input_logic::InputEventV1;

/// Wire kind 4: a pointer record for the WM. `flags` bits 0–4 = the full
/// button bitmap, bit 5 = [`WHEEL`], bit 7 = [`RESYNC`]; `code` =
/// `seq & 0xffff`; `x`/`y` = the report's relative motion, or for a
/// [`WHEEL`] record `x` = the wheel and `y` = 0.
pub const INPUT_EVENT_POINTER_REC: u8 = 4;
/// Wire kind 3: a forwarded absolute pointer event for a client. `flags` =
/// buttons, `code` = phase, `x`/`y` = surface-local (WM-chosen, i16).
pub const INPUT_EVENT_POINTER_ABS: u8 = 3;
/// Set on the first record after a button transition was lost (expired or
/// dropped): take this record's bitmap as the truth, not a delta.
pub const RESYNC: u8 = 0x80;
/// Set on the one record a spent route emits: reclaim a new route. Its tag
/// is 0, which is never minted, so it cannot be consumed.
pub const ROUTE_EXPIRED: u8 = 0x40;
/// A9-C2: a wheel record (`x` = the wheel, no motion). It carries the
/// current buttons and is never a button transition.
pub const WHEEL: u8 = 0x20;
/// A9-C2: wire kind 5, a forwarded wheel event for a client. `flags` =
/// buttons, `code` = the wheel (i16; from the kernel's record, never the
/// WM), `x`/`y` = surface-local (WM-chosen, where the pointer is).
pub const INPUT_EVENT_POINTER_WHEEL: u8 = 5;
/// The last sequence a route mints: every tag stays unique.
pub const MAX_SEQ: u32 = u16::MAX as u32;
pub const RECORD_RING: usize = 64;

pub const PHASE_DOWN: u16 = 1;
pub const PHASE_MOVE: u16 = 2;
pub const PHASE_UP: u16 = 3;

/// Phase of a record relative to the one before it.
pub const fn phase(prev: u8, buttons: u8) -> u16 {
    if buttons & !prev != 0 {
        PHASE_DOWN
    } else if prev & !buttons != 0 {
        PHASE_UP
    } else {
        PHASE_MOVE
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Empty,
    Live,
    Consumed,
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    seq: u32,
    buttons: u8,
    prev: u8,
    /// A9-C2: the wheel of a [`WHEEL`] record; 0 for motion records.
    wheel: i16,
    state: State,
}

/// What a successful consume yields: the record's own buttons and phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Consumed {
    pub buttons: u8,
    pub phase: u16,
    /// A9-C2: non-zero for a wheel record (forwarded as kind 5).
    pub wheel: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsumeError {
    /// Never minted, or overwritten since (the tag no longer matches).
    Stale,
    /// Already forwarded or dropped.
    Already,
}

#[derive(Clone, Copy, Debug)]
pub struct RecordRing {
    slots: [Slot; RECORD_RING],
    next_seq: u32,
    last_buttons: u8,
    resync: bool,
    expired: u64,
    expiry_sent: bool,
}

impl RecordRing {
    pub const fn new() -> Self {
        Self::starting_at(1)
    }

    /// A ring whose first record is `seq` (KATs use this to reach the wrap).
    pub const fn starting_at(seq: u32) -> Self {
        Self {
            slots: [Slot {
                seq: 0,
                buttons: 0,
                prev: 0,
                wheel: 0,
                state: State::Empty,
            }; RECORD_RING],
            next_seq: seq,
            last_buttons: 0,
            resync: false,
            expired: 0,
            expiry_sent: false,
        }
    }

    /// The buttons of the last record minted (a report that changes none of
    /// them, and moves nothing, is not a motion record: A9-C2).
    pub fn buttons(&self) -> u8 {
        self.last_buttons
    }

    /// Records expired before they were consumed.
    pub fn expired(&self) -> u64 {
        self.expired
    }

    /// A record was lost after minting (e.g. the WM's queue dropped it): the
    /// next record carries [`RESYNC`].
    pub fn note_lost(&mut self) {
        self.resync = true;
    }

    /// Mint the record for one report. `None` once the route is spent
    /// (past [`MAX_SEQ`]): the caller sends [`Self::expiry`] and mints
    /// nothing more on this route.
    pub fn mint(&mut self, dx: i16, dy: i16, buttons: u8) -> Option<InputEventV1> {
        self.mint_slot(dx, dy, buttons, 0)
    }

    /// A9-C2: mint a [`WHEEL`] record: `x` = `wheel`, the current buttons
    /// unchanged (its `prev` is those same buttons, so it is never a button
    /// transition and expiring it unconsumed never forces a RESYNC: CP-52).
    /// It never carries [`RESYNC`] either: the WM reads a wheel record for
    /// its wheel alone, so a pending RESYNC waits for the next motion record,
    /// which the WM runs its button decision on. `wheel` 0 mints nothing
    /// (`None` is also a spent route's answer).
    pub fn mint_wheel(&mut self, wheel: i16) -> Option<InputEventV1> {
        if wheel == 0 {
            return None;
        }
        let buttons = self.last_buttons;
        let mut record = self.mint_slot(wheel, 0, buttons, wheel)?;
        record.flags |= WHEEL;
        Some(record)
    }

    /// A9-C2 (Chad, #104 RETURN F2): mint one report's records and hand each
    /// to `deliver` (`true` = kept) before minting the next. The wheel record
    /// goes first and the motion record last, so any loss while the wheel is
    /// handled (a live transition it overwrites, or one its delivery evicts)
    /// is already pending when the motion record is minted, and that record
    /// carries [`RESYNC`] (CP-52). A loss on the motion record's own delivery
    /// reaches the next record, as before A9-C2. The motion record is skipped
    /// when the report moved nothing and changed no button (a wheel-only
    /// report); a report with no wheel mints exactly one motion record. On a
    /// spent route the one [`ROUTE_EXPIRED`] record is delivered instead.
    #[cfg(feature = "atrium-pointer-wheel")]
    pub fn mint_report(
        &mut self,
        report: &InputEventV1,
        mut deliver: impl FnMut(InputEventV1) -> bool,
    ) {
        let wheel = report.wheel();
        let motion = wheel == 0 || report.x != 0 || report.y != 0 || report.flags != self.buttons();
        if wheel != 0 {
            let minted = self.mint_wheel(wheel);
            self.deliver(minted, &mut deliver);
        }
        if motion {
            let minted = self.mint(report.x, report.y, report.flags);
            self.deliver(minted, &mut deliver);
        }
    }

    #[cfg(feature = "atrium-pointer-wheel")]
    fn deliver(
        &mut self,
        minted: Option<InputEventV1>,
        deliver: &mut impl FnMut(InputEventV1) -> bool,
    ) {
        match minted {
            Some(record) => {
                if !deliver(record) {
                    self.note_lost();
                }
            }
            None => {
                if let Some(expiry) = self.expiry() {
                    let _ = deliver(expiry);
                }
            }
        }
    }

    fn mint_slot(&mut self, dx: i16, dy: i16, buttons: u8, wheel: i16) -> Option<InputEventV1> {
        if self.next_seq == 0 || self.next_seq > MAX_SEQ {
            return None;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        let slot = &mut self.slots[seq as usize % RECORD_RING];
        if slot.state == State::Live {
            self.expired += 1;
            if slot.buttons != slot.prev {
                self.resync = true;
            }
        }
        *slot = Slot {
            seq,
            buttons,
            prev: self.last_buttons,
            wheel,
            state: State::Live,
        };
        self.last_buttons = buttons;
        let flags = if wheel == 0 && core::mem::take(&mut self.resync) {
            buttons | RESYNC
        } else {
            buttons
        };
        Some(InputEventV1 {
            kind: INPUT_EVENT_POINTER_REC,
            flags,
            code: seq as u16,
            x: dx,
            y: dy,
        })
    }

    /// The one [`ROUTE_EXPIRED`] record a spent route sends (`None` after
    /// the first call, and while the route still has sequences).
    pub fn expiry(&mut self) -> Option<InputEventV1> {
        if self.next_seq <= MAX_SEQ || self.expiry_sent {
            return None;
        }
        self.expiry_sent = true;
        Some(InputEventV1 {
            kind: INPUT_EVENT_POINTER_REC,
            flags: ROUTE_EXPIRED | self.last_buttons,
            code: 0,
            x: 0,
            y: 0,
        })
    }

    /// Consume the live record tagged `tag` (forward or drop), exactly once.
    pub fn consume(&mut self, tag: u16) -> Result<Consumed, ConsumeError> {
        let slot = &mut self.slots[tag as usize % RECORD_RING];
        if slot.state == State::Empty || slot.seq != tag as u32 {
            return Err(ConsumeError::Stale);
        }
        if slot.state == State::Consumed {
            return Err(ConsumeError::Already);
        }
        slot.state = State::Consumed;
        Ok(Consumed {
            buttons: slot.buttons,
            phase: phase(slot.prev, slot.buttons),
            wheel: slot.wheel,
        })
    }

    /// Would `consume(tag)` succeed? (Validate before consuming.)
    pub fn peek(&self, tag: u16) -> Result<(), ConsumeError> {
        let slot = &self.slots[tag as usize % RECORD_RING];
        match slot.state {
            State::Empty => Err(ConsumeError::Stale),
            _ if slot.seq != tag as u32 => Err(ConsumeError::Stale),
            State::Consumed => Err(ConsumeError::Already),
            State::Live => Ok(()),
        }
    }
}

impl Default for RecordRing {
    fn default() -> Self {
        Self::new()
    }
}

/// Forwarded coordinates must fit the i16 wire fields: rejected, never
/// clamped or wrapped (Jarvis F6).
pub fn forward_coords(x: i64, y: i64) -> Option<(i16, i16)> {
    Some((i16::try_from(x).ok()?, i16::try_from(y).ok()?))
}
