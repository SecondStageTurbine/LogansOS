// SPDX-License-Identifier: MPL-2.0
//! The WM's per-record pointer decision (DI-1a): is this record a fresh
//! press, and must the grab a lost transition left behind be cancelled?
//! Pure, so the host KATs drive the exact function the WM runs.

/// `flags` bits 0-4 of a kernel pointer record: the full button bitmap.
pub const BUTTONS: u8 = 0x1f;
/// `flags` bit 7: a transition was lost before this record.
pub const RESYNC: u8 = 0x80;

/// What one record means for the grab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Decision {
    /// The left button went down on this record (a RESYNC record restates
    /// the bitmap, so a button it shows held is never a fresh press).
    pub pressed: bool,
    /// Cancel the current grab before choosing a target: a RESYNC record
    /// with a button held (Jarvis, #46 RETURN F2). The held button may
    /// belong to a new press, so the old target must not receive it; the WM
    /// consumes until every button is up. A RESYNC record with every button
    /// up keeps the grab: it is the release the grabbed client missed.
    pub cancel_grab: bool,
    /// Every button is up after this record: the grab ends.
    pub release: bool,
}

pub fn decide(flags: u8, prev_buttons: u8, grabbed: bool) -> Decision {
    let buttons = flags & BUTTONS;
    let resync = flags & RESYNC != 0;
    Decision {
        pressed: buttons & !prev_buttons & 1 != 0 && !resync,
        cancel_grab: resync && buttons != 0 && grabbed,
        release: buttons == 0,
    }
}
