// SPDX-License-Identifier: MPL-2.0
//! Chad DS-1a F1: the order of a keyboard handoff between the desktop's
//! owners (Home, the WM's own focus; Arthur, a client's).
//!
//! ADR-074 routes each key to the one active focus object, and a focus
//! object's queue keeps what it was given while it is inactive. DS-1a changed
//! the view *before* the route: keys typed at a view already on screen were
//! queued for the previous owner and handed to it on its next turn (a stale
//! Enter could submit an intent). Every handoff now runs [`HANDOFF`]:
//!
//! 1. **Swap** the route to the new owner (one atomic kernel swap).
//! 2. **Discard** what the new owner's queue already held: keys left from its
//!    previous turn, or typed in the instant of the swap. Home is drained by
//!    the WM; a client drains its own queue as soon as it receives the
//!    message that hands it the turn (DS-1's intent), before it reads a key.
//! 3. **Present** the new owner's view.
//!
//! So no key crosses into a later turn of the same owner, and every key typed
//! while an owner's view is on screen and its route live reaches that owner.
//! A key typed after the swap but before the present (the old view still on
//! screen) is the new owner's — or, during the discard, dropped — never the
//! old owner's. `tools/atrium-logic-tests` checks this order against a model
//! with a key injected at every step boundary, and shows the DS-1a order
//! failing it.

/// One step of a handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Swap,
    Discard,
    Present,
}

/// The only order the WM hands the keyboard over in.
pub const HANDOFF: [Step; 3] = [Step::Swap, Step::Discard, Step::Present];

/// What a handoff does at each step.
pub trait Handoff {
    type Error;
    fn swap(&mut self) -> Result<(), Self::Error>;
    fn discard(&mut self) -> Result<(), Self::Error>;
    fn present(&mut self) -> Result<(), Self::Error>;
}

/// A handoff from three closures (the WM's form).
pub struct Steps<S, D, P> {
    pub swap: S,
    pub discard: D,
    pub present: P,
}

impl<E, S, D, P> Handoff for Steps<S, D, P>
where
    S: FnMut() -> Result<(), E>,
    D: FnMut() -> Result<(), E>,
    P: FnMut() -> Result<(), E>,
{
    type Error = E;
    fn swap(&mut self) -> Result<(), E> {
        (self.swap)()
    }
    fn discard(&mut self) -> Result<(), E> {
        (self.discard)()
    }
    fn present(&mut self) -> Result<(), E> {
        (self.present)()
    }
}

/// Run `order` (production: always [`HANDOFF`]).
pub fn hand_over<H: Handoff>(h: &mut H, order: &[Step]) -> Result<(), H::Error> {
    for step in order {
        match step {
            Step::Swap => h.swap()?,
            Step::Discard => h.discard()?,
            Step::Present => h.present()?,
        }
    }
    Ok(())
}
