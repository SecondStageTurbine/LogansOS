// SPDX-License-Identifier: MPL-2.0
//! OT-100 R3 (Muse's #104 observations 1 and 2): `gate_non_write`, the
//! guard's gate on `flush`, `write_protected` and a `Flush` request. A mock
//! inner device counts the calls that reach it: none while the device is
//! not armed, exactly one once it is.

use super::write_guard_logic::gate_non_write;
use std::cell::Cell;

/// The parts of an inner device the gate fronts, counting their calls.
#[derive(Default)]
struct Inner {
    flushes: Cell<u32>,
    wp_queries: Cell<u32>,
    flush_requests: Cell<u32>,
}

#[derive(Debug, PartialEq, Eq)]
enum Res {
    Ok,
    Unsupported,
}

impl Inner {
    fn flush(&self) -> Res {
        self.flushes.set(self.flushes.get() + 1);
        Res::Ok
    }
    fn write_protected(&self) -> Option<bool> {
        self.wp_queries.set(self.wp_queries.get() + 1);
        Some(false)
    }
    fn submit_flush(&self) -> Res {
        self.flush_requests.set(self.flush_requests.get() + 1);
        Res::Ok
    }
    fn calls(&self) -> (u32, u32, u32) {
        (
            self.flushes.get(),
            self.wp_queries.get(),
            self.flush_requests.get(),
        )
    }
}

/// What the guard does for each path, given its own answer.
fn guard(inner: &Inner, allowed: bool) -> (Res, Option<bool>, Res) {
    (
        gate_non_write(allowed, Res::Unsupported, || inner.flush()),
        gate_non_write(allowed, None, || inner.write_protected()),
        gate_non_write(allowed, Res::Unsupported, || inner.submit_flush()),
    )
}

#[test]
fn an_unarmed_device_is_never_reached() {
    let inner = Inner::default();
    assert_eq!(
        guard(&inner, false),
        (Res::Unsupported, None, Res::Unsupported)
    );
    assert_eq!(inner.calls(), (0, 0, 0));
}

#[test]
fn an_armed_device_is_reached_once_per_call() {
    let inner = Inner::default();
    assert_eq!(guard(&inner, true), (Res::Ok, Some(false), Res::Ok));
    assert_eq!(inner.calls(), (1, 1, 1));
}
