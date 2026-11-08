// SPDX-License-Identifier: MPL-2.0
//! PR #15 re-check item 3 (input B) discriminator: the WM's lend of a
//! permission cap to Arthur. Before the fix the lend's first step was a
//! blocking send, so an Arthur that took his permission reply and then
//! stopped receiving parked the WM for good: no other client's turn, no
//! reaping, no pointer. These peers check that a lend only ever tries, runs
//! the WM's turns between tries, and gives up at its deadline as the peer's
//! failure.

use super::atrium_lend::{LEND, LendError, LendIo, Step, Try, lend};
use std::cell::Cell;

/// A scripted peer: for each step, how many tries answer `NotReady` before
/// it answers `last` (`None`: `NotReady` forever).
struct Peer {
    waits: [Option<u32>; 4],
    last: [Try; 4],
    wrong_byte_at: Option<Step>,
    tries: Vec<Step>,
}

impl Peer {
    fn ready() -> Self {
        Peer {
            waits: [Some(0); 4],
            last: [Try::Done; 4],
            wrong_byte_at: None,
            tries: Vec::new(),
        }
    }

    fn answer(&mut self, step: Step) -> Try {
        self.tries.push(step);
        let i = LEND.iter().position(|&s| s == step).unwrap();
        match self.waits[i] {
            None => Try::NotReady,
            Some(0) => self.last[i],
            Some(n) => {
                self.waits[i] = Some(n - 1);
                Try::NotReady
            }
        }
    }
}

impl LendIo for Peer {
    fn try_offer(&mut self) -> Try {
        self.answer(Step::Offer)
    }
    fn try_control(&mut self, expected: u8) -> Try {
        let step = if expected == b'R' {
            Step::Ready
        } else {
            Step::Ack
        };
        assert!(expected == b'R' || expected == b'A');
        let t = self.answer(step);
        if t == Try::Done && self.wrong_byte_at == Some(step) {
            // The byte arrived but isn't `expected`.
            return Try::Failed;
        }
        t
    }
    fn try_grant(&mut self) -> Try {
        self.answer(Step::Grant)
    }
}

/// A clock that moves `tick` ns each time it is read.
fn clock(tick: u64) -> impl FnMut() -> u64 {
    let t = Cell::new(0u64);
    move || {
        let now = t.get();
        t.set(now + tick);
        now
    }
}

#[test]
fn a_ready_peer_is_lent_to_in_order_without_a_turn_between() {
    let mut peer = Peer::ready();
    let mut turns = 0;
    let r: Result<(), LendError<()>> = lend(&mut peer, 10, clock(1), || {
        turns += 1;
        Ok(())
    });
    assert_eq!(r, Ok(()));
    assert_eq!(peer.tries, LEND.to_vec());
    assert_eq!(turns, 0);
}

/// Chad's input B: the permission reply was taken, then Arthur never takes
/// the offer. The WM's turns keep running, and the lend ends at its deadline
/// as Arthur's failure; the pre-fix blocking send would never return.
#[test]
fn a_peer_that_never_takes_the_offer_times_out_with_the_turns_served() {
    let mut peer = Peer::ready();
    peer.waits[0] = None;
    let mut turns = 0u32;
    let r: Result<(), LendError<()>> = lend(&mut peer, 1_000, clock(10), || {
        turns += 1;
        Ok(())
    });
    assert_eq!(r, Err(LendError::TimedOut(Step::Offer)));
    // The clock reads 0, 10, ..., 1000: 100 retries, each after a turn.
    assert_eq!(turns, 100);
    assert!(peer.tries.iter().all(|&s| s == Step::Offer));
    assert_eq!(peer.tries.len(), 101);
}

#[test]
fn a_peer_that_stalls_at_any_later_step_times_out_there() {
    for (i, &step) in LEND.iter().enumerate().skip(1) {
        let mut peer = Peer::ready();
        peer.waits[i] = None;
        let mut turns = 0u32;
        let r: Result<(), LendError<()>> = lend(&mut peer, 50, clock(10), || {
            turns += 1;
            Ok(())
        });
        assert_eq!(r, Err(LendError::TimedOut(step)), "{step:?}");
        assert_eq!(turns, 5, "{step:?}");
        // Every step before it was done once, in order.
        assert_eq!(&peer.tries[..i], &LEND[..i]);
    }
}

#[test]
fn a_slow_peer_is_lent_to_with_a_turn_per_retry() {
    let mut peer = Peer::ready();
    peer.waits = [Some(2), Some(3), Some(0), Some(1)];
    let mut turns = 0u32;
    let r: Result<(), LendError<()>> = lend(&mut peer, u64::MAX, clock(1), || {
        turns += 1;
        Ok(())
    });
    assert_eq!(r, Ok(()));
    assert_eq!(turns, 6);
}

#[test]
fn a_wrong_control_byte_or_a_failed_channel_is_the_peer_s_failure() {
    let mut peer = Peer::ready();
    peer.wrong_byte_at = Some(Step::Ready);
    let r: Result<(), LendError<()>> = lend(&mut peer, 10, clock(1), || Ok(()));
    assert_eq!(r, Err(LendError::Failed(Step::Ready)));
    // Never granted after a wrong ready byte.
    assert!(!peer.tries.contains(&Step::Grant));

    let mut peer = Peer::ready();
    peer.last[2] = Try::Failed;
    let r: Result<(), LendError<()>> = lend(&mut peer, 10, clock(1), || Ok(()));
    assert_eq!(r, Err(LendError::Failed(Step::Grant)));
    assert!(!peer.tries.contains(&Step::Ack));
}

#[test]
fn the_wm_s_own_error_between_tries_is_passed_through() {
    let mut peer = Peer::ready();
    peer.waits[1] = None;
    let r = lend(&mut peer, u64::MAX, clock(1), || Err(141));
    assert_eq!(r, Err(LendError::Between(141)));
}

#[test]
fn a_deadline_already_passed_still_takes_a_ready_peer() {
    let mut peer = Peer::ready();
    let r: Result<(), LendError<()>> = lend(&mut peer, 0, clock(1), || Ok(()));
    assert_eq!(r, Ok(()));
}
