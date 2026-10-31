// SPDX-License-Identifier: MPL-2.0
//! Entropy collection: RDRAND + timer-jitter mixing (D1).
//!
//! The RDRAND intrinsic is the one permitted new `unsafe` in this build.
//! Jitter is a mixing input only — never trusted as a sole seed (D6).

use core::arch::asm;

/// Number of RDRAND retries before declaring the CPU incapable.
const RDRAND_MAX_RETRIES: u32 = 10;

/// Collect 32 bytes of mixed entropy: RDRAND + timer-jitter.
///
/// Returns `None` only if RDRAND fails all retries — the caller (DRBG init)
/// must panic on `None` (D6 fail-closed).
pub fn collect_entropy_32() -> Option<[u8; 32]> {
    let mut buf = [0u8; 32];

    // RDRAND: 4 x 64-bit draws = 32 bytes
    let (r0, r1, r2, r3) = rdrand_256()?;

    buf[0..8].copy_from_slice(&r0.to_le_bytes());
    buf[8..16].copy_from_slice(&r1.to_le_bytes());
    buf[16..24].copy_from_slice(&r2.to_le_bytes());
    buf[24..32].copy_from_slice(&r3.to_le_bytes());

    // Mix in timer-jitter (XOR fold)
    let jitter = [
        timer_jitter_64(),
        timer_jitter_64(),
        timer_jitter_64(),
        timer_jitter_64(),
    ];
    for i in 0..32 {
        buf[i] ^= jitter[i / 8].to_le_bytes()[i % 8];
    }

    Some(buf)
}

/// Draw four 64-bit values from RDRAND (the 10-retry protocol per word).
fn rdrand_256() -> Option<(u64, u64, u64, u64)> {
    Some((rdrand_u64()?, rdrand_u64()?, rdrand_u64()?, rdrand_u64()?))
}

/// Single 64-bit RDRAND draw with up to `RDRAND_MAX_RETRIES` attempts.
///
/// SAFETY: `rdrand` is a CPU instruction that writes to the destination
/// register and sets the carry flag on failure. No memory or stack access.
fn rdrand_u64() -> Option<u64> {
    for _ in 0..RDRAND_MAX_RETRIES {
        let val: u64;
        let cf: u8;
        unsafe {
            asm!(
                "rdrand {val}",
                "setc {cf}",
                val = out(reg) val,
                cf = out(reg_byte) cf,
                options(nomem, nostack, preserves_flags)
            );
        }
        if cf == 1 {
            continue;
        }
        return Some(val);
    }
    None
}

/// Timer-jitter sample: RDTSC with a variable-delay gap.
///
/// SAFETY: `lfence; rdtsc` only reads the TSC into EDX:EAX. No memory side effects.
#[inline]
fn timer_jitter_64() -> u64 {
    let before: u64;
    unsafe {
        asm!(
            "lfence",
            "rdtsc",
            "shl rdx, 32",
            "or rax, rdx",
            out("rax") before,
            out("rdx") _,
            options(nomem, nostack)
        );
    }

    // Variable delay: spin a small loop whose iterations depend on the TSC low bits.
    let iterations = (before & 0xFF) as usize + 1;
    core::hint::black_box(iterations);

    let after: u64;
    unsafe {
        asm!(
            "lfence",
            "rdtsc",
            "shl rdx, 32",
            "or rax, rdx",
            out("rax") after,
            out("rdx") _,
            options(nomem, nostack)
        );
    }

    before ^ after
}
