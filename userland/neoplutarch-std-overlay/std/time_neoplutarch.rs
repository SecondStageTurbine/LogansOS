// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.3: `std::sys::time::neoplutarch`, `Instant` on the kernel's
//! monotonic clock (`SYS_CLOCK_GETTIME(CLOCK_MONOTONIC)`, nanoseconds since
//! boot, nondecreasing, 10 ms resolution).
//!
//! Overlaid into `library/std/src/sys/time/neoplutarch.rs` by
//! `apply-overlay.py`. `SystemTime` stays std's `unsupported` one (its `now()`
//! panics): the kernel's `CLOCK_REALTIME` is wall-clock time only on kernels
//! built with `rtc-epoch` and boot-relative otherwise, and a process cannot tell
//! which, so it is not offered as the Unix epoch.

use crate::sys::neoplutarch_rt as rt;
use crate::time::Duration;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Instant(Duration);

impl Instant {
    pub fn now() -> Instant {
        match rt::monotonic_ns() {
            Ok(ns) => Instant(Duration::from_nanos(ns)),
            Err(e) => panic!("Instant::now: no monotonic clock (LogansOS errno {})", -e),
        }
    }

    pub fn checked_sub_instant(&self, other: &Instant) -> Option<Duration> {
        self.0.checked_sub(other.0)
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<Instant> {
        Some(Instant(self.0.checked_add(*other)?))
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<Instant> {
        Some(Instant(self.0.checked_sub(*other)?))
    }
}
