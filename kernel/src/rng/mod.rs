// SPDX-License-Identifier: MPL-2.0
//! Phase 10.1 — Kernel CSPRNG (ChaCha20 DRBG).
//! Phase 10.3 — Global singleton + fill_random helper for SYS_GETRANDOM.
//!
//! This module is compiled only under `#[cfg(feature = "kernel-rng")]`. When the
//! feature is OFF (the default), none of this code or its dependencies exist in
//! the kernel binary — the Rule-1 kill-switch.
//!
//! Architecture:
//!   entropy.rs  — RDRAND source + timer-jitter mixing (D1), fail-closed panic (D6)
//!   drbg.rs     — ChaCha20 DRBG with SHA-256 seed-mix (D2+D3), reseed guards
//!   kat.rs      — Known-answer tests: ChaCha20 RFC 8439, SHA-256 NIST, golden composition (D7)

mod drbg;
mod entropy;
mod kat;

use spin::Mutex;

pub use drbg::Csprng;
#[cfg(feature = "neo-test")]
pub use kat::{KatResult, run_all_kats};

/// Maximum bytes a single SYS_GETRANDOM call may request (ISC-2 D3).
pub const MAX_GETRANDOM_LEN: usize = 65536;

/// Global kernel CSPRNG singleton.
///
/// Initialized once at boot in `main.rs`. Accessed via `fill_random()` from
/// the SYS_GETRANDOM syscall path. Protected by a spinlock for SMP safety.
static KERNEL_RNG: Mutex<Option<Csprng>> = Mutex::new(None);

/// Initialize the global CSPRNG singleton at boot.
///
/// In neo-test mode, uses a fixed seed for deterministic KATs.
/// In production, seeds from RDRAND + jitter (fail-closed panic if unavailable).
#[cfg(feature = "neo-test")]
pub fn init() {
    let rng = Csprng::from_seed([0x42u8; 32]);
    *KERNEL_RNG.lock() = Some(rng);
}

#[cfg(not(feature = "neo-test"))]
pub fn init() {
    let rng = Csprng::init();
    *KERNEL_RNG.lock() = Some(rng);
}

/// Fill a kernel buffer with random bytes from the global DRBG.
///
/// Returns `Ok(())` on success. Returns `Err(-22 EINVAL)` if `len == 0` is
/// passed (callers should handle zero-length as a no-op before calling).
/// Returns `Err(-12 ENOMEM)` if the DRBG is uninitialized (shouldn't happen
/// after boot init).
///
/// # Panics
/// Panics if `len > MAX_GETRANDOM_LEN` — this is a kernel-internal API;
/// the syscall layer enforces the cap before calling.
pub fn fill_random(dest: &mut [u8]) -> Result<(), i64> {
    use crate::errno::{EINVAL, ENOMEM};

    if dest.is_empty() {
        return Err(EINVAL);
    }
    debug_assert!(
        dest.len() <= MAX_GETRANDOM_LEN,
        "fill_random: len {} exceeds MAX_GETRANDOM_LEN {}",
        dest.len(),
        MAX_GETRANDOM_LEN,
    );

    let mut guard = KERNEL_RNG.lock();
    let Some(rng) = guard.as_mut() else {
        return Err(ENOMEM);
    };
    rng.fill_bytes(dest);
    Ok(())
}
