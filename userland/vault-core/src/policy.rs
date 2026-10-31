// SPDX-License-Identifier: MPL-2.0
//! Phase-B KDF parameter floor (ADR-075 §F5).
//!
//! Before the running vault trusts the Argon2 parameters carried in a stored
//! record's header, it must enforce a lower bound. A record claiming weak
//! parameters (a downgrade attempt, or a record written by an older/hostile
//! writer) is **rejected, not honored** — otherwise an attacker who can supply
//! a record could force the vault to derive a key under cheap parameters and
//! then brute-force it offline.
//!
//! The floor is the `INTERACTIVE_V1` baseline from the crypto core. This module
//! is pure and host-testable; the running vault calls [`meets_floor`] on both
//! its own working parameters (at unlock) and on the parameters decoded from
//! any record before deriving/opening (at retrieve).

use crate::KdfParams;

/// The parameter floor. Argon2 parameters at or above this are trusted; any
/// dimension below it is rejected. This is exactly `KdfParams::INTERACTIVE_V1`
/// (64 MiB memory, 3 passes, 1 lane) — the ratified interactive baseline.
pub const FLOOR: KdfParams = KdfParams::INTERACTIVE_V1;

/// Return `true` iff every Argon2 cost dimension meets or exceeds the [`FLOOR`].
///
/// A record whose parameters fail this check must be refused before any key is
/// derived from it (fail-closed): honoring weak parameters would hand an
/// attacker a cheap offline oracle.
pub const fn meets_floor(p: KdfParams) -> bool {
    p.memory_kib >= FLOOR.memory_kib
        && p.iterations >= FLOOR.iterations
        && p.parallelism >= FLOOR.parallelism
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_v1_meets_its_own_floor() {
        assert!(meets_floor(KdfParams::INTERACTIVE_V1));
    }

    #[test]
    fn stronger_than_floor_is_accepted() {
        // More memory, more passes, more lanes — all above the floor.
        assert!(meets_floor(KdfParams {
            memory_kib: FLOOR.memory_kib * 2,
            iterations: FLOOR.iterations + 1,
            parallelism: FLOOR.parallelism + 3,
        }));
    }

    #[test]
    fn each_dimension_below_floor_is_rejected() {
        // Memory one KiB short.
        assert!(!meets_floor(KdfParams {
            memory_kib: FLOOR.memory_kib - 1,
            iterations: FLOOR.iterations,
            parallelism: FLOOR.parallelism,
        }));
        // One pass short.
        assert!(!meets_floor(KdfParams {
            memory_kib: FLOOR.memory_kib,
            iterations: FLOOR.iterations - 1,
            parallelism: FLOOR.parallelism,
        }));
        // Zero lanes.
        assert!(!meets_floor(KdfParams {
            memory_kib: FLOOR.memory_kib,
            iterations: FLOOR.iterations,
            parallelism: 0,
        }));
    }

    #[test]
    fn the_rfc_9106_kat_params_are_below_the_production_floor() {
        // The small RFC test vector parameters (32 KiB / 3 / 4) are test-only
        // and must NOT be honored as production parameters: memory is far below
        // the 64 MiB floor. This guards against a record smuggling KAT-weak
        // parameters into the live open path.
        assert!(!meets_floor(KdfParams::RFC_9106_KAT));
    }
}
