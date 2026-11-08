// SPDX-License-Identifier: MPL-2.0
//! OT-90 P4: `SYS_POWER`'s ABI, pinned. The numbers are what libneo and the
//! kernel both read (`#[path]`, one source); these KATs fail if anyone moves
//! one, so a userland built against an older libneo can't silently ask for
//! another action. The values come from the THINK (§6) and `docs/SDK-ABI.md`
//! (`SYS_POWER = 25`, the first free base slot when P4 landed), not from the
//! code.

use crate::power_abi::*;

#[test]
fn the_numbers_are_pinned() {
    assert_eq!(SYS_POWER, 25);
    assert_eq!(ACTION_POWER_OFF, 1);
    assert_eq!(ACTION_RESTART, 2);
    assert_eq!(RIGHT_POWER_OFF, 0b001);
    assert_eq!(RIGHT_RESTART, 0b010);
    assert_eq!(RIGHT_SLEEP, 0b100);
}

#[test]
fn a_holder_may_power_off_and_restart() {
    let both = RIGHT_POWER_OFF | RIGHT_RESTART;
    assert_eq!(
        decide(ACTION_POWER_OFF, 0, both, false),
        Ok(Request::PowerOff)
    );
    assert_eq!(decide(ACTION_RESTART, 0, both, false), Ok(Request::Restart));
    // Each action needs its own right.
    assert_eq!(
        decide(ACTION_POWER_OFF, 0, RIGHT_POWER_OFF, false),
        Ok(Request::PowerOff)
    );
    assert_eq!(
        decide(ACTION_RESTART, 0, RIGHT_POWER_OFF, false),
        Err(Refusal::NotPermitted)
    );
    assert_eq!(
        decide(ACTION_POWER_OFF, 0, RIGHT_RESTART, false),
        Err(Refusal::NotPermitted)
    );
    // The reserved sleep right grants neither.
    assert_eq!(
        decide(ACTION_POWER_OFF, 0, RIGHT_SLEEP, false),
        Err(Refusal::NotPermitted)
    );
}

#[test]
fn a_non_holder_is_refused() {
    assert_eq!(
        decide(ACTION_POWER_OFF, 0, 0, false),
        Err(Refusal::NotPermitted)
    );
    assert_eq!(
        decide(ACTION_RESTART, 0, 0, false),
        Err(Refusal::NotPermitted)
    );
}

#[test]
fn a_bad_call_is_invalid_whoever_makes_it() {
    let both = RIGHT_POWER_OFF | RIGHT_RESTART;
    for rights in [0, both] {
        assert_eq!(decide(0, 0, rights, false), Err(Refusal::Invalid));
        assert_eq!(decide(3, 0, rights, false), Err(Refusal::Invalid));
        assert_eq!(
            decide(ACTION_POWER_OFF, 1, rights, false),
            Err(Refusal::Invalid)
        );
        assert_eq!(
            decide(ACTION_RESTART, 1 << 63, rights, false),
            Err(Refusal::Invalid)
        );
    }
}

#[test]
fn a_running_shutdown_is_busy_for_a_holder_only() {
    let both = RIGHT_POWER_OFF | RIGHT_RESTART;
    assert_eq!(decide(ACTION_POWER_OFF, 0, both, true), Err(Refusal::Busy));
    // A non-holder learns nothing about the shutdown's state.
    assert_eq!(
        decide(ACTION_POWER_OFF, 0, 0, true),
        Err(Refusal::NotPermitted)
    );
}
