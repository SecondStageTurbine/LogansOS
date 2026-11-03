// SPDX-License-Identifier: MPL-2.0
//! PR #15 re-check item 3 (input B): the WM lends a cap over a client's
//! channel without ever waiting on the client.
//!
//! A lend is four steps on the channel, in [`LEND`] order: the WM's offer
//! `O`, the peer's ready `R`, the grant, the peer's ack `A`. Each step is a
//! try. A step the peer isn't ready for yet is tried again after `between`
//! (the WM's turns for its other clients and the pointer), until the
//! deadline; then the lend fails as the peer's, never with the WM parked.
//! A peer that answers each step at once is lent to with no `between` at all.
//! `tools/atrium-logic-tests` checks this against peers that never take the
//! offer, answer late, or answer wrong.

/// One try's outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Try {
    Done,
    /// The peer isn't parked on its side yet (`EAGAIN`).
    NotReady,
    /// The channel failed, or the peer sent the wrong control byte.
    Failed,
}

/// One step of a lend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Offer,
    Ready,
    Grant,
    Ack,
}

/// The only order a lend runs in.
pub const LEND: [Step; 4] = [Step::Offer, Step::Ready, Step::Grant, Step::Ack];

/// The channel side of a lend: each call is one try, never a wait.
pub trait LendIo {
    /// Send `O`.
    fn try_offer(&mut self) -> Try;
    /// Receive one control byte; anything but `expected` is `Failed`.
    fn try_control(&mut self, expected: u8) -> Try;
    /// Grant the lent cap.
    fn try_grant(&mut self) -> Try;
}

/// Why a lend didn't complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LendError<E> {
    /// The peer's channel failed (or answered wrong) at this step.
    Failed(Step),
    /// The peer wasn't ready for this step by the deadline.
    TimedOut(Step),
    /// `between` failed: the WM's own error, passed through.
    Between(E),
}

/// Run a lend on `io`: each step tried until done, `between` run before each
/// retry, and no retry once `now()` has reached `deadline`.
pub fn lend<I: LendIo, E>(
    io: &mut I,
    deadline: u64,
    mut now: impl FnMut() -> u64,
    mut between: impl FnMut() -> Result<(), E>,
) -> Result<(), LendError<E>> {
    for step in LEND {
        loop {
            let tried = match step {
                Step::Offer => io.try_offer(),
                Step::Ready => io.try_control(b'R'),
                Step::Grant => io.try_grant(),
                Step::Ack => io.try_control(b'A'),
            };
            match tried {
                Try::Done => break,
                Try::Failed => return Err(LendError::Failed(step)),
                Try::NotReady => {
                    if now() >= deadline {
                        return Err(LendError::TimedOut(step));
                    }
                    between().map_err(LendError::Between)?;
                }
            }
        }
    }
    Ok(())
}
